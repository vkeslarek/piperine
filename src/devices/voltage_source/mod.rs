pub mod ac;
pub mod dc;
pub mod model;
pub mod tran;

use crate::analysis::ac::AcAnalysis;
use crate::analysis::dc::DcAnalysis;
use crate::analysis::transient::TransientAnalysis;
use crate::devices::Component;
use crate::devices::voltage_source::model::{VoltageSourceModel, VoltageSourceModelType};
use crate::math::unit::{Angle, Frequency, UnitExt, Voltage};
use crate::circuit::netlist::{BranchIdentifier, CircuitReference, IntoNodeIdentifier, Netlist};
use crate::util::AsAny;
use std::any::Any;
use std::sync::Arc;

pub enum Waveform {
    DC(Voltage),
    Sine {
        amplitude: Voltage,
        frequency: Frequency,
        phase: Angle,
    },
}

impl Into<Waveform> for Voltage {
    fn into(self) -> Waveform {
        Waveform::DC(self)
    }
}

impl Waveform {
    pub fn dc_value(&self) -> Voltage {
        match self {
            Waveform::DC(v) => *v,
            Waveform::Sine { amplitude, .. } => *amplitude,
        }
    }
}

pub struct VoltageSource {
    pub name: String,
    pub model: Arc<VoltageSourceModelType>,
    pub node_plus: CircuitReference,
    pub node_minus: CircuitReference,
    pub branch: CircuitReference,
    pub waveform: Waveform,

    // Runtime parameters
    pub voltage: Voltage,
}

impl VoltageSource {
    pub fn new(
        name: &str,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        waveform: Waveform,
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
            waveform,
            voltage: 0.0.V(),
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
