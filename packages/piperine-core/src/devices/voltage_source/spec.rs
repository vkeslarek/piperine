use crate::devices::voltage_source::VoltageSource;
use crate::devices::voltage_source::model::VoltageSourceIdealModel;
use crate::devices::{Component, ComponentSpec, ModelResolver};
use crate::math::param::{IntoParameter, Parameter};
use crate::math::unit::Voltage;
use crate::netlist::{BranchIdentifier, IntoNodeIdentifier, Netlist, NodeIdentifier};
use std::any::Any;
use std::sync::Arc;

pub struct VoltageSourceSpec {
    pub name: String,
    pub model: Option<String>,
    pub node_plus: NodeIdentifier,
    pub node_minus: NodeIdentifier,
    pub voltage: Parameter<Voltage>,
}

impl VoltageSourceSpec {
    pub fn new(
        name: &str,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        voltage: impl IntoParameter<Voltage>,
    ) -> VoltageSourceSpec {
        VoltageSourceSpec {
            name: name.to_string(),
            model: None,
            node_plus: node_p.into(),
            node_minus: node_n.into(),
            voltage: voltage.into_parameter(),
        }
    }

    pub fn with_model(&mut self, name: &str) -> &mut Self {
        self.model = Some(name.to_string());
        self
    }
}

impl ComponentSpec for VoltageSourceSpec {
    fn instantiate(
        &self,
        netlist: &mut Netlist,
        model_resolver: &ModelResolver,
    ) -> crate::error::Result<Box<dyn Component>> {
        Ok(Box::new(VoltageSource {
            name: self.name.to_string(),
            model: model_resolver
                .resolve(self.model.clone())
                .unwrap_or_else(|| Arc::new(VoltageSourceIdealModel::new())),
            node_plus: netlist.connect_node(self.node_plus.clone()),
            node_minus: netlist.connect_node(self.node_minus.clone()),
            branch: netlist.connect_branch(BranchIdentifier {
                component: self.name.to_string(),
                name: None,
            }),
            voltage: self.voltage.sample(),
        }))
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
