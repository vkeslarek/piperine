use crate::analysis::transient::{
    TransientAnalysis, TransientAnalysisContext, TransientCircuitState,
};
use crate::devices::resistor::Resistor;
use crate::math::linear::Stamp;
use crate::circuit::netlist::CircuitReference;
use crate::solver::Context;

impl TransientAnalysis for Resistor {
    fn update_transient(
        &mut self,
        _: &TransientCircuitState,
        _: &TransientAnalysisContext,
        context: &Context,
    ) -> crate::result::Result<()> {
        self.model.clone().update_conductance(self, context);
        Ok(())
    }

    fn load_transient(
        &self,
        _: &TransientCircuitState,
        _: &TransientAnalysisContext,
        _: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        vec![
            Stamp::Matrix(
                self.node_plus.clone(),
                self.node_plus.clone(),
                self.conductance.value,
            ),
            Stamp::Matrix(
                self.node_minus.clone(),
                self.node_minus.clone(),
                self.conductance.value,
            ),
            Stamp::Matrix(
                self.node_plus.clone(),
                self.node_minus.clone(),
                -self.conductance.value,
            ),
            Stamp::Matrix(
                self.node_minus.clone(),
                self.node_plus.clone(),
                -self.conductance.value,
            ),
        ]
    }
}
