use std::any::Any;
use crate::analysis::ac::AcAnalysis;
use crate::analysis::dc::DcAnalysis;
use crate::analysis::transient::TransientAnalysis;
use crate::devices::Model;
use crate::devices::voltage_source::VoltageSource;
use crate::math::unit::{UnitExt, Voltage};
use crate::netlist::{BranchIdentifier, CircuitReference, Netlist};
use crate::util::AsAny;

pub type VoltageSourceModelType = dyn Model<ComponentType = VoltageSource>;

#[derive(Debug)]
pub struct VoltageSourceModel {}

impl VoltageSourceModel {
    pub fn new() -> Self {
        VoltageSourceModel {}
    }
}

impl AsAny for VoltageSourceModel {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Model for VoltageSourceModel {
    type ComponentType = VoltageSource;
}
