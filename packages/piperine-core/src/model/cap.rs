use crate::component::cap::Capacitor;
use crate::model::Model;

pub type CapacitorModel = dyn Model<ComponentType = Capacitor> + 'static;

pub struct CapacitorIdealModel {}

impl CapacitorIdealModel {
    pub fn new() -> Self {
        Self {}
    }
}

impl Model for CapacitorIdealModel {
    type ComponentType = Capacitor;
}
