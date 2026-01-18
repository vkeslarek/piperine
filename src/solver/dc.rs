use crate::analysis::dc::{DcAnalysisResult, DcAnalysisState};
use crate::circuit::Circuit;
use crate::circuit::netlist::CircuitReference;
use crate::map;
use crate::math::array::IndexedArray1;
use crate::math::iv::InitialValue;
use crate::math::linear::SparseLinearSystem;
use crate::math::linear::Stamp;
use crate::math::newton_raphson::{NewtonRaphsonSolver, NewtonRaphsonStamper};
use crate::solver::Context;
use ndarray::ArrayView1;

pub struct DcAnalysisStamper<'a> {
    pub circuit: &'a mut Circuit,
}

impl<'a> NewtonRaphsonStamper<CircuitReference, f64> for DcAnalysisStamper<'a> {
    fn static_stamps(
        &mut self,
        state: &DcAnalysisState,
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
        _state: &DcAnalysisState,
        _context: &Context,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, f64>>> {
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

    fn independent_symbols(&self) -> Vec<CircuitReference> {
        vec![CircuitReference::Iteration]
    }

    fn converged(
        &self,
        state: &DcAnalysisState,
        solution: &ArrayView1<f64>,
        context: &Context,
    ) -> bool {
        context.has_converged(&state.latest().unwrap().values, solution, &state.mapping)
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
        let solution = self.solver.step_steady_state(
            &mut self.linearizer,
            &map![CircuitReference::Iteration => 0.0],
        )?;

        Ok(DcAnalysisResult {
            values: solution,
            soa_violations: vec![],
        })
    }
}
