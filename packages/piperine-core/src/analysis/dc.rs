use crate::component::Component;
use crate::math::linear::Stamp;
use crate::netlist::CircuitReference;
use crate::solver::Context;

pub trait DcAnalysis: Component {
    fn update_dc(&mut self, context: &Context) -> crate::error::Result<()> {
        Ok(())
    }
    fn load_dc(&self, context: &Context) -> Vec<Stamp<CircuitReference, f64>>;
}
