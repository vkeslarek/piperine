use std::any::Any;
use crate::analysis::ac::AcAnalysis;
use crate::analysis::dc::DcAnalysis;
use crate::analysis::transient::TransientAnalysis;
use crate::devices::resistor::model::{ResistorModel, ResistorModelType};
use crate::devices::{Component, Model};
use crate::math::unit::{
    Conductance, Length, LinearTemperatureCoefficient, QuadraticTemperatureCoefficient, Ratio,
    Resistance, Temperature, TemperatureInterval, UnitExt,
};
use crate::netlist::{CircuitReference, IntoNodeIdentifier, Netlist};
use std::sync::Arc;
use crate::util::AsAny;

pub mod ac;
pub mod dc;
pub mod model;
pub mod tran;

#[derive(Clone)]
pub struct Resistor {
    name: String,
    model: Arc<dyn ResistorModelType>,
    node_plus: CircuitReference,
    node_minus: CircuitReference,

    resistance: Option<Resistance>,
    ac_resistance: Option<Resistance>,
    length: Option<Length>,
    width: Option<Length>,
    scale: Option<Ratio>,
    multiplier: Option<Ratio>,

    temp: Option<Temperature>,
    delta_temp: Option<TemperatureInterval>,
    tc1: Option<LinearTemperatureCoefficient>,
    tc2: Option<QuadraticTemperatureCoefficient>,
    tce: Option<Ratio>,
    noisy: bool,

    // Runtime parameters
    conductance: Conductance,
}

impl Resistor {
    pub fn new(
        name: &str,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        resistance: Option<Resistance>,
        netlist: &mut Netlist,
    ) -> Resistor {
        Resistor {
            name: name.to_string(),
            model: Arc::new(ResistorModel::default()),
            node_plus: netlist.connect_node(node_p.into()),
            node_minus: netlist.connect_node(node_n.into()),
            resistance,
            ac_resistance: None,
            length: None,
            width: None,
            scale: None,
            multiplier: None,
            temp: None,
            delta_temp: None,
            tc1: None,
            tc2: None,
            tce: None,
            noisy: false,
            conductance: 1.0.pS(),
        }
    }

    pub fn with_model(&mut self, model: Arc<dyn ResistorModelType>) -> &mut Resistor {
        self.model = model;
        self
    }

    pub fn with_ac_resistance(&mut self, ac_resistance: Resistance) -> &mut Resistor {
        self.ac_resistance = Some(ac_resistance);
        self
    }

    pub fn with_dimensions(&mut self, width: Length, length: Length) -> &mut Resistor {
        self.width = Some(width);
        self.length = Some(length);
        self
    }

    pub fn with_scale(&mut self, scale: Ratio) -> &mut Resistor {
        self.scale = Some(scale);
        self
    }

    pub fn with_multiplier(&mut self, multiplier: Ratio) -> &mut Resistor {
        self.multiplier = Some(multiplier);
        self
    }

    pub fn with_temp(&mut self, temp: Temperature) -> &mut Resistor {
        self.temp = Some(temp);
        self
    }

    pub fn with_delta_temp(&mut self, delta_temp: TemperatureInterval) -> &mut Resistor {
        self.delta_temp = Some(delta_temp);
        self
    }

    pub fn with_temperature_coefficients(
        &mut self,
        tc1: LinearTemperatureCoefficient,
        tc2: QuadraticTemperatureCoefficient,
    ) -> &mut Resistor {
        self.tc1 = Some(tc1);
        self.tc2 = Some(tc2);
        self
    }

    pub fn with_exponential_temperature_coefficient(&mut self, tce: Ratio) -> &mut Resistor {
        self.tce = Some(tce);
        self
    }
}

impl AsAny for Resistor {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Component for Resistor {
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
