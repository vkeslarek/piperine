use crate::component::cap::Capacitor;
use crate::model::Model;

pub type CapacitorModel = dyn Model<ComponentType = Capacitor> + 'static;

pub struct CapacitorIdealModel {
    pub name: String,
}

impl CapacitorIdealModel {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

impl Model for CapacitorIdealModel {
    type ComponentType = Capacitor;
    fn name(&self) -> String {
        self.name.clone()
    }
}