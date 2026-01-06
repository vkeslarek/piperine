use std::sync::Arc;
use crate::analysis::ac::AcAnalysis;
use crate::analysis::dc::DcAnalysis;
use crate::analysis::transient::TransientAnalysis;
use crate::math::unit::{Conductance, Length, LinearTemperatureCoefficient, QuadraticTemperatureCoefficient, Ratio, Resistance, Temperature, TemperatureInterval};
use model::ResistorModelType;
use crate::devices::Component;
use crate::netlist::CircuitReference;

pub mod spec;
pub mod dc;
pub mod tran;
pub mod ac;
pub mod model;

pub struct Resistor {
    pub name: String,
    pub model: Arc<ResistorModelType>,
    pub node_plus: CircuitReference,
    pub node_minus: CircuitReference,

    pub resistance: Option<Resistance>,
    pub conductance: Conductance,
    pub length: Option<Length>,
    pub width: Option<Length>,
    pub scale: Ratio,
    pub multiplier: Ratio,

    pub temp: Option<Temperature>,
    pub dtemp: Option<TemperatureInterval>,
    pub tc1: Option<LinearTemperatureCoefficient>,
    pub tc2: Option<QuadraticTemperatureCoefficient>,
    pub tce: Option<Ratio>,
}

impl Component for Resistor {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn update(&mut self) -> crate::error::Result<()> {
        let model = self.model.clone();
        model.update(self)
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
