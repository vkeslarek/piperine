use crate::analysis::dc::{DcAnalysis, DcAnalysisState};
use crate::circuit::netlist::CircuitReference;
use crate::devices::capacitor::Capacitor;
use crate::math::linear::Stamp2;
use crate::solver::Context;

impl DcAnalysis for Capacitor {
    fn load_dc(&self, _: &DcAnalysisState, context: &Context) -> Vec<Stamp2<CircuitReference, f64>> {
        vec![
            Stamp2::Matrix(self.node_plus.clone(), self.node_plus.clone(), context.gmin),
            Stamp2::Matrix(
                self.node_minus.clone(),
                self.node_minus.clone(),
                context.gmin,
            ),
            Stamp2::Matrix(
                self.node_plus.clone(),
                self.node_minus.clone(),
                -context.gmin,
            ),
            Stamp2::Matrix(
                self.node_minus.clone(),
                self.node_plus.clone(),
                -context.gmin,
            ),
        ]
    }
}
