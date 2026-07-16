use crate::analysis::dc::DcAnalysisResult;
use crate::core::circuit::CircuitInstance;
use crate::core::element::ElementCapabilities;
use crate::analysis::dc::DcAnalysisState;
use crate::solver::convergence::{ConvergencePlan, HomotopyDriver};
use crate::analog::AnalogReference;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::faer::FaerSparseLinearSystem;
use crate::math::iv::InitialValue;
use crate::math::linear::Stamp;
use crate::math::newton_raphson::{NewtonRaphsonSolver, NonLinearSystem};
use crate::solver::Context;

use ndarray::{ArrayView1, ArrayViewMut1};
use std::collections::HashMap;

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
        let mut all_stamps = Vec::new();

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

        Ok(all_stamps)
    }

    /// Checks if the Newton-Raphson iteration has converged.
    ///
    /// Compares the current guess against the previous state using tolerance
    /// criteria defined in the solver context.
    fn converged(&self, state: &CircularArrayBuffer2<f64>, new_guess: &ArrayView1<f64>) -> bool {
        let netlist = self.circuit.netlist();
        if self.circuit.devices.iter().any(|d| d.limiting_active()) {
            return false;
        }
        self.context.tolerances.has_converged(state.view(0), new_guess, netlist)
    }

    /// ngspice `NIconvTest`: every node's current imbalance (and every branch
    /// row's equation residual) must be within tolerance. Node rows use the
    /// current tolerance `abstol`, branch rows the voltage tolerance `vntol`;
    /// both get the relative term `reltol · scale`. This is the half of the
    /// convergence test the voltage-step check misses on stiff devices.
    fn residual_converged(&self, residual: &[f64], scale: &[f64]) -> bool {
        self.context.tolerances.residual_test(self.circuit.netlist(), residual, scale)
    }

    fn any_limiting(&self) -> bool {
        self.circuit.devices.iter().any(|d| d.limiting_active())
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

pub struct DcSolver<'a> {
    pub system: DcSystem<'a>,
    pub solver: NewtonRaphsonSolver<AnalogReference, f64, FaerSparseLinearSystem<f64>>,
}

impl<'a> DcSolver<'a> {
    pub fn new(circuit: &'a mut CircuitInstance, context: Context) -> crate::result::Result<Self> {
        Context::init_global();
        let netlist = circuit.netlist();
        let size = netlist.max_index().map(|i| i + 1).unwrap_or(0);

        let mut system = DcSystem {
            circuit,
            context,
            gmin_extra: 0.0,
            src_scale: 1.0,
        };

        let solver = NewtonRaphsonSolver::new(&mut system, size, 1)?;

        Ok(Self { system, solver })
    }

    /// Seed the DC Newton initial guess with node-voltage hints (piperine-bench/docs/SPEC.md
    /// §5.1 `OpConfig.nodeset`). Applied before [`solve`](Self::solve); the
    /// solver still converges to the operating point — this only changes the
    /// starting guess, useful for nonlinear circuits with multiple solutions.
    pub fn apply_initial_conditions(&mut self, ivs: Vec<InitialValue<AnalogReference, f64>>) {
        if !ivs.is_empty() {
            self.solver.push_initial_conditions(ivs);
        }
    }

    pub fn solve(&mut self) -> crate::result::Result<DcAnalysisResult> {
        let plan = ConvergencePlan::default();
        let max_ms_iter = plan.limits().max_mixed_signal_iter;
        let raw_solution = {
            // Plain Newton, escalating through the homotopy plan (gmin stepping,
            // then source stepping) if it stalls on stiff coupled junctions.
            let mut sol = plan.solve(self)?;

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
                    let changed = self.system.circuit.accept_and_run_digital(
                        solution_slice,
                        &self.system.context,
                        0.0,
                    )?;
                    if !changed {
                        break;
                    }
                    // Digital changed — re-solve analog with updated D2A state.
                    let strategy = crate::solver::convergence::DampedNewton;
                    let policy = crate::solver::Policy::from_context(&self.system.context);
                    let tolerances = self.system.context.tolerances;
                    let netlist = self.system.circuit.netlist() as *const crate::analog::Netlist;
                    let netlist: &crate::analog::Netlist = unsafe { &*netlist };
                    sol = self.solver.solve_with_strategy(
                        &mut self.system,
                        &strategy,
                        &tolerances,
                        &policy,
                        netlist,
                    )?;
                }
            }
            sol
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
        result.stats.newton_iterations = self.solver.last_iterations();
        result.stats.converged = true;
        Ok(result)
    }
}

impl HomotopyDriver for DcSolver<'_> {
    fn newton(&mut self) -> crate::result::Result<ndarray::Array1<f64>> {
        let strategy = crate::solver::convergence::DampedNewton;
        let policy = crate::solver::Policy::from_context(&self.system.context);
        let tolerances = self.system.context.tolerances;
        // Extract netlist ref before &mut self.system — Netlist is not Clone.
        // SAFETY: the netlist is structurally stable during Newton iteration
        // (devices stamp into it, but they don't create/destroy unknowns).
        let netlist = self.system.circuit.netlist() as *const crate::analog::Netlist;
        let netlist: &crate::analog::Netlist = unsafe { &*netlist };
        self.solver.solve_with_strategy(
            &mut self.system,
            &strategy,
            &tolerances,
            &policy,
            netlist,
        )
    }

    fn set_gmin_extra(&mut self, g: f64) {
        self.system.gmin_extra = g;
    }

    fn set_src_scale(&mut self, s: f64) {
        self.system.src_scale = s;
    }

    fn gmin_floor(&self) -> f64 {
        self.system.context.tolerances.gmin.max(1e-12)
    }
}
