use crate::devices::resistor::Resistor;
use crate::devices::resistor::model::{ResistorModel, ResistorModelParameters};
use crate::devices::{Component, ComponentSpec, ModelResolver};
use crate::math::param::{IntoOptionalParameter, IntoParameter, OptionalParameter, SampleOptional};
use crate::math::unit::{
    Length, LinearTemperatureCoefficient, QuadraticTemperatureCoefficient, Ratio, Resistance,
    Temperature, TemperatureInterval, UnitExt,
};
use crate::netlist::{IntoNodeIdentifier, Netlist, NodeIdentifier};
use std::any::Any;
use std::sync::Arc;

pub struct ResistorSpec {
    name: String,
    model: Option<String>,
    node_plus: NodeIdentifier,
    node_minus: NodeIdentifier,

    resistance: OptionalParameter<Resistance>,
    ac_resistance: OptionalParameter<Resistance>,
    length: OptionalParameter<Length>,
    width: OptionalParameter<Length>,
    scale: OptionalParameter<Ratio>,
    multiplier: OptionalParameter<Ratio>,

    temp: OptionalParameter<Temperature>,
    delta_temp: OptionalParameter<TemperatureInterval>,
    tc1: OptionalParameter<LinearTemperatureCoefficient>,
    tc2: OptionalParameter<QuadraticTemperatureCoefficient>,
    tce: OptionalParameter<Ratio>,
    noisy: bool,
}

impl ResistorSpec {
    pub fn new(
        name: &str,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        resistance: impl IntoOptionalParameter<Resistance>,
    ) -> ResistorSpec {
        ResistorSpec {
            name: name.to_string(),
            model: None,
            node_plus: node_p.into(),
            node_minus: node_n.into(),
            resistance: resistance.into_optional_parameter(),
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
        }
    }

    pub fn with_model(&mut self, model: &str) -> &mut ResistorSpec {
        self.model = Some(model.to_string());
        self
    }

    pub fn with_ac_resistance(
        &mut self,
        ac_resistance: impl IntoParameter<Resistance>,
    ) -> &mut ResistorSpec {
        self.ac_resistance = Some(ac_resistance.into_parameter());
        self
    }

    pub fn with_dimensions(
        &mut self,
        width: impl IntoOptionalParameter<Length>,
        length: impl IntoOptionalParameter<Length>,
    ) -> &mut ResistorSpec {
        self.width = width.into_optional_parameter();
        self.length = length.into_optional_parameter();
        self
    }

    pub fn with_scale(&mut self, scale: impl IntoOptionalParameter<Ratio>) -> &mut ResistorSpec {
        self.scale = scale.into_optional_parameter();
        self
    }

    pub fn with_multiplier(
        &mut self,
        multiplier: impl IntoOptionalParameter<Ratio>,
    ) -> &mut ResistorSpec {
        self.multiplier = multiplier.into_optional_parameter();
        self
    }

    pub fn with_temp(
        &mut self,
        temp: impl IntoOptionalParameter<Temperature>,
    ) -> &mut ResistorSpec {
        self.temp = temp.into_optional_parameter();
        self
    }

    pub fn with_delta_temp(
        &mut self,
        delta_temp: impl IntoOptionalParameter<TemperatureInterval>,
    ) -> &mut ResistorSpec {
        self.delta_temp = delta_temp.into_optional_parameter();
        self
    }

    pub fn with_temperature_coefficients(
        &mut self,
        tc1: impl IntoOptionalParameter<LinearTemperatureCoefficient>,
        tc2: impl IntoOptionalParameter<QuadraticTemperatureCoefficient>,
    ) -> &mut ResistorSpec {
        self.tc1 = tc1.into_optional_parameter();
        self.tc2 = tc2.into_optional_parameter();
        self
    }

    pub fn with_exponential_temperature_coefficient(
        &mut self,
        tce: impl IntoOptionalParameter<Ratio>,
    ) -> &mut ResistorSpec {
        self.tce = tce.into_optional_parameter();
        self
    }
}

impl ComponentSpec for ResistorSpec {
    fn instantiate(
        &self,
        netlist: &mut Netlist,
        model_resolver: &ModelResolver,
    ) -> crate::error::Result<Box<dyn Component>> {
        Ok(Box::new(Resistor {
            name: self.name.clone(),
            model: model_resolver
                .resolve(self.model.clone())
                .unwrap_or_else(|| {
                    Arc::new(ResistorModel::new(ResistorModelParameters::default()))
                }),
            node_plus: netlist.connect_node(self.node_plus.clone()),
            node_minus: netlist.connect_node(self.node_minus.clone()),
            resistance: self.resistance.sample_opt(),
            conductance: 0.0.S(),
            length: self.length.sample_opt(),
            width: self.length.sample_opt(),
            scale: self.scale.sample_opt().unwrap_or(1.0.ratio()),
            multiplier: self.multiplier.sample_opt().unwrap_or(1.0.ratio()),
            temp: self.temp.sample_opt(),
            dtemp: self.delta_temp.sample_opt(),
            tc1: self.tc1.sample_opt(),
            tc2: self.tc2.sample_opt(),
            tce: self.tce.sample_opt(),
        }))
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
