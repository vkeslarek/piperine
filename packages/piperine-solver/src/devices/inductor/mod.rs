pub mod model;

mod runtime;
#[cfg(test)]
pub mod test;

use crate::circuit::netlist::{IntoNodeIdentifier, Netlist, NodeIdentifier};
use crate::devices::inductor::model::{InductorModel, InductorModelType};
use crate::devices::inductor::runtime::InductorRuntime;
use crate::devices::{AnyRuntime, Component, Runtime};
use crate::math::unit::Henry;
use crate::util::AsAny;
use std::any::Any;
use std::sync::Arc;

#[derive(Clone)]
pub struct Inductor {
    pub name: String,
    pub model: Arc<InductorModelType>,
    pub node_plus: NodeIdentifier,
    pub node_minus: NodeIdentifier,
    pub inductance: Henry,
}

impl Inductor {
    pub fn new(
        name: String,
        node_plus: impl IntoNodeIdentifier,
        node_minus: impl IntoNodeIdentifier,
        inductance: Henry,
    ) -> Self {
        Self {
            name: name.to_string(),
            model: Arc::new(InductorModel::new()),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            inductance,
        }
    }
}

impl AsAny for Inductor {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Component for Inductor {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn runtime(&self, netlist: &mut Netlist) -> Box<dyn AnyRuntime> {
        Box::new(InductorRuntime::allocate(Arc::new(self.clone()), netlist))
    }
}
