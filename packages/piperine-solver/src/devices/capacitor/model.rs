use crate::devices::Model;
use crate::devices::capacitor::Capacitor;
use crate::util::AsAny;
use std::any::Any;

pub type CapacitorModelType = dyn Model<ComponentType = Capacitor> + 'static;

#[derive(Debug, Clone)]
pub struct CapacitorModel {}

impl CapacitorModel {
    pub fn new() -> Self {
        Self {}
    }
}

impl AsAny for CapacitorModel {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Model for CapacitorModel {
    type ComponentType = Capacitor;
}
