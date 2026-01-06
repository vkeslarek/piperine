pub mod ac;
pub mod dc;
pub mod model;
pub mod spec;
pub mod tran;

use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::analysis::dc::DcAnalysis;
use crate::analysis::transient::{TransientAnalysis, TransientAnalysisContext};
use crate::devices::capacitor::model::CapacitorModelType;
use crate::devices::{Component, ComponentSpec, ModelResolver};
use crate::math::linear::Stamp;
use crate::math::param::{IntoParameter, Parameter};
use crate::math::unit::{AdmittanceConvert, Capacitance, ReactanceConvert};
use crate::netlist::{CircuitReference, IntoNodeIdentifier, Netlist, NodeIdentifier};
use crate::solver::Context;
use crate::state::CircuitState;
use num_complex::Complex;
use std::any::Any;
use std::sync::Arc;

pub struct Capacitor {
    pub name: String,
    pub model: Arc<CapacitorModelType>,
    pub node_plus: CircuitReference,
    pub node_minus: CircuitReference,
    pub capacitance: Capacitance,
}

impl Component for Capacitor {
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
