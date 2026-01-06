pub mod model;
pub mod spec;
pub mod dc;
pub mod tran;
pub mod ac;

use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::analysis::dc::DcAnalysis;
use crate::analysis::transient::{TransientAnalysis, TransientAnalysisContext};
use crate::devices::inductor::model::InductorModelType;
use crate::devices::{Component, ComponentSpec, ModelResolver};
use crate::math::linear::Stamp;
use crate::math::param::Parameter;
use crate::math::unit::{Inductance, ReactanceConvert};
use crate::netlist::{BranchIdentifier, CircuitReference, Netlist, NodeIdentifier};
use crate::solver::Context;
use crate::state::CircuitState;
use num_complex::Complex;
use num_traits::One;
use std::any::Any;
use std::sync::Arc;

pub struct InductorParameters {
    pub name: String,
    pub model: Arc<InductorModelType>,
    pub node_plus: NodeIdentifier,
    pub node_minus: NodeIdentifier,
    pub inductance: Inductance,
}

pub struct Inductor {
    pub name: String,
    pub model: Arc<InductorModelType>,
    pub node_plus: CircuitReference,
    pub node_minus: CircuitReference,
    pub branch: CircuitReference,
    pub inductance: Inductance,
}

impl Component for Inductor {
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

impl Inductor {
    pub fn new(
        netlist: &mut Netlist,
        parameters: InductorParameters,
    ) -> crate::error::Result<Self> {
        Ok(Self {
            name: parameters.name.clone(),
            model: parameters.model,
            node_plus: netlist.connect_node(parameters.node_plus),
            node_minus: netlist.connect_node(parameters.node_minus),
            branch: netlist.connect_branch(BranchIdentifier {
                component: parameters.name,
                name: None,
            }),
            inductance: parameters.inductance,
        })
    }
}
