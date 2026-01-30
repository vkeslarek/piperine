pub mod ac;
pub mod dc;
pub mod model;
pub mod tran;

#[cfg(test)]
pub mod test;

use crate::analysis::ac::AcAnalysis;
use crate::analysis::dc::DcAnalysis;
use crate::analysis::transient::TransientAnalysis;
use crate::circuit::netlist::{
    BranchIdentifier, CircuitReference, IntoNodeIdentifier, Netlist,
};
use crate::devices::Component;
use crate::devices::inductor::model::{InductorModel, InductorModelType};
use crate::math::unit::Henry;
use crate::util::AsAny;
use std::any::Any;
use std::sync::Arc;

pub struct Inductor {
    pub name: String,
    pub model: Arc<InductorModelType>,
    pub node_plus: CircuitReference,
    pub node_minus: CircuitReference,
    pub current_ref: CircuitReference,
    pub inductance: Henry,
}

impl Inductor {
    pub fn new(
        name: String,
        node_p: impl IntoNodeIdentifier,
        node_m: impl IntoNodeIdentifier,
        inductance: Henry,
        netlist: &mut Netlist,
    ) -> Self {
        let current_ref = netlist.connect_branch(BranchIdentifier::from_component(name.clone()));

        Self {
            name: name.to_string(),
            model: Arc::new(InductorModel::new()),
            node_plus: netlist.connect_node(node_p.into().clone()),
            node_minus: netlist.connect_node(node_m.into().clone()),
            current_ref,
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
    fn as_dc(&mut self) -> Option<&mut dyn DcAnalysis> {
        Some(self)
    }
    fn as_ac(&mut self) -> Option<&mut dyn AcAnalysis> {
        Some(self)
    }
    fn as_transient(&mut self) -> Option<&mut dyn TransientAnalysis> {
        Some(self)
    }
}
