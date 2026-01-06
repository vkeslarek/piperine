use crate::analysis::dc::DcAnalysis;
use crate::devices::capacitor::Capacitor;
use crate::math::linear::Stamp;
use crate::netlist::CircuitReference;
use crate::solver::Context;

impl DcAnalysis for Capacitor {
    fn load_dc(&self, _: &Context) -> Vec<Stamp<CircuitReference, f64>> {
        vec![]
    }
}