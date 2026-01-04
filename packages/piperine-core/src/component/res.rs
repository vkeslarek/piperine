use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::analysis::dc::DcAnalysis;
use crate::analysis::transient::{TransientAnalysis, TransientAnalysisContext};
use crate::component::{Component, ComponentSpec, Context};
use crate::math::linear::Stamp;
use crate::math::param::{IntoOptionalParameter, IntoParameter, OptionalParameter, SampleOptional};
use crate::math::unit::{
    AdmittanceConvert, Conductance, Length, LinearTemperatureCoefficient,
    QuadraticTemperatureCoefficient, Ratio, Resistance, Temperature, TemperatureInterval, UnitExt,
};
use crate::model::ModelResolver;
use crate::model::res::{ResistorIdealModel, ResistorModel};
use crate::netlist::{CircuitReference, IntoNodeIdentifier, Netlist, NodeIdentifier};
use crate::state::CircuitState;
use num_complex::Complex;
use std::any::Any;
use std::sync::Arc;

pub(crate) struct ResistorSpec {
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
    pub(crate) fn new(
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
                .unwrap_or_else(|| Arc::new(ResistorIdealModel::new())),
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

pub trait ResistorFactory {
    fn resistor(
        &mut self,
        name: &str,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        resistance: impl IntoOptionalParameter<Resistance>,
    ) -> &mut ResistorSpec;
}

pub struct Resistor {
    pub name: String,
    pub model: Arc<ResistorModel>,
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

impl DcAnalysis for Resistor {
    fn load_dc(&self, context: &Context) -> Vec<Stamp<CircuitReference, f64>> {
        vec![
            Stamp::Matrix(
                self.node_plus.clone(),
                self.node_plus.clone(),
                self.conductance.value,
            ),
            Stamp::Matrix(
                self.node_minus.clone(),
                self.node_minus.clone(),
                self.conductance.value,
            ),
            Stamp::Matrix(
                self.node_plus.clone(),
                self.node_minus.clone(),
                -self.conductance.value,
            ),
            Stamp::Matrix(
                self.node_minus.clone(),
                self.node_plus.clone(),
                -self.conductance.value,
            ),
        ]
    }
}

impl TransientAnalysis for Resistor {
    fn load_transient(
        &self,
        _: &CircuitState<f64>,
        _: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        self.load_dc(context)
    }
}

impl AcAnalysis for Resistor {
    fn load_ac(
        &self,
        _circuit_states: &CircuitState<Complex<f64>>,
        _: &AcAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, Complex<f64>>> {
        let g = self.conductance.to_admittance();

        vec![
            Stamp::Matrix(self.node_plus.clone(), self.node_plus.clone(), g.value),
            Stamp::Matrix(self.node_minus.clone(), self.node_minus.clone(), g.value),
            Stamp::Matrix(self.node_plus.clone(), self.node_minus.clone(), -g.value),
            Stamp::Matrix(self.node_minus.clone(), self.node_plus.clone(), -g.value),
        ]
    }
}
