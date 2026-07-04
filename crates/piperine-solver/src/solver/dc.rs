use crate::analysis::dc::DcAnalysisResult;
use crate::circuit::CircuitInstance;
use crate::analog::AnalogReference;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::faer::FaerSparseLinearSystem;
use crate::math::iv::InitialValue;
use crate::math::linear::Stamp;
use crate::math::newton_raphson::{NewtonRaphsonSolver, NonLinearSystem};
use crate::solver::Context;
use log::debug;
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
        for dc in self.circuit.all_devices_mut() {
            all_stamps.extend(dc.load_dc(state, &self.context));
        }

        Ok(all_stamps)
    }

    /// Checks if the Newton-Raphson iteration has converged.
    ///
    /// Compares the current guess against the previous state using tolerance
    /// criteria defined in the solver context.
    fn converged(&self, state: &CircularArrayBuffer2<f64>, new_guess: &ArrayView1<f64>) -> bool {
        for device in self.circuit.all_devices() {
            if device.limiting_active() {
                debug!("Device {} requested limiting reiteration", device.device_name());
                return false;
            }
        }
        let netlist = self.circuit.netlist();
        self.context
            .has_converged(state.view(0), new_guess, netlist)
    }

    /// Applies damping to limit step size and improve convergence stability.
    ///
    /// If the L2 norm of the update vector exceeds `dc_damp_tolerance`, this method
    /// averages the current guess with the previous guess (0.5 damping factor).
    /// This prevents large oscillations that can destabilize the Newton-Raphson solver.
    fn apply_limit(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
        mut current_guess: ArrayViewMut1<f64>,
    ) {
        let last_guess = match state.latest() {
            Some(guess) => guess,
            None => return,
        };

        let diff_norm_sq: f64 = current_guess
            .iter()
            .zip(last_guess.iter())
            .fold(0.0, |acc, (curr, prev)| acc + (curr - prev).powi(2));

        let diff_norm = diff_norm_sq.sqrt();

        if diff_norm >= self.context.dc_damp_tolerance {
            for (curr, prev) in current_guess.iter_mut().zip(last_guess.iter()) {
                *curr = (*curr + *prev) * 0.5;
            }

            debug!(
                "Damping applied: Step norm {:.2e} > Tolerance {:.2e}",
                diff_norm, self.context.dc_damp_tolerance
            );
        }
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
            let mut sol = self.solver.solve(&mut self.system, 0.0, max_iter)?;
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
}

