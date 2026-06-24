use crate::circuit::netlist::{IntoNodeIdentifier, Netlist, NodeIdentifier};
use crate::devices::dynamic::Dynamic;
use crate::devices::resistor::model::ResistorModel;
use crate::devices::resistor::runtime::ResistorRuntime;
use crate::devices::{AnyRuntime, Component, Runtime};
use crate::math::unit::{Celsius, Dimensionless, Kelvin, Meter, Ohm};
use crate::util::AsAny;
use std::any::Any;
use std::sync::Arc;

pub mod model;
mod runtime;

#[derive(Clone)]
pub struct Resistor {
    name: String,
    model: Arc<ResistorModel>,
    node_plus: NodeIdentifier,
    node_minus: NodeIdentifier,

    resistance: Dynamic<Ohm>,
    ac: Option<Ohm>,
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
}

impl Resistor {
    pub fn new(
        name: String,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        resistance: impl Into<Dynamic<Ohm>>,
    ) -> Resistor {
        Resistor {
            name: name.to_string(),
            model: Arc::new(ResistorModel::default()),
            node_plus: node_p.into(),
            node_minus: node_n.into(),
            resistance: resistance.into(),
            ac: None,
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
        }
    }

    pub fn with_model(&mut self, model: Arc<ResistorModel>) -> &mut Resistor {
        self.model = model;
        self
    }

    pub fn with_ac(&mut self, ac: Ohm) -> &mut Resistor {
        self.ac = Some(ac);
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

    pub fn node_plus(&self) -> &NodeIdentifier {
        &self.node_plus
    }

    pub fn node_minus(&self) -> &NodeIdentifier {
        &self.node_minus
    }

    pub fn model(&self) -> &Arc<ResistorModel> {
        &self.model
    }

    pub fn resistance(&self) -> &Dynamic<Ohm> {
        &self.resistance
    }

    pub fn ac(&self) -> Option<Ohm> {
        self.ac
    }

    pub fn multiplier(&self) -> Option<Dimensionless> {
        self.multiplier
    }

    pub fn scale(&self) -> Option<Dimensionless> {
        self.scale
    }

    pub fn width(&self) -> Option<Meter> {
        self.width
    }

    pub fn length(&self) -> Option<Meter> {
        self.length
    }

    pub fn temp(&self) -> Option<Celsius> {
        self.temp
    }

    pub fn delta_temp(&self) -> Option<Celsius> {
        self.delta_temp
    }

    pub fn tc1(&self) -> Option<Dimensionless> {
        self.tc1
    }

    pub fn tc2(&self) -> Option<Dimensionless> {
        self.tc2
    }

    pub fn tce(&self) -> Option<Dimensionless> {
        self.tce
    }

    pub fn noisy(&self) -> bool {
        self.noisy
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

    fn runtime(&self, netlist: &mut Netlist) -> Box<dyn AnyRuntime> {
        Box::new(ResistorRuntime::allocate(Arc::new(self.clone()), netlist))
    }
}
