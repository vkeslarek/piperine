use crate::devices::inductor::Inductor;
use crate::devices::inductor::model::InductorIdealModel;
use crate::devices::{Component, ComponentSpec, ModelResolver};
use crate::math::param::{IntoParameter, Parameter};
use crate::math::unit::Inductance;
use crate::netlist::{BranchIdentifier, IntoNodeIdentifier, Netlist, NodeIdentifier};
use std::any::Any;
use std::sync::Arc;

pub struct InductorSpec {
    pub name: String,
    pub model: Option<String>,
    pub node_plus: NodeIdentifier,
    pub node_minus: NodeIdentifier,
    pub inductance: Parameter<Inductance>,
}

impl InductorSpec {
    pub fn new(
        name: &str,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        inductance: impl IntoParameter<Inductance>,
    ) -> InductorSpec {
        InductorSpec {
            name: name.to_string(),
            model: None,
            node_plus: node_p.into(),
            node_minus: node_n.into(),
            inductance: inductance.into_parameter(),
        }
    }
}

impl ComponentSpec for InductorSpec {
    fn instantiate(
        &self,
        netlist: &mut Netlist,
        model_resolver: &ModelResolver,
    ) -> crate::error::Result<Box<dyn Component>> {
        Ok(Box::new(Inductor {
            name: self.name.clone(),
            model: model_resolver
                .resolve(self.model.clone())
                .unwrap_or_else(|| Arc::new(InductorIdealModel::new())),
            node_plus: netlist.connect_node(self.node_plus.clone()),
            node_minus: netlist.connect_node(self.node_minus.clone()),
            branch: netlist.connect_branch(BranchIdentifier {
                component: self.name.clone(),
                name: None,
            }),
            inductance: self.inductance.sample(),
        }))
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
