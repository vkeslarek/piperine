use crate::component::{Component, Context};
use crate::math::unit::Conductance;
use crate::solver::Stamp;

pub trait DcAnalysis: Component {
    fn load_dc(&self, context: &Context) -> Vec<Stamp<Conductance>>;
}
