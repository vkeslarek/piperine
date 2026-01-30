use crate::analysis::dc::{DcAnalysis, DcAnalysisState};
use crate::circuit::netlist::CircuitReference;
use crate::devices::inductor::Inductor;
use crate::math::linear::Stamp;
use crate::solver::Context;

impl DcAnalysis for Inductor {
    fn load_dc(&self, _: &DcAnalysisState, _: &Context) -> Vec<Stamp<CircuitReference, f64>> {
        vec![
            Stamp::Matrix(self.current_ref.clone(), self.node_plus.clone(), 1.0),
            Stamp::Matrix(self.current_ref.clone(), self.node_minus.clone(), -1.0),
            Stamp::Matrix(self.node_plus.clone(), self.current_ref.clone(), 1.0),
            Stamp::Matrix(self.node_minus.clone(), self.current_ref.clone(), -1.0),
        ]
    }
}
