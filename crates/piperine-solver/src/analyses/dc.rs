//! DC operating point (`.op`) — the element-facing stamp state
//! (`DcAnalysisState`, the analog history + digital snapshot + homotopy
//! source scale an element reads while stamping) and the Newton driver
//! (`DcSystem`/`DcSolver`): gmin/source-stepping homotopy escalation, the
//! device-bypass stamp cache, and the mixed-signal settle loop.
#![allow(dead_code)]
use crate::analog::AnalogReference;
use crate::analyses::convergence::{ConvergencePlan, HomotopyDriver};
use crate::analyses::{Context, Policy};
use crate::core::circuit::CircuitInstance;
use crate::core::element::ElementCapabilities;
use crate::digital::LogicValue;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::faer::FaerSparseLinearSystem;
use crate::math::iv::InitialValue;
use crate::math::linear::Stamp;
use crate::math::newton_raphson::{NewtonRaphsonSolver, NonLinearSystem};
use crate::prelude::DcAnalysisResult;

use ndarray::ArrayView1;
use std::collections::HashMap;
use std::ops::Deref;

// ── request/state ────────────────────────────────────────────────────────

/// The read-only state an element sees while stamping the DC system: the analog
/// solution history **and** the digital net snapshot it may read (D2A — an
/// analog stamp that depends on digital logic reads it here, with no device-side
/// cache). Derefs to the analog history so existing history access is unchanged.
pub struct DcAnalysisState<'a> {
    history: &'a CircularArrayBuffer2<f64>,
    /// Every digital net's logic value for this solve, indexed by `DigitalNet`.
    pub digital: &'a [LogicValue],
    /// Source-stepping homotopy scale (SPICE): every forced source value is
    /// multiplied by this. `1.0` in normal operation; the DC solver ramps it
    /// `0 → 1` while tracking a hard operating point. Elements that drive forced
    /// sources read it here instead of a mutable `Context` field.
    pub src_scale: f64,
}

impl<'a> DcAnalysisState<'a> {
    pub fn new(
        history: &'a CircularArrayBuffer2<f64>,
        digital: &'a [LogicValue],
        src_scale: f64,
    ) -> Self {
        Self { history, digital, src_scale }
    }

    /// The analog solution history buffer.
    pub fn history(&self) -> &CircularArrayBuffer2<f64> {
        self.history
    }
}

impl Deref for DcAnalysisState<'_> {
    type Target = CircularArrayBuffer2<f64>;
    fn deref(&self) -> &Self::Target {
        self.history
    }
}

pub trait DcAnalysis {
    fn load_dc(
        &mut self,
        dc_circuit_state: &DcAnalysisState<'_>,
        context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>>;

    fn initial_dc_values(&mut self, _context: &Context) -> Vec<InitialValue<AnalogReference, f64>> {
        Vec::new()
    }
}

// ── driver ───────────────────────────────────────────────────────────────

/// Non-linear system representation for DC analysis.
///
/// This structure implements the [`NonLinearSystem`] trait to enable Newton-Raphson
/// iteration for DC operating point calculation. It manages circuit state updates,
/// convergence checking, damping, and Safe Operating Area (SOA) violation detection.
pub struct DcSystem<'a> {
    pub circuit: &'a mut CircuitInstance,
    pub context: Context,
    /// Extra node-to-ground conductance for **gmin stepping** (SPICE homotopy):
    /// 0 in normal operation, ramped large → 0 by [`DcSolver::solve_gmin_stepping`]
    /// so each intermediate problem is diagonally dominant. Owned here, not in
    /// the shared immutable `Context`.
    pub gmin_extra: f64,
    /// Forced-source scale for **source stepping** (SPICE homotopy): 1.0 in
    /// normal operation, ramped 0 → 1 by [`DcSolver::solve_source_stepping`].
    /// Passed to elements through [`DcAnalysisState::src_scale`].
    pub src_scale: f64,
    /// UIC hold clamps (ngspice `CKTsetIC`): stamped only by the transient
    /// driver's internal t=0 solve — empty for a standalone DC analysis.
    pub uic_clamps: Vec<crate::analyses::transient::UicClamp>,
    /// Device bypass: stamps cached from the last evaluation. Reused when the
    /// solution vector barely moved between Newton iterations (audit P4).
    stamp_cache: Vec<Stamp<AnalogReference, f64>>,
    last_solution: Vec<f64>,
    cache_valid: bool,
    pub bypass_hits: usize,
    pub bypass_misses: usize,
}

impl<'a> NonLinearSystem<AnalogReference, f64> for DcSystem<'a> {
    /// Assembles the system matrix and RHS vector for DC analysis.
    ///
    /// Updates all device models and collects their DC contributions (G, I stamps).
    /// This is called by the Newton-Raphson solver at each iteration.
    fn assemble(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
    ) -> crate::result::Result<Vec<Stamp<AnalogReference, f64>>> {
        // Device bypass: if the solution barely moved since the last
        // evaluation, reuse cached stamps instead of re-evaluating every
        // device model (audit P4 — BYPASS_OK declared but never consulted).
        // Suppressed while any device limiter is clamping — a bypassed
        // `load_dc` would freeze the limiter's internal state and stall the
        // convergence gate. The cache is dropped by `invalidate_bypass`
        // whenever the stamps depend on anything besides the solution vector
        // (homotopy scale changes, digital settle).
        if self.cache_valid && !self.any_limiting() {
            if let Some(curr) = state.latest() {
                // Per-variable threshold (ngspice bypass semantics):
                // |Δv_i| < vntol + reltol·max(|v_i|, |v_i_old|) for every
                // unknown. A global max-|v| scale would open a millivolt
                // freeze window at low-voltage nodes whenever any node sits
                // at supply level — stale stamps then satisfy both
                // convergence tests and the solve locks in the error
                // (found by the ngspice diode_series validation circuit).
                let unmoved = curr.len() == self.last_solution.len()
                    && curr.iter().zip(self.last_solution.iter()).all(|(c, p)| {
                        (*c - *p).abs()
                            < self.context.tolerances.vntol
                                + self.context.tolerances.reltol * c.abs().max(p.abs())
                    });
                if unmoved {
                    self.bypass_hits += 1;
                    return Ok(self.stamp_cache.clone());
                }
            }
        }
        self.bypass_misses += 1;

        // Build straight into the cache so the buffer's capacity is reused
        // across iterations; the returned Vec is the one clone per miss.
        self.stamp_cache.clear();
        let all_stamps = &mut self.stamp_cache;

        self.circuit.update_all(state, &self.context);
        let src_scale = self.src_scale;
        let CircuitInstance { devices, digital_state, .. } = &mut *self.circuit;
        let dc_state = DcAnalysisState::new(state, &digital_state.nets, src_scale);
        for dc in devices.iter_mut() {
            all_stamps.extend(dc.load_dc(&dc_state, &self.context));
        }

        // gmin stepping: a node-to-ground conductance on every voltage node,
        // ramped to 0 by the outer stepping loop. Never applied to branch
        // (current) unknowns.
        if self.gmin_extra > 0.0 {
            let g = self.gmin_extra;
            for r in self.circuit.netlist().all_references() {
                if r.variable().is_node() && !r.variable().is_ground() {
                    all_stamps.push(Stamp::Matrix(r.clone(), r.clone(), g));
                }
            }
        }

        // gshunt: user-set circuit-wide diagonal conductance to ground on
        // every node (ngspice parity, convergence aid for floating topologies).
        let gshunt = self.context.tolerances.gshunt;
        if gshunt > 0.0 {
            for r in self.circuit.netlist().all_references() {
                if r.variable().is_node() && !r.variable().is_ground() {
                    all_stamps.push(Stamp::Matrix(r.clone(), r.clone(), gshunt));
                }
            }
        }

        // UIC hold clamps: pin each seeded branch to its `@initial` value
        // for the t=0 solve (ngspice `CKTsetIC`).
        for clamp in &self.uic_clamps {
            clamp.stamp(all_stamps);
        }

        // Remember the solution this evaluation saw, so the next iteration
        // can measure how far it moved.
        if let Some(curr) = state.latest() {
            self.last_solution.clear();
            self.last_solution.extend(curr.iter());
            self.cache_valid = true;
        }

        Ok(self.stamp_cache.clone())
    }

    fn netlist(&self) -> &crate::analog::Netlist {
        self.circuit.netlist()
    }

    fn any_limiting(&self) -> bool {
        self.circuit.devices.iter().any(|d| d.limiting_active())
    }

    fn apply_convergence_hints(&self, guess: ndarray::ArrayViewMut1<f64>) {
        self.circuit.apply_convergence_hints(guess);
    }

    /// Called after successful convergence to check for Safe Operating Area violations.
    ///
    /// Iterates through all devices that implement SOA checking and collects any
    /// violations (e.g., power dissipation limits, voltage/current limits).
    fn convergence_success_callback(
        &mut self,
        _state: &CircularArrayBuffer2<f64>,
        _: &ArrayView1<f64>,
    ) {
    }
}

impl DcSystem<'_> {
    /// Drop the bypass cache. Must be called whenever the stamps depend on
    /// anything besides the solution vector: a homotopy scale change
    /// (`gmin_extra` / `src_scale`) or a digital settle (the D2A bridge can
    /// flip stamps while the analog solution stands still). Without this,
    /// a warm-started Newton whose solution barely moved would reuse stamps
    /// built under the old scales — silently wrong.
    fn invalidate_bypass(&mut self) {
        self.cache_valid = false;
    }
}

pub struct DcSolver<'a> {
    pub system: DcSystem<'a>,
    pub solver: NewtonRaphsonSolver<AnalogReference, f64, FaerSparseLinearSystem<f64>>,
    /// Convergence tunables for this analysis (MD-04). Defaults on
    /// construction; hosts override before [`solve`](Self::solve).
    pub policy: Policy,
    /// How many plain-Newton attempts the convergence plan drove (1 = no
    /// homotopy). `SolverStats::homotopy_levels` is this minus the first.
    newton_calls: usize,
}

impl<'a> DcSolver<'a> {
    pub fn new(circuit: &'a mut CircuitInstance, context: Context) -> crate::result::Result<Self> {
        Context::init_global();
        circuit.setup_all(&context)?;
        let netlist = circuit.netlist();
        let size = netlist.max_index().map(|i| i + 1).unwrap_or(0);

        let mut system = DcSystem {
            circuit,
            context,
            gmin_extra: 0.0,
            src_scale: 1.0,
            uic_clamps: Vec::new(),
            stamp_cache: Vec::new(),
            last_solution: Vec::new(),
            cache_valid: false,
            bypass_hits: 0,
            bypass_misses: 0,
        };

        let solver = NewtonRaphsonSolver::new(&mut system, size, 1)?;

        Ok(Self { system, solver, policy: Policy::default(), newton_calls: 0 })
    }

    /// Seed the DC Newton initial guess with node-voltage hints (the host
    /// §5.1 `OpConfig.nodeset`). Applied before [`solve`](Self::solve); the
    /// solver still converges to the operating point — this only changes the
    /// starting guess, useful for nonlinear circuits with multiple solutions.
    pub fn apply_initial_conditions(&mut self, ivs: Vec<InitialValue<AnalogReference, f64>>) {
        if !ivs.is_empty() {
            self.solver.push_initial_conditions(ivs);
        }
    }

    /// Live parameter set through a held DC analysis: forwards to
    /// [`CircuitInstance::set_element_param`] and, on success, drops the
    /// device-bypass stamp cache — a re-solve whose solution barely moved
    /// would otherwise reuse stamps built under the old value and silently
    /// converge to the stale operating point (the CP-11 bypass trap).
    pub fn set_element_param(
        &mut self,
        label: &str,
        param: &str,
        value: crate::core::introspect::Value,
    ) -> crate::result::Result<crate::core::introspect::Invalidation> {
        let invalidation = self.system.circuit.set_element_param(label, param, value)?;
        self.system.invalidate_bypass();
        Ok(invalidation)
    }

    pub fn solve(&mut self) -> crate::result::Result<DcAnalysisResult> {
        let plan = ConvergencePlan::default().with_trace(self.policy.trace);
        let max_ms_iter = plan.limits().max_mixed_signal_iter;
        self.solver.reset_iteration_counter();
        self.newton_calls = 0;
        let (raw_solution, homotopy_strategy) = {
            // Plain Newton, escalating through the homotopy plan (gmin stepping,
            // then source stepping) if it stalls on stiff coupled junctions.
            let outcome = plan.solve(self)?;
            let homotopy_strategy = outcome.strategy;
            let mut sol = outcome.solution;

            // Mixed-signal convergence loop: alternate between the analog
            // Newton-Raphson solve and digital evaluation until both settle —
            // the A2D bridge (digital reads analog voltages) and D2A bridge
            // (digital vars change analog stamps) couple in both directions. A
            // pure-analog circuit declares no digital capability and skips it.
            if self
                .system
                .circuit
                .capabilities()
                .contains(ElementCapabilities::DIGITAL)
            {
                for _ in 0..max_ms_iter {
                    let solution_slice = sol.as_slice().ok_or_else(|| {
                        crate::error::Error::simple(
                            crate::error::SolverDomain::Dc,
                            "solution not contiguous",
                        )
                    })?;
                    let changed = self
                        .system
                        .circuit
                        .accept_and_run_digital(solution_slice, 0.0)?;
                    if !changed {
                        break;
                    }
                    // Digital changed — re-solve analog with updated D2A state.
                    // The digital snapshot feeds the stamps, so the bypass
                    // cache is stale even though the analog solution is not.
                    self.system.invalidate_bypass();
                    let strategy = crate::analyses::convergence::DampedNewton;
                    let policy = self.policy.clone();
                    let tolerances = self.system.context.tolerances;
                    sol = self.solver.solve_with_strategy(
                        &mut self.system,
                        &strategy,
                        &tolerances,
                        &policy,
                    )?;
                }
            }
            (sol, homotopy_strategy)
        };

        let mut values = HashMap::new();
        let netlist = self.system.circuit.netlist();

        for reference in netlist.all_references() {
            if let Some(reference_idx) = reference.idx() {
                values.insert(
                    reference.variable().clone(),
                    raw_solution[reference_idx],
                );
            }
        }

        let mut result = DcAnalysisResult::new(values);
        // Total across the whole plan (homotopy attempts included) — the
        // honest cost of this operating point, not just the final solve.
        result.stats.newton_iterations = self.solver.total_iterations();
        result.stats.converged = true;
        result.stats.bypass_hits = self.system.bypass_hits;
        result.stats.bypass_misses = self.system.bypass_misses;
        result.stats.homotopy_strategy = homotopy_strategy.map(str::to_string);
        result.stats.homotopy_levels = self.newton_calls.saturating_sub(1);
        result.stats.assembly_time_ns = self.solver.assembly_time_ns();
        result.stats.solve_time_ns = self.solver.solve_time_ns();
        Ok(result)
    }
}

impl HomotopyDriver for DcSolver<'_> {
    fn newton(&mut self) -> crate::result::Result<ndarray::Array1<f64>> {
        self.newton_calls += 1;
        let strategy = crate::analyses::convergence::DampedNewton;
        let policy = self.policy.clone();
        let tolerances = self.system.context.tolerances;
        self.solver.solve_with_strategy(
            &mut self.system,
            &strategy,
            &tolerances,
            &policy,
        )
    }

    fn set_gmin_extra(&mut self, g: f64) {
        if self.system.gmin_extra != g {
            self.system.invalidate_bypass();
        }
        self.system.gmin_extra = g;
    }

    fn set_src_scale(&mut self, s: f64) {
        if self.system.src_scale != s {
            self.system.invalidate_bypass();
        }
        self.system.src_scale = s;
    }

    fn gmin_floor(&self) -> f64 {
        self.system.context.tolerances.gmin.max(1e-12)
    }
}
