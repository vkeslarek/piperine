pub mod ac;
pub mod dc;
pub mod model;
pub mod tran;

use std::any::Any;
use crate::analysis::ac::AcAnalysis;
use crate::analysis::dc::DcAnalysis;
use crate::analysis::transient::TransientAnalysis;
use crate::devices::Component;
use crate::devices::voltage_source::model::{VoltageSourceModel, VoltageSourceModelType};
use crate::math::unit::Voltage;
use crate::netlist::{BranchIdentifier, CircuitReference, IntoNodeIdentifier, Netlist};
use std::sync::Arc;
use crate::util::AsAny;

pub struct VoltageSource {
    pub name: String,
    pub model: Arc<VoltageSourceModelType>,
    pub node_plus: CircuitReference,
    pub node_minus: CircuitReference,
    pub branch: CircuitReference,
    pub voltage: Voltage,
}

impl VoltageSource {
    pub fn new(
        name: &str,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        voltage: Voltage,
        netlist: &mut Netlist,
    ) -> Self {
        Self {
            name: name.to_string(),
            model: Arc::new(VoltageSourceModel::new()),
            node_plus: netlist.connect_node(node_p.into()),
            node_minus: netlist.connect_node(node_n.into()),
            branch: netlist.connect_branch(BranchIdentifier {
                component: name.to_string(),
                name: None,
            }),
            voltage,
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
