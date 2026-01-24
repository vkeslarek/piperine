use crate::analysis::dc::{DcAnalysis, DcAnalysisState};
use crate::circuit::netlist::{CircuitReference, CircuitVariable};
use crate::devices::voltage_source::{VoltageSource, Waveform};
use crate::math::linear::{Stamp, Stamp2};
use crate::math::unit::UnitExt;
use crate::solver::Context;

impl DcAnalysis for VoltageSource {
    fn load_dc(&self, _: &DcAnalysisState, _: &Context) -> Vec<Stamp2<CircuitReference, f64>> {
        let dc_value = match self.waveform {
            Waveform::DC(v) => v,
            Waveform::Sine { amplitude: _, .. } => 0.0.V(),
            Waveform::Step { initial, delay, .. } => {
                if delay > 0.0 {
                    initial
                } else {
                    0.0.V()
                }
            }
        };

        vec![
            Stamp2::Matrix(self.branch.clone(), self.node_plus.clone(), 1.0),
            Stamp2::Matrix(self.branch.clone(), self.node_minus.clone(), -1.0),
            Stamp2::Matrix(self.node_plus.clone(), self.branch.clone(), 1.0),
            Stamp2::Matrix(self.node_minus.clone(), self.branch.clone(), -1.0),
            Stamp2::Rhs(self.branch.clone(), dc_value),
        ]
    }
}
