pub mod ac;
pub mod dc;
pub mod model;
pub mod spec;
pub mod tran;

use crate::analysis::ac::AcModelInstance;
use crate::analysis::dc::DcAnalysis;
use crate::analysis::transient::TransientModelInstance;
use crate::devices::diode::model::DiodeModelType;
use crate::devices::{Component, ComponentSpec};
use crate::math::param::{IntoParameter, SampleOptional};
use crate::math::unit::{Conductance, Current, Ratio, UnitExt, Voltage};
use crate::netlist::{CircuitReference, IntoNodeIdentifier};
use num_complex::ComplexFloat;
use num_traits::Zero;
use std::any::Any;
use std::sync::Arc;

#[derive(Debug)]
pub struct Diode {
    pub name: String,
    pub model: Arc<DiodeModelType>,
    pub node_plus: CircuitReference,
    pub node_minus: CircuitReference,
    pub saturation_current: Current,
    pub emission_coefficient: Ratio,
    pub g_eq: Conductance,
    pub i_eq: Current,

    // CHANGE HERE: Separate new guess from old linearization point
    pub v_new: Voltage,        // The raw guess from the matrix (k)
    pub v_old: Voltage,        // The limited voltage from the previous iteration (k-1)
    pub v_guess: Voltage,      // The raw input from the matrix solver (Iteration K)
    pub v_linearized: Voltage, // The safe, limited voltage we used last time (Iteration K-1)
}

impl Component for Diode {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn update(&mut self) -> crate::error::Result<()> {
        self.model.clone().update(self)
    }

    fn as_dc_mut(&mut self) -> Option<&mut dyn DcAnalysis> {
        Some(self)
    }

    fn as_transient_mut(&mut self) -> Option<&mut dyn TransientModelInstance> {
        Some(self)
    }

    fn as_ac_mut(&mut self) -> Option<&mut dyn AcModelInstance> {
        Some(self)
    }
}
