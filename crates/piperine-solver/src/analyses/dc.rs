//! DC operating point (`.op`) — the element-facing stamp state
//! (`DcAnalysisState`, the analog history + digital snapshot + homotopy
//! source scale an element reads while stamping) and the Newton driver
//! (`DcSystem`/`DcSolver`): gmin/source-stepping homotopy escalation, the
//! device-bypass stamp cache, and the mixed-signal settle loop.
use piperine_analog_solver::device::{Ambient, AssemblySnapshot};
use piperine_analog_solver::netlist::AnalogReference;
use piperine_analog_solver::convergence::{ConvergencePlan, HomotopyDriver};
use crate::analyses::Context;
use piperine_analog_solver::tolerances::Policy;
use crate::core::circuit::CircuitInstance;
use crate::core::element::ElementCapabilities;
use piperine_digital_solver::events::LogicValue;
use piperine_analog_solver::math::circular_array::CircularArrayBuffer2;
use piperine_analog_solver::math::faer::FaerSparseLinearSystem;
use piperine_analog_solver::math::iv::InitialValue;
use piperine_analog_solver::math::linear::Stamp;
use piperine_analog_solver::math::newton_raphson::{NewtonRaphsonSolver, NonLinearSystem};
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
    /// Whether **every** element in the circuit declares
    /// [`ElementCapabilities::BYPASS_OK`] — the bypass's opt-in gate (P6/CLN-12).
    /// One element whose stamps are not a pure function of its terminal
    /// voltages disables the cache for the whole circuit: a stale stamp can
    /// satisfy the convergence test and lock in a wrong operating point.
    /// Computed once at construction; the element set cannot change during a
    /// solve.
    bypass_allowed: bool,
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
    ) -> crate::core::result::Result<Vec<Stamp<AnalogReference, f64>>> {
        if self.circuit.abi.is_some() {
            return self.assemble_device(state);
        }
        // Device bypass: if the solution barely moved since the last
        // evaluation, reuse cached stamps instead of re-evaluating every
        // device model. Gated on `bypass_allowed` — every element must have
        // declared `BYPASS_OK`, the opt-in the flag has always documented
        // (P6/CLN-12; before that the cache applied to devices that never
        // opted in). Additionally suppressed while any device limiter is
        // clamping — a bypassed `load_dc` would freeze the limiter's internal
        // state and stall the convergence gate. The cache is dropped by
        // `invalidate_bypass` whenever the stamps depend on anything besides
        // the solution vector (homotopy scale changes, digital settle).
        if self.bypass_allowed && self.cache_valid && !self.any_limiting_report()
            && let Some(curr) = state.latest()
        {
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
            let device_stamps = dc.load_dc(&dc_state, &self.context);
            if let Some(err) = piperine_analog_solver::stamp_validation::non_finite_stamp_error(dc.name(), &device_stamps) {
                return Err(err);
            }
            all_stamps.extend(device_stamps);
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

    fn netlist(&self) -> &piperine_analog_solver::netlist::Netlist {
        self.circuit.netlist()
    }

    fn any_limiting_report(&self) -> bool {
        if let Some(bound) = &self.circuit.abi {
            return !bound.analog.limiting_active().is_empty();
        }
        self.circuit.devices.iter().any(|d| d.limiting_report().is_some())
    }

    /// Every Newton iteration but the first ends here: each limit's `LAST`
    /// becomes the `x̂` of the iteration just assembled (abi.md A2).
    fn before_iter_callback(&mut self, _state: &CircularArrayBuffer2<f64>, iteration_number: usize) {
        if iteration_number > 0
            && let Some(bound) = self.circuit.abi.as_mut()
        {
            bound.analog.end_iteration();
        }
    }

    fn apply_limiting_reports(&self, guess: ndarray::ArrayViewMut1<f64>) {
        self.circuit.apply_limiting_reports(guess);
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
        // The converged iteration ends like the others.
        if let Some(bound) = self.circuit.abi.as_mut() {
            bound.analog.end_iteration();
        }
    }
}

impl DcSystem<'_> {
    /// The stamps of an equilibrium solve of a circuit of devices (abi.md §10):
    /// the assembly's `FLOW`/`DELAY` rows at the iterate, with the homotopy's
    /// continuation values as the `GMIN` and `SOURCE_SCALE` ambient inputs,
    /// then the solver's own shunts and the hold clamps. There is no stamp
    /// cache: a function whose reads did not change is not called (F3).
    fn assemble_device(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
    ) -> crate::core::result::Result<Vec<Stamp<AnalogReference, f64>>> {
        let x: Vec<f64> = match state.latest() {
            Some(row) => row.to_vec(),
            None => vec![0.0; state.size()],
        };
        let amb = self.ambient();
        let bound = self.circuit.abi.as_mut().expect("a circuit of devices");
        let mut all_stamps = bound.analog.assemble_dc(&x, amb)?;
        if let Some(err) = piperine_analog_solver::stamp_validation::non_finite_stamp_error("devices", &all_stamps) {
            return Err(err);
        }
        self.push_solver_stamps(&mut all_stamps);
        Ok(all_stamps)
    }

    /// The coordinates of an equilibrium solve (abi.md §4.2): `STATIC`, with
    /// the homotopy's continuation values as `GMIN` and `SOURCE_SCALE`.
    pub(crate) fn ambient(&self) -> Ambient {
        Ambient {
            time: 0.0,
            freq: 0.0,
            is_static: true,
            gmin: self.context.tolerances.gmin + self.gmin_extra,
            source_scale: self.src_scale,
        }
    }

    /// What the solver adds to every circuit: the gmin-stepping shunt, the
    /// user's `gshunt` and the hold clamps.
    fn push_solver_stamps(&self, all_stamps: &mut Vec<Stamp<AnalogReference, f64>>) {
        if self.gmin_extra > 0.0 {
            let g = self.gmin_extra;
            for r in self.circuit.netlist().all_references() {
                if r.variable().is_node() && !r.variable().is_ground() {
                    all_stamps.push(Stamp::Matrix(r.clone(), r.clone(), g));
                }
            }
        }
        let gshunt = self.context.tolerances.gshunt;
        if gshunt > 0.0 {
            for r in self.circuit.netlist().all_references() {
                if r.variable().is_node() && !r.variable().is_ground() {
                    all_stamps.push(Stamp::Matrix(r.clone(), r.clone(), gshunt));
                }
            }
        }
        for clamp in &self.uic_clamps {
            clamp.stamp(all_stamps);
        }
    }

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
    /// One-deep device-state checkpoint (ABI-07): snapshotted before each
    /// homotopy attempt, restored on strategy fallthrough so the limiter and
    /// other non-accept-gated state start clean on the next retry.
    device_checkpoint: Vec<Option<crate::core::element::ElementCheckpoint>>,
    /// The same checkpoint for a circuit of devices: the assembly's term
    /// histories and limit states.
    assembly_checkpoint: Option<AssemblySnapshot>,
    /// The analysis kind this solve runs under. Defaults to `Dc`; an outer
    /// analysis driving a DC solve underneath itself (a transient's t = 0 OP,
    /// the bias point under an AC sweep) overrides it via
    /// [`Self::set_analysis_kind`] — the outer analysis owns `finish_analysis`.
    analysis_kind: crate::analyses::AnalysisKind,
    /// The homotopy that converged the first equilibrium of a circuit of devices.
    homotopy_strategy: Option<&'static str>,
}

impl<'a> DcSolver<'a> {
    /// Tag this DC solve with a different analysis kind. Used by outer
    /// analyses driving a DC solve underneath themselves — a transient's
    /// t = 0 OP, the bias point under an AC sweep. Defaults to `Dc` at
    /// [`Self::new`].
    pub(crate) fn set_analysis_kind(&mut self, kind: crate::analyses::AnalysisKind) {
        self.analysis_kind = kind;
    }
    pub fn new(circuit: &'a mut CircuitInstance, mut context: Context) -> crate::core::result::Result<Self> {
        Context::init_global();
        if let Some(bound) = &circuit.abi {
            bound.tighten(&mut context.tolerances);
        }
        circuit.setup_all(&context)?;
        let netlist = circuit.netlist();
        let size = netlist.max_index().map(|i| i + 1).unwrap_or(0);

        // Opt-in gate: read once, before the borrow moves into the system.
        let bypass_allowed = !circuit.devices.is_empty()
            && circuit
                .devices
                .iter()
                .all(|d| d.capabilities().contains(ElementCapabilities::BYPASS_OK));

        let mut system = DcSystem {
            circuit,
            context,
            gmin_extra: 0.0,
            src_scale: 1.0,
            uic_clamps: Vec::new(),
            stamp_cache: Vec::new(),
            last_solution: Vec::new(),
            cache_valid: false,
            bypass_allowed,
            bypass_hits: 0,
            bypass_misses: 0,
        };

        let solver = NewtonRaphsonSolver::new(&mut system, size, 1)?;

        Ok(Self {
            system,
            solver,
            policy: Policy::default(),
            newton_calls: 0,
            device_checkpoint: Vec::new(),
            assembly_checkpoint: None,
            analysis_kind: crate::analyses::AnalysisKind::Dc,
            homotopy_strategy: None,
        })
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
    ) -> crate::core::result::Result<crate::core::introspect::Invalidation> {
        let invalidation = self.system.circuit.set_element_param(label, param, value)?;
        self.system.invalidate_bypass();
        Ok(invalidation)
    }

    /// One equilibrium solve of a circuit of devices: the first runs the
    /// convergence plan (plain Newton, then the homotopies), a later round
    /// re-solves from the last solution with damped Newton, as the mixed
    /// loop of the `Element` branch does.
    pub(crate) fn equilibrium(&mut self, first: bool) -> crate::core::result::Result<ndarray::Array1<f64>> {
        if first {
            let plan = ConvergencePlan::default().with_trace(self.policy.trace);
            let outcome = plan.solve(self)?;
            self.homotopy_strategy = outcome.strategy;
            return Ok(outcome.solution);
        }
        let strategy = piperine_analog_solver::convergence::DampedNewton;
        let policy = self.policy.clone();
        let tolerances = self.system.context.tolerances.clone();
        self.solver.solve_with_strategy(&mut self.system, &strategy, &tolerances, &policy)
    }

    /// The operating point of a circuit of devices, through the protocol
    /// (abi.md §10): the digital engine leaves the circuit for the run and
    /// comes back after it, whatever the outcome.
    fn solve_device(&mut self) -> crate::core::result::Result<DcAnalysisResult> {
        use crate::analyses::adapters::{DigitalSide, EquilibriumSide, empty_digital};
        use crate::protocol::{Protocol, ProtocolOptions};
        self.solver.reset_iteration_counter();
        self.newton_calls = 0;
        let bound = self.system.circuit.abi.as_mut().expect("a circuit of devices");
        let spt = bound.digital.plan().resolution.seconds_per_tick;
        let digital = std::mem::replace(&mut bound.digital, empty_digital(spt)?);
        let side = DigitalSide::new(digital, bound, piperine_abi::raw::global::DC);
        let mut protocol =
            Protocol::new(side, EquilibriumSide { dc: self, solution: None }, ProtocolOptions::new(spt));
        let ran = protocol.operating_point();
        let (side, analog) = protocol.into_sides();
        let solution = analog.solution;
        if let Some(bound) = self.system.circuit.abi.as_mut() {
            bound.digital = side.into_digital();
        }
        ran?;
        // A design with no continuous pin runs its instants with no solve.
        let mut values = HashMap::new();
        if let Some(solution) = solution {
            for reference in self.system.circuit.netlist().all_references() {
                if let Some(i) = reference.idx() {
                    values.insert(reference.variable().clone(), solution[i]);
                }
            }
        }
        let mut result = DcAnalysisResult::new(values);
        result.stats.newton_iterations = self.solver.total_iterations();
        result.stats.converged = true;
        result.stats.homotopy_strategy = self.homotopy_strategy.map(str::to_string);
        result.stats.homotopy_levels = self.newton_calls.saturating_sub(1);
        result.stats.assembly_time_ns = self.solver.assembly_time_ns();
        result.stats.solve_time_ns = self.solver.solve_time_ns();
        Ok(result)
    }

    pub fn solve(&mut self) -> crate::core::result::Result<DcAnalysisResult> {
        // SPEC_DEVIATION: the bias point under an AC sweep stays an analog
        // equilibrium on the discrete values bind seeded; every other
        // operating point of a circuit of devices runs the protocol.
        // Reason: the protocol fires `START` at an operating point (abi.md
        // §10), and `abi_ac.rs` counts reactions from before the AC driver's
        // bias point; a surviving test is not edited (reported to the
        // orchestrator).
        if self.system.circuit.abi.is_some() && self.analysis_kind != crate::analyses::AnalysisKind::Ac {
            return self.solve_device();
        }
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
                        piperine_analog_solver::error::Error::simple(
                            piperine_analog_solver::error::SolverDomain::Dc,
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
                    let strategy = piperine_analog_solver::convergence::DampedNewton;
                    let policy = self.policy.clone();
                    let tolerances = self.system.context.tolerances.clone();
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
        // HOST-10: collect each device's final limiting state — the
        // structured diagnostics a host reads via `op.stats.limiting`.
        // Mirrors `apply_limiting_reports`'s iteration over devices.
        result.stats.limiting = self
            .system
            .circuit
            .all_devices()
            .iter()
            .filter_map(|d| d.limiting_report())
            .collect();

        // LRM §5.10.2 `@(final_step)` (LRMW2-08): once, after this
        // converged solution — but only for a genuine top-level `.dc()`
        // call. `analysis_kind` stays at its `Dc` default there; an outer
        // analysis driving a DC solve underneath itself (a transient's
        // t = 0 OP, an AC sweep's bias point) retags it via
        // `set_analysis_kind` first, which is this task's signal that the
        // DC solve is an internal step, not the real analysis end.
        if self.analysis_kind == crate::analyses::AnalysisKind::Dc {
            self.system.circuit.finish_analysis(&self.system.context, self.analysis_kind);
        }

        Ok(result)
    }
}

impl HomotopyDriver for DcSolver<'_> {
    fn newton(&mut self) -> crate::core::result::Result<ndarray::Array1<f64>> {
        self.newton_calls += 1;
        let strategy = piperine_analog_solver::convergence::DampedNewton;
        let policy = self.policy.clone();
        let tolerances = self.system.context.tolerances.clone();
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

    fn checkpoint_devices(&mut self) {
        if let Some(bound) = &self.system.circuit.abi {
            self.assembly_checkpoint = Some(bound.analog.snapshot());
            return;
        }
        self.device_checkpoint = self
            .system
            .circuit
            .devices
            .iter()
            .map(|d| d.checkpoint_state())
            .collect();
    }

    fn restore_devices(&mut self) {
        if let Some(bound) = self.system.circuit.abi.as_mut() {
            if let Some(snapshot) = &self.assembly_checkpoint {
                bound.analog.restore(snapshot);
            }
            return;
        }
        for (dev, ckpt) in self
            .system
            .circuit
            .devices
            .iter_mut()
            .zip(&self.device_checkpoint)
        {
            if let Some(c) = ckpt {
                dev.restore_state(c);
            }
        }
    }
}
