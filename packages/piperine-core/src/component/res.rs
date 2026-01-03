use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::analysis::dc::DcAnalysis;
use crate::analysis::transient::{TransientAnalysis, TransientAnalysisContext};
use crate::circuit::{CircuitReference, IntoNodeIdentifier, Netlist, NodeIdentifier};
use crate::component::{Component, Context};
use crate::experiment::{ComponentBlueprint, ModelResolver};
use crate::math::param::{IntoOptionalParameter, IntoParameter, OptionalParameter, SampleOptional};
use crate::math::unit::{
    Admittance, AdmittanceConvert, Conductance, ConductanceExt, DimensionLessExt, Length,
    MetersExt, OhmsPerCelsius, OhmsPerCelsiusSquared, Ratio, Resistance, TempCoeffExt, Temperature,
    TemperatureExt,
};
use crate::model::res::ResistorModel;
use crate::solver::Stamp;
use crate::state::CircuitStates;
use num_complex::Complex;
use num_traits::One;
use piperine_macros::stamps;
use std::any::Any;
use std::sync::Arc;

struct ResistorSpec {
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
    delta_temp: OptionalParameter<Temperature>,
    tc1: OptionalParameter<OhmsPerCelsius>,
    tc2: OptionalParameter<OhmsPerCelsiusSquared>,
    noisy: bool,
}

impl ResistorSpec {
    fn new(
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
        delta_temp: impl IntoOptionalParameter<Temperature>,
    ) -> &mut ResistorSpec {
        self.delta_temp = delta_temp.into_optional_parameter();
        self
    }

    pub fn with_temperature_coefficients(
        &mut self,
        tc1: impl IntoOptionalParameter<OhmsPerCelsius>,
        tc2: impl IntoOptionalParameter<OhmsPerCelsiusSquared>,
    ) -> &mut ResistorSpec {
        self.tc1 = tc1.into_optional_parameter();
        self.tc2 = tc2.into_optional_parameter();
        self
    }
}

impl ComponentBlueprint for ResistorSpec {
    fn instantiate(
        &self,
        netlist: &mut Netlist,
        model_resolver: &ModelResolver,
    ) -> crate::error::Result<Box<dyn Component>> {
        Ok(Box::new(Resistor {
            name: self.name.clone(),
            model: model_resolver
                .resolve(self.model.clone())
                .expect("failed to resolve model"),
            node_plus: netlist.connect_node(self.node_plus.clone()),
            node_minus: netlist.connect_node(self.node_minus.clone()),
            resistance: self.resistance.sample_opt(),
            conductance: 0.0.S(),
            length: self.length.sample_opt().unwrap_or(10.0.um()),
            width: self.length.sample_opt().unwrap_or(10.0.um()),
            scale: self.scale.sample_opt().unwrap_or(1.0.ratio()),
            m: self.multiplier.sample_opt().unwrap_or(1.0.ratio()),
            temp: self.temp.sample_opt().unwrap_or(27.0.degC()),
            dtemp: self.delta_temp.sample_opt().unwrap_or(0.0.degC()),
            tc1: self.tc1.sample_opt().unwrap_or(0.0.OhmsPerC()),
            tc2: self.tc2.sample_opt().unwrap_or(0.0.OhmsPerC2()),
            // tce: parameters.tce,
        }))
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

pub trait ResistorFactory {
    fn resistor(
        &mut self,
        name: &str,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        resistance: impl IntoOptionalParameter<Resistance>,
    ) -> &mut ResistorSpec;
}

impl ResistorFactory for crate::experiment::CircuitBuilder {
    fn resistor(
        &mut self,
        name: &str,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        resistance: impl IntoOptionalParameter<Resistance>,
    ) -> &mut ResistorSpec {
        self.insert_get(name, ResistorSpec::new(name, node_p, node_n, resistance))
            .expect("Failed to insert component")
    }
}

pub struct Resistor {
    pub name: String,
    pub model: Arc<ResistorModel>,
    pub node_plus: CircuitReference,
    pub node_minus: CircuitReference,

    pub resistance: Option<Resistance>,
    pub conductance: Conductance,
    pub length: Length,
    pub width: Length,
    pub scale: Ratio,
    pub m: Ratio,

    pub temp: Temperature,
    pub dtemp: Temperature,
    pub tc1: OhmsPerCelsius,
    pub tc2: OhmsPerCelsiusSquared,
    // pub tce: Option<f64>,
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

// impl Resistor {
//     pub fn new(
//         netlist: &mut Netlist,
//         parameters: ResistorParameters,
//     ) -> crate::error::Result<Self> {
//         Ok(Self {
//             name: parameters.name,
//             model: parameters.model,
//             node_plus: netlist.connect_node(parameters.node_plus),
//             node_minus: netlist.connect_node(parameters.node_minus),
//             resistance: parameters.resistance,
//             conductance: 0.0,
//             length: parameters.length.unwrap_or(10.0.um()),
//             width: parameters.width.unwrap_or(10e-6),
//             scale: parameters.scale.unwrap_or(1.0),
//             m: parameters.m.unwrap_or(1.0),
//             temp: parameters.temp,
//             dtemp: parameters.dtemp.unwrap_or(0.0),
//             tc1: parameters.tc1,
//             tc2: parameters.tc2,
//             tce: parameters.tce,
//         })
//     }
// }

impl DcAnalysis for Resistor {
    fn load_dc(&self, context: &Context) -> Vec<Stamp<Conductance>> {
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
    ) -> Vec<Stamp<Conductance>> {
        self.load_dc(context)
    }
}

impl AcAnalysis for Resistor {
    fn load_ac(
        &self,
        _circuit_states: &CircuitStates,
        _: &AcAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<Admittance>> {
        let g = self.conductance.to_admittance();

        stamps!(
            KCL(self.node_plus): {
                self.node_plus  => g,
                self.node_minus => g * -Complex::one()
            },
            KCL(self.node_minus): {
                self.node_plus  => g * -Complex::one(),
                self.node_minus => g
            }
        )
    }
}
