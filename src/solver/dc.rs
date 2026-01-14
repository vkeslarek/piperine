use crate::analysis::dc::DcAnalysisResult;
use crate::circuit::Circuit;
use crate::circuit::netlist::{CircuitReference, IndependentVariable};
use crate::math::Stamp;
use crate::math::linear::SparseLinearSystem;
use crate::math::newton_raphson::{NewtonRaphsonSolver, NewtonRaphsonStamper, SolverState};
use crate::math::vector::{InitialValue, SymbolicVector1};
use crate::solver::Context;
use ndarray::{Array1, ArrayView1};

pub struct DcAnalysisStamper<'a> {
    pub circuit: &'a mut Circuit,
}

impl<'a> NewtonRaphsonStamper<CircuitReference, f64> for DcAnalysisStamper<'a> {
    fn static_stamps(
        &mut self,
        state: &SolverState<CircuitReference, f64>,
        context: &Context,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, f64>>> {
        let mut stamps = Vec::new();
        for (name, comp) in self.circuit.components_mut() {
            let dc = comp.as_dc().ok_or_else(|| {
                crate::error::Error::simple(
                    format!("Component '{}' invalid for DC", name),
                    "Missing DC impl",
                )
            })?;

            // DC physics: Update linearization and collect stamps
            dc.update_dc(state, context)?;
            stamps.extend(
                dc.load_dc(state, context)
                    .into_iter()
                    .filter(|s| !s.has_ground_node()),
            );
        }
        Ok(stamps)
    }

    fn dynamic_stamps(
        &mut self,
        _state: &SolverState<CircuitReference, f64>,
        _context: &Context,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, f64>>> {
        // In DC steady state, capacitors are open and inductors are shorts.
        // These are handled by the component's DC implementation or ignored.
        Ok(Vec::new())
    }

    fn initial_conditions(
        &mut self,
        context: &Context,
    ) -> crate::result::Result<Vec<InitialValue<CircuitReference, f64>>> {
        Ok(self
            .circuit
            .components_mut()
            .values_mut()
            .filter_map(|c| c.as_dc())
            .flat_map(|dc| dc.initial_dc_values(context))
            .collect())
    }

    fn active_symbols(&self) -> Vec<CircuitReference> {
        self.circuit
            .netlist()
            .all_references()
            .into_iter()
            .filter(|s| s.is_dependent())
            .collect()
    }

    fn independent_symbols(&self) -> Vec<IndependentVariable> {
        // DC doesn't strictly depend on Time, but we provide an empty set
        // or a dummy if the generic solver requires one.
        Vec::new()
    }

    fn converged(
        &self,
        state: &SolverState<CircuitReference, f64>,
        solution: &ArrayView1<f64>,
        context: &Context,
    ) -> bool {
        context.has_converged(
            &state.get_dependent_column(0),
            solution,
            &state.solver_mapping,
        )
    }
}

pub struct DcSolver<'a> {
    pub linearizer: DcAnalysisStamper<'a>,
    pub solver: NewtonRaphsonSolver<CircuitReference, f64>,
}

impl<'a> DcSolver<'a> {
    pub fn new(circuit: &'a mut Circuit, context: Context) -> crate::result::Result<Self> {
        let mut linearizer = DcAnalysisStamper { circuit };

        // NewtonRaphsonSolver::create handles symbolic analysis and IC application
        let solver = NewtonRaphsonSolver::create(&mut linearizer, context)?;

        Ok(Self { linearizer, solver })
    }

    pub fn solve(&mut self) -> crate::result::Result<DcAnalysisResult> {
        // We use a dummy independent variable because DC is static
        let dummy_vars = Array1::zeros(0);
        let dummy_var = IndependentVariable::Time;

        let solution = self
            .solver
            .step(&mut self.linearizer, &dummy_vars.view(), &dummy_var)?;

        Ok(DcAnalysisResult {
            values: SymbolicVector1::from_values(
                solution,
                self.solver.symbolic_matrix.mapping.clone(),
            ),
            soa_violations: vec![],
        })
    }
}
