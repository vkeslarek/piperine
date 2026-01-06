use crate::devices::Model;
use crate::devices::voltage_source::VoltageSource;

pub type VoltageSourceModelType = dyn Model<ComponentType = VoltageSource>;

#[derive(Debug)]
pub struct VoltageSourceIdealModel {}

impl VoltageSourceIdealModel {
    pub fn new() -> Self {
        VoltageSourceIdealModel {}
    }
}

impl Model for VoltageSourceIdealModel {
    type ComponentType = VoltageSource;
}
