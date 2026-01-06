pub mod ac;
pub mod dc;
pub mod model;
pub mod spec;
pub mod tran;

use crate::analysis::ac::AcAnalysis;
use crate::analysis::dc::DcAnalysis;
use crate::analysis::transient::TransientAnalysis;
use crate::devices::voltage_source::model::VoltageSourceModelType;
use crate::devices::{Component, ComponentSpec, Model};
use crate::math::param::IntoParameter;
use crate::math::unit::Voltage;
use crate::netlist::{
    CircuitReference, IntoNodeIdentifier,
};
use num_traits::{One, ToPrimitive};
use std::any::Any;
use std::sync::Arc;

pub struct VoltageSource {
    pub name: String,
    pub model: Arc<VoltageSourceModelType>,
    pub node_plus: CircuitReference,
    pub node_minus: CircuitReference,
    pub branch: CircuitReference,
    pub voltage: Voltage,
}

impl Component for VoltageSource {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn as_dc_mut(&mut self) -> Option<&mut dyn DcAnalysis> {
        Some(self)
    }

    fn as_transient_mut(&mut self) -> Option<&mut dyn TransientAnalysis> {
        Some(self)
    }

    fn as_ac_mut(&mut self) -> Option<&mut dyn AcAnalysis> {
        Some(self)
    }
}
