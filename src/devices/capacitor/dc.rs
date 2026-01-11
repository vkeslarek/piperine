use crate::analysis::dc::DcAnalysis;
use crate::circuit::netlist::CircuitReference;
use crate::circuit::state::CircuitState;
use crate::devices::capacitor::Capacitor;
use crate::math::linear::Stamp;
use crate::solver::Context;

impl DcAnalysis for Capacitor {
    fn load_dc(
        &self,
        _: &CircuitState<f64>,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        vec![]
    }
}
