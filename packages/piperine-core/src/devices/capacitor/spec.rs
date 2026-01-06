use crate::devices::capacitor::Capacitor;
use crate::devices::capacitor::model::CapacitorIdealModel;
use crate::devices::{Component, ComponentSpec, ModelResolver};
use crate::math::param::{IntoParameter, Parameter};
use crate::math::unit::Capacitance;
use crate::netlist::{IntoNodeIdentifier, Netlist, NodeIdentifier};
use std::any::Any;
use std::sync::Arc;

pub struct CapacitorSpec {
    pub name: String,
    pub model: Option<String>,
    pub node_plus: NodeIdentifier,
    pub node_minus: NodeIdentifier,
    pub capacitance: Parameter<Capacitance>,
}

impl CapacitorSpec {
    pub fn new(
        name: &str,
        node_p: impl IntoNodeIdentifier,
        node_m: impl IntoNodeIdentifier,
        capacitance: impl IntoParameter<Capacitance>,
    ) -> CapacitorSpec {
        CapacitorSpec {
            name: name.to_string(),
            model: None,
            node_plus: node_p.into(),
            node_minus: node_m.into(),
            capacitance: capacitance.into_parameter(),
        }
    }
    pub fn with_model(&mut self, name: &str) -> &mut Self {
        self.model = Some(name.to_string());
        self
    }
}

impl ComponentSpec for CapacitorSpec {
    fn instantiate(
        &self,
        netlist: &mut Netlist,
        model_resolver: &ModelResolver,
    ) -> crate::error::Result<Box<dyn Component>> {
        Ok(Box::new(Capacitor {
            name: self.name.clone(),
            model: model_resolver
                .resolve(self.model.clone())
                .unwrap_or_else(|| Arc::new(CapacitorIdealModel::new())),
            node_plus: netlist.connect_node(self.node_plus.clone()),
            node_minus: netlist.connect_node(self.node_minus.clone()),
            capacitance: self.capacitance.sample(),
        }))
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
