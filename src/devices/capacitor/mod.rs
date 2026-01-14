pub mod ac;
pub mod dc;
pub mod model;
pub mod tran;

use crate::analysis::ac::AcAnalysis;
use crate::analysis::dc::DcAnalysis;
use crate::analysis::transient::TransientAnalysis;
use crate::devices::capacitor::model::{CapacitorModel, CapacitorModelType};
use crate::devices::{Component, Model};
use crate::math::param::IntoParameter;
use crate::circuit::netlist::{CircuitReference, IntoNodeIdentifier, Netlist};
use crate::util::AsAny;
use std::any::Any;
use std::sync::Arc;
use crate::math::unit::Farad;

pub struct Capacitor {
    pub name: String,
    pub model: Arc<CapacitorModelType>,
    pub node_plus: CircuitReference,
    pub node_minus: CircuitReference,
    pub capacitance: Farad,
}

impl Capacitor {
    pub fn new(
        name: &str,
        node_p: impl IntoNodeIdentifier,
        node_m: impl IntoNodeIdentifier,
        capacitance: Farad,
        netlist: &mut Netlist,
    ) -> Self {
        Self {
            name: name.to_string(),
            model: Arc::new(CapacitorModel::new()),
            node_plus: netlist.connect_node(node_p.into().clone()),
            node_minus: netlist.connect_node(node_m.into().clone()),
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
