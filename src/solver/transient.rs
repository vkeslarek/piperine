use crate::analysis::transient::{
    TransientAnalysisContext, TransientAnalysisOptions, TransientAnalysisResult,
};
use crate::circuit::Circuit;
use crate::circuit::netlist::CircuitReference;
use crate::map;
use crate::math::array::IndexedArray2;
use crate::math::iv::InitialValue;
use crate::math::linear::Stamp;
use crate::math::newton_raphson::{NewtonRaphsonSolver, NewtonRaphsonStamper};
use crate::math::unit::UnitExt;
use crate::solver::Context;
use ndarray::ArrayView1;

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
        let mut result = TransientAnalysisResult::new(self.solver.state.mapping.clone());
        let mut t = 0.0;

        if let Some(initial_state) = self.solver.state.latest() {
            result.push_view(&initial_state);
        }

        t += self.options.dt;

        while t <= self.options.stop_time {
            let solution = self.solver.step_dynamic(
                &mut self.linearizer,
                &map![CircuitReference::Time => t],
                &CircuitReference::Time,
            )?;

            result.push(&solution);

            t += self.options.dt;
        }

        Ok(result)
    }
}

pub struct TransientAnalysisStamper<'a> {
    circuit: &'a mut Circuit,
}

impl<'a> TransientAnalysisStamper<'a> {
    fn get_context(
        &self,
        state: &IndexedArray2<CircuitReference, f64>,
    ) -> TransientAnalysisContext {
        let t0 = state
            .latest()
            .and_then(|val| val.get(&CircuitReference::Time).cloned())
            .unwrap_or(0.0);

        let t1 = state
            .view(1)
            .and_then(|v| v.get(&CircuitReference::Time).cloned())
            .unwrap_or(t0);

        TransientAnalysisContext {
            time: t0.s(),
            dt: (t0 - t1).s(),
        }
    }
}

impl<'a> NewtonRaphsonStamper<CircuitReference, f64> for TransientAnalysisStamper<'a> {
    fn static_stamps(
        &mut self,
        state: &IndexedArray2<CircuitReference, f64>,
        context: &Context,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, f64>>> {
        let tran_ctx = self.get_context(state);

        let mut stamps = Vec::new();
        for comp in self.circuit.components_mut().values_mut() {
            if let Some(t_comp) = comp.as_transient() {
                // Update physics state (e.g. diode conductance based on new V)
                t_comp.update_transient(state, &tran_ctx, context)?;

                // Collect Jacobian (G) and RHS (I) stamps
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
        state: &IndexedArray2<CircuitReference, f64>,
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

    fn independent_symbols(&self) -> Vec<CircuitReference> {
        vec![CircuitReference::Time]
    }

    fn converged(
        &self,
        state: &IndexedArray2<CircuitReference, f64>,
        solution: &ArrayView1<f64>,
        context: &Context,
    ) -> bool {
        context.has_converged(&state.latest().unwrap().values, solution, &state.mapping)
    }
}
