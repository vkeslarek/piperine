use crate::component::vsrc::VoltageSource;
use crate::model::Model;

pub type VoltageSourceModel = dyn Model<ComponentType = VoltageSource>;

pub struct VoltageSourceIdealModel {}

impl VoltageSourceIdealModel {
    pub fn new() -> Self {
        VoltageSourceIdealModel {}
    }
}

impl Model for VoltageSourceIdealModel {
    type ComponentType = VoltageSource;
}
