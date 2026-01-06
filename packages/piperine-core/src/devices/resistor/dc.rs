use crate::analysis::dc::DcAnalysis;
use crate::devices::resistor::Resistor;
use crate::math::linear::Stamp;
use crate::netlist::CircuitReference;
use crate::solver::Context;

impl DcAnalysis for Resistor {
    fn load_dc(&self, context: &Context) -> Vec<Stamp<CircuitReference, f64>> {
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