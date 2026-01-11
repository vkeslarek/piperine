use crate::analysis::dc::DcAnalysisResult;
use crate::circuit::Circuit;
use crate::circuit::netlist::CircuitReference;
use crate::circuit::state::CircuitState;
use crate::math::faer::{FaerLinearSystem, FaerSymbolicMatrix};
use crate::math::linear::{LinearSystem, Stamp, SymbolicMatrix};
use crate::solver::Context;
use std::collections::HashMap;

pub struct DcSolver<'a> {
    // Borrow the circuit mutably for the duration of the solver's life
    circuit: &'a mut Circuit,
    context: Context,
    symbolic: FaerSymbolicMatrix<CircuitReference>,
    state: CircuitState<f64>,
}

impl<'a> DcSolver<'a> {
    pub fn build(circuit: &'a mut Circuit, context: Context) -> crate::result::Result<Self> {
        // 1. Determine dependent symbols
        let symbols = circuit
            .netlist()
            .all_references()
            .into_iter()
            .filter(|s| s.is_dependent())
            .collect();

        // 2. Determine Matrix Structure via dummy linearization
        // We use a temporary state just to map out the non-zero entries (stamps)
        let mut temp_state = CircuitState::new(HashMap::new(), HashMap::new(), 0);
        let stamps = Self::linearize(circuit, &mut temp_state, &context)?;
        let symbolic = FaerSymbolicMatrix::new(symbols, stamps)?;

        // 3. Initialize Solver State and apply initial conditions
        let mut state = CircuitState::new(symbolic.mapping.clone(), HashMap::new(), 2);
        for comp in circuit.components_mut().values_mut() {
            if let Some(dc) = comp.as_dc() {
                state.apply_initial_conditions(dc.initial_dc_values(&context));
            }
        }

        Ok(Self {
            circuit,
            context,
            symbolic,
            state,
        })
    }

    pub fn solve(&mut self) -> crate::result::Result<DcAnalysisResult> {
        for _ in 0..self.context.max_iter {
            // Linearize around current state
            let stamps = Self::linearize(self.circuit, &mut self.state, &self.context)?;
            let solution = self.solve_linear_system(stamps)?;

            // Check convergence
            let converged = self.context.has_converged(
                self.state.get_dependent_column(0),
                solution.view(),
                &self.symbolic.mapping,
            );

            // Update current guess in state
            self.state
                .get_current_dependent_column()
                .assign(&solution.view());

            if converged {
                return Ok(DcAnalysisResult {
                    values: solution,
                    mapping: self.symbolic.mapping().clone(),
                });
            }
        }

        Err(crate::error::Error::simple(
            "DC Solver did not converge",
            "Max iterations reached",
        ))
    }

    fn solve_linear_system(
        &self,
        stamps: Vec<Stamp<CircuitReference, f64>>,
    ) -> crate::result::Result<ndarray::Array1<f64>> {
        let mut system = FaerLinearSystem::new(self.symbolic.size());
        system.apply_stamps(&self.symbolic, stamps);
        system.solve_with_backend(&self.symbolic)
    }

    fn linearize(
        circuit: &mut Circuit,
        state: &mut CircuitState<f64>,
        ctx: &Context,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, f64>>> {
        let mut stamps = Vec::new();
        for (name, comp) in circuit.components_mut() {
            let dc = comp.as_dc().ok_or_else(|| {
                crate::error::Error::simple(
                    format!("Component '{}' invalid for DC", name),
                    "Missing DC impl",
                )
            })?;

            dc.update_dc(state, ctx)?;
            stamps.extend(
                dc.load_dc(state, ctx)
                    .into_iter()
                    .filter(|s| !s.has_ground_node()),
            );
        }
        Ok(stamps)
    }
}
