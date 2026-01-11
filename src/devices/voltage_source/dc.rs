use crate::analysis::dc::DcAnalysis;
use crate::circuit::netlist::CircuitReference;
use crate::circuit::state::CircuitState;
use crate::devices::voltage_source::VoltageSource;
use crate::math::linear::Stamp;
use crate::solver::Context;

impl DcAnalysis for VoltageSource {
    fn load_dc(&self, _: &CircuitState<f64>, _: &Context) -> Vec<Stamp<CircuitReference, f64>> {
        vec![
            Stamp::Matrix(self.branch.clone(), self.node_plus.clone(), 1.0),
            Stamp::Matrix(self.branch.clone(), self.node_minus.clone(), -1.0),
            Stamp::Matrix(self.node_plus.clone(), self.branch.clone(), 1.0),
            Stamp::Matrix(self.node_minus.clone(), self.branch.clone(), -1.0),
            Stamp::Rhs(self.branch.clone(), self.waveform.dc_value().value),
        ]
    }
}
