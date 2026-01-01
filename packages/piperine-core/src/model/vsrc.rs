use crate::component::vsrc::VoltageSource;
use crate::model::Model;

pub type VoltageSourceModel = dyn Model<ComponentType = VoltageSource>;

pub struct VoltageSourceIdealModel {
    pub name: String,
}

impl VoltageSourceIdealModel {
    pub fn new(name: String) -> Self {
        VoltageSourceIdealModel { name }
    }
}

impl Model for VoltageSourceIdealModel {
    type ComponentType = VoltageSource;

    fn name(&self) -> String {
        self.name.clone()
    }
}
