use crate::analysis::dc::{DcAnalysis, DcCircuitState};
use crate::devices::capacitor::Capacitor;
use crate::math::linear::Stamp;
use crate::netlist::CircuitReference;
use crate::solver::Context;

impl DcAnalysis for Capacitor {
    fn load_dc(&self, _: &DcCircuitState, context: &Context) -> Vec<Stamp<CircuitReference, f64>> {
        vec![]
    }
}
