use crate::analysis::transient::{
    TransientAnalysis, TransientAnalysisContext, TransientAnalysisState,
};
use crate::circuit::netlist::CircuitReference;
use crate::devices::resistor::Resistor;
use crate::math::linear::Stamp2;
use crate::solver::Context;

impl TransientAnalysis for Resistor {
    fn update_transient(
        &mut self,
        _: &TransientAnalysisState,
        _: &TransientAnalysisContext,
        context: &Context,
    ) -> crate::result::Result<()> {
        self.model.clone().update_conductance(self, context);
        Ok(())
    }

    fn load_transient(
        &self,
        _: &TransientAnalysisState,
        _: &TransientAnalysisContext,
        _: &Context,
    ) -> Vec<Stamp2<CircuitReference, f64>> {
        vec![
            Stamp2::Matrix(
                self.node_plus.clone(),
                self.node_plus.clone(),
                self.conductance,
            ),
            Stamp2::Matrix(
                self.node_minus.clone(),
                self.node_minus.clone(),
                self.conductance,
            ),
            Stamp2::Matrix(
                self.node_plus.clone(),
                self.node_minus.clone(),
                -self.conductance,
            ),
            Stamp2::Matrix(
                self.node_minus.clone(),
                self.node_plus.clone(),
                -self.conductance,
            ),
        ]
    }
}
