use crate::analysis::dc::DcAnalysis;
use crate::circuit::netlist::{IntoNodeIdentifier, Netlist, NodeIdentifier};
use crate::devices::diode::model::{DiodeModel, DiodeModelType};
use crate::devices::diode::runtime::DiodeRuntime;
use crate::devices::{AnyRuntime, Component, Runtime};
use crate::math::unit::{Kelvin, UnitExt};
use crate::util::AsAny;
use std::any::Any;
use std::sync::Arc;

mod model;
mod runtime;

#[derive(Clone)]
pub struct Diode {
    name: String,
    model: Arc<dyn DiodeModelType>,
    node_plus: NodeIdentifier,
    node_minus: NodeIdentifier,

    pub temp: Option<Kelvin>,
}

impl Diode {
    pub fn new(
        name: String,
        node_plus: impl IntoNodeIdentifier,
        node_minus: impl IntoNodeIdentifier,
    ) -> Self {
        Self {
            name: name.to_string(),
            model: Arc::new(DiodeModel::default()),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            temp: None,
        }
    }

    pub fn with_model(&mut self, model: Arc<dyn DiodeModelType>) -> &mut Self {
        self.model = model;
        self
    }

    pub fn model(&self) -> &Arc<dyn DiodeModelType> {
        &self.model
    }
}

// Standard Boilerplate
impl AsAny for Diode {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Component for Diode {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn runtime(&self, netlist: &mut Netlist) -> Box<dyn AnyRuntime> {
        Box::new(DiodeRuntime::allocate(Arc::new(self.clone()), netlist))
    }
}
