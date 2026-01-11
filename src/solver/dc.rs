use crate::analysis::dc::{DcAnalysis, DcAnalysisResult, DcCircuitState};
use crate::circuit::Circuit;
use crate::circuit::netlist::CircuitReference;
use crate::error::Error;
use crate::math::faer::{FaerLinearSystem, FaerSymbolicMatrix};
use crate::math::linear::{LinearSystem, Stamp, SymbolicMatrix};
use crate::solver::Context;
use std::collections::HashMap;

pub struct DcSolver {
    circuit: Circuit,
    context: Context,
    symbolic: FaerSymbolicMatrix<CircuitReference>,
    state: DcCircuitState,
}

impl DcSolver {
    pub fn build(mut circuit: Circuit, context: Context) -> crate::result::Result<Self> {
        // 1. Linearize once with dummy state to determine Matrix Structure (Sparsity)
        let symbols = circuit
            .netlist()
            .all_references()
            .into_iter()
            .filter(|s| !s.is_ground())
            .collect();

        let mut temp_state = DcCircuitState::new(HashMap::new(), 0, 1);
        let stamps = Self::linearize(&mut circuit, &mut temp_state, &context)?;

        let symbolic = FaerSymbolicMatrix::new(symbols, stamps)?;

        // 2. Initialize Real State & Apply Initial Conditions
        let mut state = DcCircuitState::new(symbolic.mapping.clone(), symbolic.size, 2);
        let mut guess = state.current_guess_mut();

        for (_, comp) in circuit.components_mut() {
            if let Some(dc) = comp.as_dc() {
                for init in dc.initial_dc_values(&context) {
                    if let Some(&idx) = symbolic.mapping.get(&init.reference) {
                        guess[idx] = init.value;
                    }
                }
            }
        }

        Ok(Self {
            circuit,
            context,
            symbolic: symbolic,
            state,
        })
    }

    pub fn solve(&mut self) -> crate::result::Result<DcAnalysisResult> {
        for _ in 0..self.context.max_iter {
            // 1. Physics: Linearize Circuit around current state
            let stamps = Self::linearize(&mut self.circuit, &mut self.state, &self.context)?;

            // 2. Math: Build & Solve Matrix
            let mut system = FaerLinearSystem::new(self.symbolic.size());
            system.apply_stamps(&self.symbolic, stamps);

            let solution = system.solve_with_backend(&self.symbolic)?;

            // 3. Convergence: Check Input (row 0) vs Output (solution)
            // We verify if the physics has settled before updating the history.
            let converged = self.context.has_converged(
                self.state.values.row(0),
                solution.view(),
                &self.symbolic.mapping,
            );

            // 4. Update History
            self.state.push(solution.view());

            if converged {
                return Ok(DcAnalysisResult {
                    values: solution,
                    mapping: self.symbolic.mapping().clone(),
                });
            }
        }

        Err(Error::simple(
            "DC Solver did not converge",
            "Maximum iterations reached without convergence",
        ))
    }

    fn linearize(
        circuit: &mut Circuit,
        state: &mut DcCircuitState,
        ctx: &Context,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, f64>>> {
        let mut stamps = Vec::new();

        for (name, comp) in circuit.components_mut() {
            let dc = comp.as_dc().ok_or_else(|| {
                Error::simple(
                    format!("Component '{}' invalid for DC", name),
                    "Missing DcAnalysis implementation",
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
