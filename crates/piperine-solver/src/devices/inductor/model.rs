use crate::devices::Model;
use crate::devices::inductor::Inductor;
use crate::util::AsAny;
use std::any::Any;

pub type InductorModelType = dyn Model<ComponentType = Inductor> + 'static;

#[derive(Debug, Clone)]
pub struct InductorModel {}

impl InductorModel {
    pub fn new() -> Self {
        Self {}
    }
}

impl AsAny for InductorModel {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Model for InductorModel {
    type ComponentType = Inductor;
}
