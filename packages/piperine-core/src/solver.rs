use crate::analysis::transient::TransientAnalysisContext;
use crate::circuit::Circuit;
use crate::error::ErrorDetail;
use crate::math::linear::{LinearSystem, Stamp, SymbolicMatrix};
use crate::math::unit::{Conductance, UnitExt};
use crate::netlist::CircuitReference;
use crate::state::CircuitState;
use std::collections::HashMap;

pub struct Context {
    pub gmin: Conductance,
    pub reltol: f64,
    pub abstol: f64,
    pub vntol: f64,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            gmin: 1.0.pS(),
            reltol: 1e-3,
            abstol: 1e-12,
            vntol: 1e-6,
        }
    }
}

pub struct Solver {
    circuit: Circuit,
    context: Context,
    symbolic: SymbolicMatrix<CircuitReference>,
}

impl Solver {
    pub fn build(mut circuit: Circuit, context: Context) -> crate::error::Result<Self> {
        // 1. Gather ALL possible symbols (nodes + branches)
        let symbols = circuit.netlist().all_references();

        let mut stamps = circuit.load_dc(&context);

        let dummy_state = CircuitState::new(HashMap::new(), HashMap::new(), 1);
        let dummy_tran = TransientAnalysisContext { time: 0.0, dt: 1.0 };
        stamps.extend(circuit.load_transient(&dummy_state, &dummy_tran, &context));

        Ok(Self {
            circuit,
            context,
            symbolic: SymbolicMatrix::new(symbols, stamps)?,
        })
    }

    /// Newton-Raphson Engine: The heart of the solver.
    /// Used by both OP and Transient analysis to find convergence for non-linearities.
    fn solve_nr(
        &mut self,
        circuit_state: &mut CircuitState<f64>,
        time_ctx: &TransientAnalysisContext,
    ) -> crate::error::Result<()> {
        let max_iters = 100;

        for i in 0..max_iters {
            // 1. Prepare stamps based on current guess in circuit_state
            let stamps = if time_ctx.dt == 0.0 {
                self.circuit.update_dc(&self.context)?;
                self.circuit.update()?;
                self.circuit.load_dc(&self.context)
            } else {
                self.circuit
                    .update_transient(circuit_state, time_ctx, &self.context)?;
                self.circuit
                    .load_transient(circuit_state, time_ctx, &self.context)
            };

            // 2. Solve the linear system
            let mut system = LinearSystem::new(self.symbolic.size());
            system.apply_stamps(&self.symbolic, stamps);
            let next_solution = system.solve_with_backend(&self.symbolic)?;

            // 3. Update the guess in the state
            circuit_state.push_guess(time_ctx.time, next_solution);

            // 4. Check Convergence
            if self
                .circuit
                .check_convergence(circuit_state, time_ctx, &self.context)?
            {
                circuit_state.commit_guess();
                return Ok(());
            }
        }

        Err(ErrorDetail::simple(
            "Convergence Failure",
            "NR failed to converge.",
        ))
    }

    /// Operating Point: Solves at t=0 with dt=0 (no reactive effects)
    pub fn op(&mut self) -> crate::error::Result<HashMap<CircuitReference, f64>> {
        // Create an empty state for DC
        let mut state = CircuitState::new(HashMap::new(), HashMap::new(), 1);
        let dc_ctx = TransientAnalysisContext { time: 0.0, dt: 0.0 };

        // Seed the first guess with zeros if empty
        state.push_guess(0.0, HashMap::new());

        self.solve_nr(&mut state, &dc_ctx)?;

        Ok(state.get_last_vector().clone())
    }

    /// Transient: Steps through time, running NR at every step.
    pub fn transient(
        &mut self,
        trans_ctx: TransientAnalysisContext,
    ) -> crate::error::Result<Vec<(f64, HashMap<CircuitReference, f64>)>> {
        let mut history = Vec::new();

        // 1. Initial Operating Point
        let mut circuit_state = CircuitState::new(self.op()?, HashMap::new(), 5);
        history.push((0.0, circuit_state.get_last_vector().clone()));

        let mut t = trans_ctx.dt;
        while t <= trans_ctx.time {
            let step_ctx = TransientAnalysisContext {
                time: t,
                dt: trans_ctx.dt,
            };

            // Use the previous solution as the starting guess for the new time step
            let last_val = circuit_state.get_last_vector().clone();
            circuit_state.push_guess(t, last_val);

            // 2. Converge non-linearities for this time step
            self.solve_nr(&mut circuit_state, &step_ctx)?;

            history.push((t, circuit_state.get_last_vector().clone()));
            t += trans_ctx.dt;
        }

        Ok(history)
    }
    //
    // pub fn solve_ac(
    //     &self,
    //     dc_state: &CircuitStates,
    //     omega: f64,
    //     context: &Context,
    // ) -> crate::error::Result<Vec<Complex<f64>>> {
    //     let ac_analysis_context = AcAnalysisContext { frequency: omega };
    //     let mut system = LinearSystem::new(self.symbolic.size);
    //
    //     // 1. Collect AC stamps from all components
    //     // Note: You may need to cast your components to a trait that supports load_ac
    //     let stamps: Vec<Stamp<Complex<f64>>> = self
    //         .circuit
    //         .components
    //         .get_all()
    //         .iter()
    //         .flat_map(|comp| {
    //             // Assuming components implement an AcAnalysis trait you defined
    //             if let Some(ac_interface) = comp.as_ac() {
    //                 ac_interface.load_ac(dc_state, &ac_analysis_context, context)
    //             } else {
    //                 vec![]
    //             }
    //         })
    //         .collect();
    //
    //     // 2. Apply and Solve
    //     system.apply_stamps(&self.symbolic, stamps);
    //     system.solve_with_backend(&self.symbolic)
    // }
    //
    // pub fn solve_ac_sweep(
    //     &mut self,
    //     start_freq: f64,
    //     stop_freq: f64,
    //     steps: usize,
    //     logarithmic: bool,
    //     context: &Context,
    // ) -> crate::error::Result<Vec<(f64, Vec<Complex<f64>>)>> {
    //     // 1. Always find the DC operating point first
    //     let dc_state = self.solve_dc(context)?;
    //
    //     let mut results = Vec::with_capacity(steps);
    //
    //     for i in 0..steps {
    //         let freq = if logarithmic {
    //             let log_start = start_freq.log10();
    //             let log_stop = stop_freq.log10();
    //             10.0f64.powf(log_start + (log_stop - log_start) * (i as f64 / (steps - 1) as f64))
    //         } else {
    //             start_freq + (stop_freq - start_freq) * (i as f64 / (steps - 1) as f64)
    //         };
    //
    //         let omega = 2.0 * std::f64::consts::PI * freq;
    //         let solution = self.solve_ac(&dc_state, omega, context)?;
    //         results.push((freq, solution));
    //     }
    //
    //     Ok(results)
    // }
    //
    // pub fn solve_dc(&mut self, context: &Context) -> crate::error::Result<CircuitStates> {
    //     let mut state = CircuitStates::new(self.symbolic.mapping.clone(), 2);
    //     let transient_analysis_context = TransientAnalysisContext { time: 0.0, dt: 0.0 };
    //
    //     self.solve_nr(&mut state, &transient_analysis_context, &context)?;
    //     Ok(state)
    // }
    //
    // pub fn dc_stamps(
    //     &mut self,
    //     context: Context,
    // ) -> crate::error::Result<Vec<Stamp<CircuitReference, f64>>> {
    //     self.update_components(self, context)
    // }
    //
    // pub fn transient(&mut self, stop_time: f64, dt: f64) -> crate::error::Result<Vec<Vec<f64>>> {
    //     // Initiate at rest
    //     let mut state = CircuitState::new(self.symbolic.mapping.clone(), 2);
    //     state.push_commited(vec![0.0; self.symbolic.size], 0.0);
    //     let mut all_states = Vec::new();
    //
    //     let mut current_time = dt;
    //
    //     while current_time <= stop_time {
    //         let transient_analysis_context = TransientAnalysisContext {
    //             time: current_time,
    //             dt,
    //         };
    //
    //         // Run NR for this specific time step
    //         self.solve_nr(&mut state, &transient_analysis_context, &context)?;
    //         all_states.push(state.history.values.get(0).unwrap().clone());
    //
    //         current_time += dt;
    //     }
    //
    //     println!("{:?}", state);
    //
    //     Ok(all_states)
    // }
    //
    // fn solve_nr(
    //     &mut self,
    //     state: &mut CircuitStates<TimedCircuitState<f64>, f64>,
    //     transient_analysis_context: &TransientAnalysisContext,
    //     context: &Context,
    // ) -> crate::error::Result<()> {
    //     let max_iterations = 1000;
    //
    //     state.new_guess();
    //     let current_guess = state.history.values.get(0).cloned().unwrap();
    //
    //     // Insert the guess into the buffer so components can read it at lookback 0
    //     state.history.values.push_front(current_guess);
    //
    //     for i in 0..max_iterations {
    //         self.update_components(state, context)?;
    //
    //         let stamps = self.stamp_components(|c| {
    //             if transient_analysis_context.dt == 0.0 {
    //                 c.as_dc_mut().unwrap().load_dc(context)
    //             } else {
    //                 c.as_transient_mut().unwrap().load_transient(
    //                     state,
    //                     transient_analysis_context,
    //                     context,
    //                 )
    //             }
    //         });
    //
    //         let mut system = LinearSystem::new(self.symbolic.size);
    //         system.apply_stamps(&self.symbolic, stamps);
    //         let next_solution = system.solve_with_backend(&self.symbolic)?;
    //
    //         state.history.values.push_front(next_solution);
    //
    //         let converged = self.circuit.components.get_all().iter().all(|c| {
    //             c.as_transient().unwrap().check_convergence(
    //                 state,
    //                 transient_analysis_context,
    //                 context,
    //             )
    //         });
    //
    //         if converged {
    //             // Success! Remove the iteration history, keeping only the winner at index 0
    //             let winner = state.history.values.pop_front().unwrap();
    //             state.history.values.pop_front(); // Remove the old guess
    //             state.history.values.push_front(winner);
    //
    //             state.commit_step(transient_analysis_context.time, context.numerical_method);
    //             return Ok(());
    //         }
    //
    //         // 4. Not converged: Remove the "oldest" guess to keep buffer size managed for next iter
    //         // index 0 is the new solution (will be the guess for next iter)
    //         // index 1 is the guess we just used
    //         state.history.values.remove(1);
    //     }
    //
    //     Err(ErrorDetail {
    //         title: "Convergence Failure".to_string(),
    //         detail: format!(
    //             "Newton-Raphson failed to converge in {} iterations at t={}",
    //             max_iterations, transient_analysis_context.time
    //         ),
    //         problems: vec![],
    //     })
    // }
    //
    // fn update_components(
    //     &mut self,
    //     state: &mut CircuitStates,
    //     context: &Context,
    // ) -> crate::error::Result<()> {
    //     for comp in self.circuit.components_mut().components.values_mut() {
    //         comp.update(state, context)?;
    //     }
    //
    //     Ok(())
    // }
    //
    // fn stamp_components<F>(&self, mapper_fn: F) -> Vec<Stamp<f64>>
    // where
    //     F: Fn(&dyn Component) -> Vec<Stamp<f64>>,
    // {
    //     self.circuit
    //         .components
    //         .get_all()
    //         .iter()
    //         .flat_map(|c| mapper_fn(c.as_ref())) // c is Box<dyn Component>, as_ref() gets &dyn Component
    //         .collect()
    // }
}
