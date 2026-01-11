use crate::analysis::transient::{
    TransientAnalysisContext, TransientAnalysisOptions, TransientAnalysisResult,
};
use crate::circuit::Circuit;
use crate::circuit::netlist::{CircuitReference, IndependentVariable};
use crate::math::linear::{InitialValue, LinearSystem, Stamp};
use crate::math::newton_raphson::{SolverState, NewtonRaphsonSolver, NewtonRaphsonStamper};
use crate::math::unit::UnitExt;
use crate::solver::Context;
use ndarray::{Array1, ArrayView1};

pub struct TransientSolver<'a> {
    pub linearizer: TransientAnalysisStamper<'a>,
    pub options: TransientAnalysisOptions,
    pub solver: NewtonRaphsonSolver<CircuitReference, f64>,
}

impl<'a> TransientSolver<'a> {
    pub fn new(
        circuit: &'a mut Circuit,
        options: TransientAnalysisOptions,
        context: Context,
    ) -> crate::result::Result<Self> {
        let mut linearizer = TransientAnalysisStamper { circuit };
        let solver = NewtonRaphsonSolver::create(&mut linearizer, context)?;

        Ok(Self {
            linearizer,
            options,
            solver,
        })
    }

    pub fn solve(&mut self) -> crate::result::Result<TransientAnalysisResult> {
        let mut result = TransientAnalysisResult::new(self.solver.symbolic_matrix.mapping.clone());
        let mut t = 0.0;

        while t <= self.options.stop_time {
            // NewtonRaphsonSolver::step now handles the commit internally
            let solution = self.solver.step(
                &mut self.linearizer,
                &Array1::from_elem(1, t).view(),
                &IndependentVariable::Time,
            )?;

            result.push(t, solution.view());
            t += self.options.dt;
        }
        Ok(result)
    }
}

pub struct TransientAnalysisStamper<'a> {
    pub circuit: &'a mut Circuit,
}

impl<'a> TransientAnalysisStamper<'a> {
    /// Helper to extract timing info from state and build the context
    fn get_context(
        &self,
        state: &SolverState<CircuitReference, f64>,
    ) -> TransientAnalysisContext {
        let t0 = state
            .get_independent_value(&IndependentVariable::Time, 0)
            .unwrap_or(0.0);
        let t1 = state
            .get_independent_value(&IndependentVariable::Time, 1)
            .unwrap_or(t0);

        TransientAnalysisContext {
            time: t0.Sec(),
            dt: (t0 - t1).Sec(),
        }
    }
}

impl<'a> NewtonRaphsonStamper<CircuitReference, f64> for TransientAnalysisStamper<'a> {
    fn static_stamps(
        &mut self,
        state: &SolverState<CircuitReference, f64>,
        context: &Context,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, f64>>> {
        let tran_ctx = self.get_context(state);

        let mut stamps = Vec::new();
        for comp in self.circuit.components_mut().values_mut() {
            if let Some(t_comp) = comp.as_transient() {
                t_comp.update_transient(state, &tran_ctx, context)?;
                stamps.extend(
                    t_comp
                        .load_transient(state, &tran_ctx, context)
                        .into_iter()
                        .filter(|s| !s.has_ground_node()),
                );
            }
        }
        Ok(stamps)
    }

    fn dynamic_stamps(
        &mut self,
        state: &SolverState<CircuitReference, f64>,
        context: &Context,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, f64>>> {
        let tran_ctx = self.get_context(state);

        Ok(self
            .circuit
            .components_mut()
            .values_mut()
            .filter_map(|c| c.as_transient())
            .flat_map(|t| t.load_transient_dynamic(state, &tran_ctx, context))
            .filter(|s| !s.has_ground_node())
            .collect())
    }

    fn initial_conditions(
        &mut self,
        context: &Context,
    ) -> crate::result::Result<Vec<InitialValue<CircuitReference, f64>>> {
        Ok(self
            .circuit
            .components_mut()
            .values_mut()
            .filter_map(|c| c.as_transient())
            .flat_map(|t| t.initial_transient_values(context))
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
        vec![IndependentVariable::Time]
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
