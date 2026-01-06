use crate::devices::inductor::Inductor;
use crate::devices::Model;

pub type InductorModelType = dyn Model<ComponentType = Inductor> + 'static;

#[derive(Debug)]
pub struct InductorIdealModel {}

impl InductorIdealModel {
    pub fn new() -> Self {
        Self {}
    }
}

impl Model for InductorIdealModel {
    type ComponentType = Inductor;
}
