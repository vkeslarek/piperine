use crate::analysis::transient::{TransientAnalysis, TransientAnalysisContext};
use crate::circuit::netlist::CircuitReference;
use crate::circuit::state::CircuitState;
use crate::devices::resistor::Resistor;
use crate::math::linear::Stamp;
use crate::solver::Context;

impl TransientAnalysis for Resistor {
    fn update_transient(
        &mut self,
        _: &CircuitState<f64>,
        _: &TransientAnalysisContext,
        context: &Context,
    ) -> crate::result::Result<()> {
        self.model.clone().update_conductance(self, context);
        Ok(())
    }

    fn load_transient(
        &self,
        _: &CircuitState<f64>,
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
