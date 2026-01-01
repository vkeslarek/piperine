pub mod cap;
pub mod dio;
pub mod ind;
pub mod res;
pub mod vsrc;

use crate::component::Component;
use crate::state::CircuitStates;
use std::any::Any;

pub trait Model {
    type ComponentType: Component;

    fn name(&self) -> String;

    fn update(
        &self,
        component: &mut Self::ComponentType,
        circuit_states: &CircuitStates,
    ) -> crate::error::Result<()> {
        Ok(())
    }
}

pub trait AnyModel: 'static + Any {
    fn as_any(&self) -> &dyn Any;
    fn name(&self) -> String;
}

impl<M: 'static + Model> AnyModel for M {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> String {
        M::name(self)
    }
}
