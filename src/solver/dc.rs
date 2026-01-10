use crate::analysis::dc::{DcAnalysis, DcAnalysisResult, DcCircuitState, DcSolver};
use crate::circuit::Circuit;
use crate::error::Error;
use crate::math::faer::{FaerLinearSystem, FaerSymbolicMatrix};
use crate::math::linear::{Stamp, LinearSystem, SymbolicMatrix};
use crate::circuit::netlist::CircuitReference;
use crate::solver::Context;

pub struct DcSolverImpl {
    circuit: Circuit,
    context: Context,
    symbolic_matrix: FaerSymbolicMatrix<CircuitReference>,
    state: DcCircuitState,
}

impl DcSolver for DcSolverImpl {
    fn build(mut circuit: Circuit, context: Context) -> crate::result::Result<DcSolverImpl> {
        let symbols = Self::get_active_symbols(&circuit);

        let mut zero_state = DcCircuitState::new(std::collections::HashMap::new(), 0, 2);
        let stamps = Self::linearize_circuit(&mut circuit, &mut zero_state, &context)?;

        let symbolic_matrix = FaerSymbolicMatrix::new(symbols, stamps)?;

        let state = DcCircuitState::new(symbolic_matrix.mapping.clone(), symbolic_matrix.size, 2);

        Ok(Self {
            circuit,
            context,
            symbolic_matrix,
            state,
        })
    }

    fn solve(&mut self) -> crate::result::Result<DcAnalysisResult> {
        for iteration in 0..self.context.max_iter {
            let stamps =
                Self::linearize_circuit(&mut self.circuit, &mut self.state, &self.context)?;

            let mut linear_system: FaerLinearSystem<CircuitReference, f64> =
                FaerLinearSystem::new(self.symbolic_matrix.size());
            linear_system.apply_stamps(&self.symbolic_matrix, stamps);

            let new_values = linear_system.solve_with_backend(&self.symbolic_matrix)?;

            let converged = self.state.check_convergence(
                &new_values,
                self.context.reltol,
                self.context.vntol,
                self.context.abstol,
            );

            println!("ITERATION: {}", iteration);
            println!("NEWV: {:?}", new_values);
            println!("ABS: {:?}", self.state.get_diff(&new_values));

            if converged {
                println!("Solved in {} iterations", iteration);
                return Ok(DcAnalysisResult {
                    values: new_values,
                    mapping: self.symbolic_matrix.mapping().clone(),
                });
            }

            self.state.update_guess(new_values);
        }

        Err(Error::simple(
            "Convergence Failure",
            "Newton-Raphson loop exceeded max iterations without converging.",
        ))
    }
}

impl DcSolverImpl {
    fn linearize_circuit(
        circuit: &mut Circuit,
        state: &mut DcCircuitState,
        context: &Context,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, f64>>> {
        let mut stamps = Vec::new();

        for (name, component_box) in circuit.components_mut().iter_mut() {
            let component = component_box.as_dc().ok_or_else(|| {
                Error::simple(
                    format!("Component '{}' invalid for DC", name),
                    "Component does not implement DcAnalysis.",
                )
            })?;

            component.update_dc(state, context)?;

            let new_stamps = component.load_dc(state, context);

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
}
