use crate::analysis::dc::DcAnalysis;
use crate::devices::inductor::Inductor;
use crate::math::linear::Stamp;
use crate::netlist::CircuitReference;
use crate::solver::Context;

impl DcAnalysis for Inductor {
    fn load_dc(&self, _: &Context) -> Vec<Stamp<CircuitReference, f64>> {
        vec![
            Stamp::Matrix(self.node_plus.clone(), self.branch.clone(), 1.0),
            Stamp::Matrix(self.node_minus.clone(), self.branch.clone(), -1.0),
            Stamp::Matrix(self.branch.clone(), self.node_plus.clone(), 1.0),
            Stamp::Matrix(self.branch.clone(), self.node_minus.clone(), -1.0),
            Stamp::Rhs(self.branch.clone(), 0.0),
        ]
    }
}