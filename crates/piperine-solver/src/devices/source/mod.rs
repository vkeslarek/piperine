pub mod model;
mod runtime;

use crate::circuit::netlist::{IntoNodeIdentifier, Netlist, NodeIdentifier};
use crate::devices::source::model::{VoltageSourceModel, VoltageSourceModelType};
use crate::devices::source::runtime::VoltageSourceRuntime;
use crate::devices::{AnyRuntime, Component, Runtime};
use crate::math::unit::{Hertz, Radian, Second, Volt};
use crate::util::AsAny;
use std::any::Any;
use std::sync::Arc;

#[derive(Clone)]
pub enum Waveform {
    DC(Volt),
    Sine {
        amplitude: Volt,
        frequency: Hertz,
        phase: Radian,
    },
    Step {
        initial: Volt,
        final_value: Volt,
        delay: Second,
        rise_time: Second,
    },
}

impl Into<Waveform> for Volt {
    fn into(self) -> Waveform {
        Waveform::DC(self)
    }
}

#[derive(Clone)]
pub struct VoltageSource {
    pub name: String,
    pub model: Arc<VoltageSourceModelType>,
    pub node_plus: NodeIdentifier,
    pub node_minus: NodeIdentifier,
    pub waveform: Waveform,
}

impl VoltageSource {
    pub fn new(
        name: String,
        node_plus: impl IntoNodeIdentifier,
        node_minus: impl IntoNodeIdentifier,
        waveform: Waveform,
    ) -> Self {
        Self {
            name: name.to_string(),
            model: Arc::new(VoltageSourceModel::new()),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            waveform,
        }
    }

    pub fn with_model(&mut self, model: Arc<VoltageSourceModelType>) -> &mut Self {
        self.model = model;
        self
    }
}

impl AsAny for VoltageSource {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Component for VoltageSource {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn runtime(&self, netlist: &mut Netlist) -> Box<dyn AnyRuntime> {
        Box::new(VoltageSourceRuntime::allocate(
            Arc::new(self.clone()),
            netlist,
        ))
    }
}
