use crate::analysis::transient::{
    TransientAnalysisOptions, TransientAnalysisResult, TransientCircuitState, TransientSolver,
};
use crate::circuit::Circuit;
use crate::error::Error;
use crate::math::faer::{FaerLinearSystem, FaerSymbolicMatrix};
use crate::math::linear::{Stamp, LinearSystem, SymbolicMatrix};
use crate::math::unit::{Second, Time};
use crate::circuit::netlist::CircuitReference;
use crate::solver::Context;
use faer::Col;

pub struct TransientSolverImpl {
    circuit: Circuit,
    context: Context,
    options: TransientAnalysisOptions,
    symbolic_matrix: FaerSymbolicMatrix<CircuitReference>,
    state: TransientCircuitState,
}

impl TransientSolver for TransientSolverImpl {
    fn build(
        mut circuit: Circuit,
        options: TransientAnalysisOptions,
        context: Context,
    ) -> crate::result::Result<Self> {
        let symbols = Self::get_active_symbols(&circuit);

        let mut zero_state = TransientCircuitState::new(std::collections::HashMap::new(), 0, 3);
        let stamps = Self::linearize_circuit(&mut circuit, &mut zero_state, &context, 0.0)?;

        let symbolic_matrix = FaerSymbolicMatrix::new(symbols, stamps)?;

        let state =
            TransientCircuitState::new(symbolic_matrix.mapping.clone(), symbolic_matrix.size, 3);

        Ok(Self {
            circuit,
            context,
            options,
            symbolic_matrix,
            state,
        })
    }

    fn solve(&mut self) -> crate::result::Result<TransientAnalysisResult> {
        let mut result = TransientAnalysisResult::new(self.symbolic_matrix.mapping.clone());

        println!("--- Starting Transient Analysis ---");

        self.solve_newton_raphson(0.0, 0.0)?;

        let initial_solution = self.state.values[0].clone();
        result.push(0.0, initial_solution.clone());

        self.state.push_new_step(self.options.dt);

        let mut current_time = self.options.dt;

        while current_time <= self.options.stop_time {
            self.solve_newton_raphson(current_time, self.options.dt)?;

            let converged_values = self.state.values[0].clone();
            result.push(current_time, converged_values.clone());

            current_time += self.options.dt;

            if current_time <= self.options.stop_time {
                self.state.push_new_step(current_time);
            }
        }

        Ok(result)
    }
}

impl TransientSolverImpl {
    /// The Newton-Raphson Solver Loop
    fn solve_newton_raphson(&mut self, time: f64, dt: f64) -> crate::result::Result<()> {
        for iteration in 0..self.context.max_iter {
            let stamps =
                Self::linearize_circuit(&mut self.circuit, &mut self.state, &self.context, dt)?;

            // 2. Build Linear System (Ax = b)
            let mut linear_system = FaerLinearSystem::new(self.symbolic_matrix.size());
            linear_system.apply_stamps(&self.symbolic_matrix, stamps);

            // 3. Solve for New Voltage Vector
            let new_values = linear_system.solve_with_backend(&self.symbolic_matrix)?;

            // 4. Check Convergence
            let converged = self.check_convergence_internal(
                &new_values,
                self.context.reltol,
                self.context.vntol,
                self.context.abstol,
            );

            // Update the current guess (values[0]) with the Newton-Raphson result
            self.state.update_guess(new_values);

            if converged {
                return Ok(());
            }
        }

        Err(Error::simple(
            "Convergence Failure",
            format!(
                "Transient step at t={} failed to converge after {} iterations.",
                time, self.context.max_iter
            ),
        ))
    }

    /// Helper to iterate over all components and collect their stamps
    fn linearize_circuit(
        circuit: &mut Circuit,
        state: &mut TransientCircuitState,
        context: &Context,
        dt: f64,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, f64>>> {
        let mut stamps = Vec::new();

        // Note: Creating a temporary context for the component.
        // Ideally, you would update your Context struct to carry 'time' and 'dt'.
        // For now, we assume components use the state history for time info.

        // Update state.current_dt for components to access if needed
        state.current_dt = dt;

        for (name, component_box) in circuit.components_mut().iter_mut() {
            // Assuming your components implement a trait that accepts TransientCircuitState
            // You likely need to define `as_transient()` on your Component trait
            // For now, we cast assuming the component handles the specific state type

            // NOTE: You will need to ensure your Component trait exposes a method
            // that accepts `TransientCircuitState`.
            // Here I assume `load_transient` is available or bridged.

            /* WARNING: The code below assumes you have updated the Component trait
               to support `load_transient`. If not, you must bridge it here.
            */

            let component = component_box.as_transient().ok_or_else(|| {
                Error::simple(
                    format!("Component '{}' invalid", name),
                    "Component does not implement TransientAnalysis.",
                )
            })?;

            // Use dummy context for now if your signature requires it
            let trans_ctx = crate::analysis::transient::TransientAnalysisContext {
                time: Time::new::<Second>(state.timestamps[0]),
                dt: Time::new::<Second>(dt),
            };

            // Call the component's update logic (calculating G_eq, I_eq)
            component.update_transient(state, &trans_ctx, context)?;

            // Get the MNA stamps
            let new_stamps = component.load_transient(state, &trans_ctx, context);

            stamps.extend(new_stamps.into_iter().filter(|s| !s.has_ground_node()));
        }

        Ok(stamps)
    }

    fn get_active_symbols(circuit: &Circuit) -> Vec<CircuitReference> {
        circuit
            .netlist()
            .all_references()
            .into_iter()
            .filter(|s| !s.is_ground())
            .collect()
    }

    /// Re-implementation of convergence check since TransientCircuitState
    /// didn't implement it in your provided snippet.
    fn check_convergence_internal(
        &self,
        new_values: &Col<f64>,
        reltol: f64,
        vntol: f64,
        abstol: f64,
    ) -> bool {
        let old_values = &self.state.values[0];

        for i in 0..self.symbolic_matrix.size {
            let old_v = old_values[i];
            let new_v = new_values[i];
            let diff = (new_v - old_v).abs();

            // Determine if index is branch or node (simple check via mapping usually needed)
            // Defaulting to vntol for simplicity unless mapped
            let limit = reltol * old_v.abs().max(new_v.abs()) + vntol;

            if diff > limit {
                return false;
            }
        }
        true
    }
}
