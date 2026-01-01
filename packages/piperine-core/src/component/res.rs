use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::analysis::dc::DcAnalysis;
use crate::analysis::transient::{TransientAnalysis, TransientAnalysisContext};
use crate::circuit::{CircuitReference, Netlist, NodeIdentifier};
use crate::component::{Component, Context};
use crate::model::res::{ResistorIdealModel, ResistorModel};
use crate::solver::Stamp;
use crate::state::CircuitStates;
use num_complex::Complex;
use piperine_macros::stamps;
use std::sync::Arc;

pub struct ResistorParameters {
    pub name: String,
    pub model: Arc<ResistorModel>,
    pub node_plus: NodeIdentifier,
    pub node_minus: NodeIdentifier,

    pub resistance: Option<f64>,
    pub length: Option<f64>,
    pub width: Option<f64>,
    pub scale: Option<f64>,
    pub m: Option<f64>,

    pub temp: Option<f64>,
    pub dtemp: Option<f64>,
    pub tc1: Option<f64>,
    pub tc2: Option<f64>,
    pub tce: Option<f64>,
}

impl Default for ResistorParameters {
    fn default() -> Self {
        Self {
            name: "Unknown".to_string(),
            model: Arc::new(ResistorIdealModel::new("DefaultResistorModel".to_string())),
            node_plus: NodeIdentifier::Gnd,
            node_minus: NodeIdentifier::Gnd,
            resistance: None,
            length: None,
            width: None,
            scale: None,
            m: None,
            temp: None,
            dtemp: None,
            tc1: None,
            tc2: None,
            tce: None,
        }
    }
}

pub struct Resistor {
    pub name: String,
    pub model: Arc<ResistorModel>,
    pub node_plus: CircuitReference,
    pub node_minus: CircuitReference,

    pub resistance: Option<f64>,
    pub conductance: f64,
    pub length: f64,
    pub width: f64,
    pub scale: f64,
    pub m: f64,

    pub temp: Option<f64>,
    pub dtemp: f64,
    pub tc1: Option<f64>,
    pub tc2: Option<f64>,
    pub tce: Option<f64>,
}

impl Component for Resistor {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn update(
        &mut self,
        circuit_states: &CircuitStates,
        context: &Context,
    ) -> crate::error::Result<()> {
        let model = self.model.clone();
        model.update(self, circuit_states)
    }

    fn as_dc(&self) -> Option<&dyn DcAnalysis> {
        Some(self)
    }

    fn as_transient(&self) -> Option<&dyn TransientAnalysis> {
        Some(self)
    }

    fn as_ac(&self) -> Option<&dyn AcAnalysis> {
        Some(self)
    }
}

impl Resistor {
    pub fn new(
        netlist: &mut Netlist,
        parameters: ResistorParameters,
    ) -> crate::error::Result<Self> {
        Ok(Self {
            name: parameters.name,
            model: parameters.model,
            node_plus: netlist.connect_node(parameters.node_plus),
            node_minus: netlist.connect_node(parameters.node_minus),
            resistance: parameters.resistance,
            conductance: 0.0,
            length: parameters.length.unwrap_or(10e-6),
            width: parameters.width.unwrap_or(10e-6),
            scale: parameters.scale.unwrap_or(1.0),
            m: parameters.m.unwrap_or(1.0),
            temp: parameters.temp,
            dtemp: parameters.dtemp.unwrap_or(0.0),
            tc1: parameters.tc1,
            tc2: parameters.tc2,
            tce: parameters.tce,
        })
    }
}

impl DcAnalysis for Resistor {
    fn load_dc(&self, context: &Context) -> Vec<Stamp<f64>> {
        stamps!(
            KCL(self.node_plus): {
                self.node_plus  => self.conductance,
                self.node_minus => -self.conductance
            },
            KCL(self.node_minus): {
                self.node_plus  => -self.conductance,
                self.node_minus => self.conductance
            }
        )
    }
}

impl TransientAnalysis for Resistor {
    fn load_transient(
        &self,
        _: &CircuitStates,
        _: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<f64>> {
        self.load_dc(context)
    }
}

impl AcAnalysis for Resistor {
    fn load_ac(
        &self,
        _circuit_states: &CircuitStates,
        _: &AcAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<Complex<f64>>> {
        let g = Complex::new(self.conductance, 0.0);
        stamps!(
            KCL(self.node_plus): {
                self.node_plus  => g,
                self.node_minus => -g
            },
            KCL(self.node_minus): {
                self.node_plus  => -g,
                self.node_minus => g
            }
        )
    }
}
