use crate::analysis::ac::AcAnalysis;
use crate::analysis::dc::DcAnalysis;
use crate::analysis::noise::NoiseSource;
use crate::analysis::transient::TransientAnalysis;
use crate::circuit::netlist::{CircuitReference, IntoNodeIdentifier, Netlist};
use crate::devices::resistor::model::ResistorModel;
use crate::devices::{Component, Model};
use crate::math::unit::{Dimensionless, Kelvin, Meter, Ohm, Siemens, UnitExt};
use crate::util::AsAny;
use std::any::Any;
use std::sync::Arc;

pub mod ac;
pub mod dc;
pub mod model;
mod noise;
pub mod transient;
mod soa;

#[derive(Clone)]
pub struct Resistor {
    name: String,
    model: Arc<ResistorModel>,
    node_plus: CircuitReference,
    node_minus: CircuitReference,

    resistance: Option<Ohm>,
    ac_resistance: Option<Ohm>,
    length: Option<Meter>,
    width: Option<Meter>,
    scale: Option<Dimensionless>,
    multiplier: Option<Dimensionless>,

    temp: Option<Kelvin>,
    delta_temp: Option<Kelvin>,
    tc1: Option<Dimensionless>,
    tc2: Option<Dimensionless>,
    tce: Option<Dimensionless>,
    noisy: bool,

    // Runtime parameters
    conductance: Siemens,
}

impl Resistor {
    pub fn new(
        name: String,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        resistance: Option<Ohm>,
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

    pub fn with_model(&mut self, model: Arc<ResistorModel>) -> &mut Resistor {
        self.model = model;
        self
    }

    pub fn with_ac_resistance(&mut self, ac_resistance: Ohm) -> &mut Resistor {
        self.ac_resistance = Some(ac_resistance);
        self
    }

    pub fn with_dimensions(&mut self, width: Meter, length: Meter) -> &mut Resistor {
        self.width = Some(width);
        self.length = Some(length);
        self
    }

    pub fn with_scale(&mut self, scale: Dimensionless) -> &mut Resistor {
        self.scale = Some(scale);
        self
    }

    pub fn with_multiplier(&mut self, multiplier: Dimensionless) -> &mut Resistor {
        self.multiplier = Some(multiplier);
        self
    }

    pub fn with_temp(&mut self, temp: Kelvin) -> &mut Resistor {
        self.temp = Some(temp);
        self
    }

    pub fn with_delta_temp(&mut self, delta_temp: Kelvin) -> &mut Resistor {
        self.delta_temp = Some(delta_temp);
        self
    }

    pub fn with_temperature_coefficients(
        &mut self,
        tc1: Dimensionless,
        tc2: Dimensionless,
    ) -> &mut Resistor {
        self.tc1 = Some(tc1);
        self.tc2 = Some(tc2);
        self
    }

    pub fn with_exponential_temperature_coefficient(
        &mut self,
        tce: Dimensionless,
    ) -> &mut Resistor {
        self.tce = Some(tce);
        self
    }

    pub fn with_noise(&mut self, enable: bool) -> &mut Resistor {
        self.noisy = enable;
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

    fn as_noise_source(&mut self) -> Option<&mut dyn NoiseSource> {
        Some(self)
    }
}
