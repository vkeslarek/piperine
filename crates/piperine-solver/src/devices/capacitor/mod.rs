pub mod model;
mod runtime;

use crate::circuit::netlist::{IntoNodeIdentifier, Netlist, NodeIdentifier};
use crate::devices::capacitor::model::{CapacitorModel, CapacitorModelType};
use crate::devices::capacitor::runtime::CapacitorRuntime;
use crate::devices::{AnyRuntime, Component, Runtime};
use crate::math::unit::Farad;
use crate::util::AsAny;
use std::any::Any;
use std::sync::Arc;

#[derive(Clone)]
pub struct Capacitor {
    pub name: String,
    pub model: Arc<CapacitorModelType>,
    pub node_plus: NodeIdentifier,
    pub node_minus: NodeIdentifier,
    pub capacitance: Farad,
}

impl Capacitor {
    pub fn new(
        name: String,
        node_plus: impl IntoNodeIdentifier,
        node_minus: impl IntoNodeIdentifier,
        capacitance: Farad,
    ) -> Self {
        Self {
            name: name.to_string(),
            model: Arc::new(CapacitorModel::new()),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            capacitance,
        }
    }
    pub fn with_model(&mut self, model: Arc<CapacitorModelType>) -> &mut Self {
        self.model = model;
        self
    }
}

impl AsAny for Capacitor {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Component for Capacitor {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn runtime(&self, netlist: &mut Netlist) -> Box<dyn AnyRuntime> {
        Box::new(CapacitorRuntime::allocate(Arc::new(self.clone()), netlist))
    }
}
