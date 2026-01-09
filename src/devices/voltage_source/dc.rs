use crate::analysis::dc::{DcAnalysis, DcCircuitState};
use crate::devices::voltage_source::VoltageSource;
use crate::math::linear::Stamp;
use crate::netlist::CircuitReference;
use crate::solver::Context;

impl DcAnalysis for VoltageSource {
    fn load_dc(&self, _: &DcCircuitState, _: &Context) -> Vec<Stamp<CircuitReference, f64>> {
        vec![
            Stamp::Matrix(self.branch.clone(), self.node_plus.clone(), 1.0),
            Stamp::Matrix(self.branch.clone(), self.node_minus.clone(), -1.0),
            Stamp::Matrix(self.node_plus.clone(), self.branch.clone(), 1.0),
            Stamp::Matrix(self.node_minus.clone(), self.branch.clone(), -1.0),
            Stamp::Rhs(self.branch.clone(), self.voltage.value),
        ]
    }
}
