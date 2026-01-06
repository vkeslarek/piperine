use crate::component::ind::Inductor;
use crate::model::Model;

pub type InductorModel = dyn Model<ComponentType = Inductor> + 'static;

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
