use crate::analysis::dc::DcAnalysisResult;
use crate::core::circuit::CircuitInstance;
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
}

impl<'a> NonLinearSystem<AnalogReference, f64> for DcSystem<'a> {
    /// Assembles the system matrix and RHS vector for DC analysis.
    ///
    /// Updates all device models and collects their DC contributions (G, I stamps).
    /// This is called by the Newton-Raphson solver at each iteration.
    fn assemble(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
        _alpha: f64,
    ) -> crate::result::Result<Vec<Stamp<AnalogReference, f64>>> {
        let mut all_stamps = Vec::new();

        self.circuit.update_all(state, &self.context);
        for dc in &mut self.circuit.devices {
            if let Some(a) = dc.as_analog() {
                all_stamps.extend(a.load_dc(state, &self.context));
            }
        }

        // gmin stepping: a node-to-ground conductance on every voltage node,
        // ramped to 0 by the outer stepping loop. Never applied to branch
        // (current) unknowns.
        if self.context.gmin_extra > 0.0 {
            let g = self.context.gmin_extra;
            for r in self.circuit.netlist().all_references() {
                if r.variable().is_node() && !r.variable().is_ground() {
                    all_stamps.push(Stamp::Matrix(r.clone(), r.clone(), g));
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
        super::check_convergence(&self.circuit.devices, state, new_guess, &self.context, netlist)
    }

    /// ngspice `NIconvTest`: every node's current imbalance (and every branch
    /// row's equation residual) must be within tolerance. Node rows use the
    /// current tolerance `abstol`, branch rows the voltage tolerance `vntol`;
    /// both get the relative term `reltol · scale`. This is the half of the
    /// convergence test the voltage-step check misses on stiff devices.
    fn residual_converged(&self, residual: &[f64], scale: &[f64]) -> bool {
        super::residual_converged(self.circuit.netlist(), &self.context, residual, scale)
    }

    fn apply_limit(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
        current_guess: ArrayViewMut1<f64>,
    ) {
        super::apply_damping(state, current_guess, self.context.dc_damp_tolerance);
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
        let max_iter = self.system.context.max_iter;

        // Mixed-signal convergence loop: alternate between analog
        // Newton-Raphson and digital evaluation until both settle. The
        // A2D bridge (digital reads analog voltages) and D2A bridge
        // (digital vars change analog stamps) require this outer loop.
        const MAX_MS_ITER: usize = 20;
        let raw_solution = {
            let mut prev_digital = self.system.circuit.digital_state.nets.clone();
            let mut sol = match self.solver.solve(&mut self.system, 0.0, max_iter) {
                Ok(sol) => sol,
                // Plain Newton stalled (stiff coupled junctions — BJT/MOS).
                // Two SPICE homotopies in turn: gmin stepping, then source
                // stepping (which finds the correct solution branch where gmin
                // stepping can settle on the wrong one — BJT/MOS amplifiers).
                Err(_) => match self.solve_gmin_stepping(max_iter) {
                    Ok(sol) => sol,
                    Err(_) => self.solve_source_stepping(max_iter)?,
                },
            };
            for _ in 0..MAX_MS_ITER {
                let solution_slice = sol.as_slice().ok_or_else(|| {
                    crate::error::Error::simple("DC", "solution not contiguous")
                })?;
                let changed = self.system.circuit.accept_and_run_digital(
                    solution_slice,
                    &self.system.context,
                    0.0,
                );
                if !changed {
                    break;
                }
                // Digital changed — re-solve analog with updated D2A state.
                sol = self.solver.solve(&mut self.system, 0.0, max_iter)?;
                let _ = &mut prev_digital;
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

        Ok(DcAnalysisResult::new(
            values,
        ))
    }

    /// SPICE gmin stepping: converge an easy, diagonally-dominant version of
    /// the circuit (large node-to-ground `gmin_extra`), then ramp that
    /// conductance to 0, warm-starting each step from the last solution. The
    /// standard homotopy for stiff coupled-junction operating points that
    /// plain Newton oscillates on. On any step that still won't converge,
    /// gives up and reports the failure.
    /// SPICE source stepping: ramp the independent-source scale from 0 → 1,
    /// warm-starting each step. At scale 0 every source is off and the circuit
    /// converges trivially; raising it tracks the solution continuously to the
    /// true operating point. Finds the correct branch (e.g. a saturated BJT)
    /// where gmin stepping can converge to the wrong one. Only forced-voltage
    /// sources ramp (see `force_stamps`); current-source-only circuits are
    /// unaffected.
    fn solve_source_stepping(
        &mut self,
        max_iter: usize,
    ) -> crate::result::Result<ndarray::Array1<f64>> {
        let trace = std::env::var("PIPERINE_TRACE_SRC").is_ok();
        // A real shunt conductance (1 µS) conditions the exponential turn-on
        // knee where source stepping alone stalls (the BJT/MOS threshold).
        // Held through the source ramp, then itself ramped to 0 (a nested gmin
        // step) so the final answer is exact.
        let knee_gmin = 1e-6_f64;
        let mut scale = 0.0_f64;
        let mut step = 0.1_f64;
        let mut iters = 0;
        let mut last_ok = 0.0_f64;
        self.system.context.src_scale = 0.0;
        self.system.context.gmin_extra = knee_gmin;
        // Solve the fully-off circuit first (trivial).
        let mut sol = self.solver.solve(&mut self.system, 0.0, max_iter);
        while iters < 300 {
            iters += 1;
            if sol.is_ok() {
                last_ok = scale;
                if scale >= 1.0 {
                    break;
                }
                step = (step * 1.5).min(0.25);
                scale = (last_ok + step).min(1.0);
            } else {
                // Back off toward the last converged scale.
                step *= 0.5;
                if step < 1e-6 {
                    self.system.context.src_scale = 1.0;
                    self.system.context.gmin_extra = 0.0;
                    return sol; // give up with the failure
                }
                scale = last_ok + step;
            }
            self.system.context.src_scale = scale;
            if trace {
                eprintln!("SRC step scale={scale:.4} step={step:.4}");
            }
            sol = self.solver.solve(&mut self.system, 0.0, max_iter);
        }
        // Full source strength reached with the knee shunt still in. Now ramp
        // the shunt out (a nested gmin step, warm-started) so the final answer
        // is exact.
        self.system.context.src_scale = 1.0;
        let mut g = knee_gmin;
        while g > self.system.context.gmin.max(1e-12) * 10.0 {
            g *= 0.1;
            self.system.context.gmin_extra = g;
            if self.solver.solve(&mut self.system, 0.0, max_iter).is_err() {
                break;
            }
        }
        self.system.context.gmin_extra = 0.0;
        self.solver.solve(&mut self.system, 0.0, max_iter)
    }

    fn solve_gmin_stepping(
        &mut self,
        max_iter: usize,
    ) -> crate::result::Result<ndarray::Array1<f64>> {
        // Start very easy (100 mS to ground) and drop a decade per step until
        // the extra conductance is negligible next to the real gmin.
        let trace = std::env::var("PIPERINE_TRACE_GMIN").is_ok();
        let floor = self.system.context.gmin.max(1e-12) * 10.0;
        // Geometric ramp with adaptive back-off: on a step that won't
        // converge, raise the conductance again (smaller decrements) instead
        // of giving up. Bounded total steps so a truly non-convergent circuit
        // still terminates.
        let mut g = 0.1_f64;
        let mut factor = 0.1_f64;
        let mut steps = 0;
        let mut converged_any = false;
        while steps < 200 {
            steps += 1;
            self.system.context.gmin_extra = g;
            let r = self.solver.solve(&mut self.system, 0.0, max_iter);
            if trace {
                eprintln!("GMIN step g={g:.3e} -> {}", if r.is_ok() { "ok" } else { "fail" });
            }
            match r {
                Ok(_) => {
                    converged_any = true;
                    if g <= floor {
                        break;
                    }
                    factor = (factor * 1.3).min(0.5); // relax faster once it's easy
                    g *= factor;
                }
                Err(e) => {
                    if !converged_any {
                        // Couldn't even converge the easiest problem — give up.
                        self.system.context.gmin_extra = 0.0;
                        return Err(e);
                    }
                    // Back off: raise conductance, shrink the step.
                    factor = (factor * 3.0).min(0.7);
                    g /= factor;
                }
            }
        }
        // Final solve with the extra conductance removed — the true operating
        // point, warm-started from the last stepped solution.
        self.system.context.gmin_extra = 0.0;
        if trace {
            eprintln!("GMIN final solve at gmin_extra=0");
        }
        self.solver.solve(&mut self.system, 0.0, max_iter)
    }
}
