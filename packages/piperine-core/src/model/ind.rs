use crate::component::ind::Inductor;
use crate::model::Model;

pub type InductorModel = dyn Model<ComponentType = Inductor> + 'static;

pub struct InductorIdealModel {
    pub name: String,
}

impl InductorIdealModel {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

impl Model for InductorIdealModel {
    type ComponentType = Inductor;
    fn name(&self) -> String {
        self.name.clone()
    }
}
