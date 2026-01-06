use crate::analysis::transient::{TransientAnalysis, TransientAnalysisContext};
use crate::devices::resistor::Resistor;
use crate::math::linear::Stamp;
use crate::netlist::CircuitReference;
use crate::solver::Context;
use crate::state::CircuitState;

impl TransientAnalysis for Resistor {
    fn load_transient(
        &self,
        _: &CircuitState<f64>,
        _: &TransientAnalysisContext,
        context: &Context,
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
