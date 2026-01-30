use crate::circuit::netlist::{ComponentIdentifier, NodeIdentifier};
use crate::devices::{Component, Dynamic, Model};
use crate::unit::{Ampere, Celsius, Dimensionless, Henry, Meter, MeterSquared};
use std::sync::Arc;

pub struct Inductor {
    name: String,
    model: Arc<InductorModel>,
    node_plus: NodeIdentifier,
    node_minus: NodeIdentifier,
    value: Option<Dynamic<Henry>>,
    number_turns: Option<Dimensionless>,
    multiplier: Option<Dimensionless>,
    scale: Option<Dimensionless>,
    temp: Option<Celsius>,
    delta_temp: Option<Celsius>,
    tc1: Option<Dimensionless>,
    tc2: Option<Dimensionless>,
    ic: Option<Ampere>,
}

impl Inductor {
    pub fn new(
        name: impl Into<String>,
        node_plus: impl Into<NodeIdentifier>,
        node_minus: impl Into<NodeIdentifier>,
        value: impl Into<Option<Dynamic<Henry>>>,
    ) -> Self {
        Self {
            name: name.into(),
            model: Arc::new(InductorModel::default()),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            value: None,
            number_turns: None,
            multiplier: None,
            scale: None,
            temp: None,
            delta_temp: None,
            tc1: None,
            tc2: None,
            ic: None,
        }
    }
    pub fn with_model(&mut self, model: Arc<InductorModel>) -> &mut Self {
        self.model = model;
        self
    }

    pub fn with_value(&mut self, value: impl Into<Dynamic<Henry>>) -> &mut Self {
        self.value = Some(value.into());
        self
    }

    pub fn with_number_turns(&mut self, number_turns: impl Into<Dimensionless>) -> &mut Self {
        self.number_turns = Some(number_turns.into());
        self
    }

    pub fn with_multiplier(&mut self, multiplier: impl Into<Dimensionless>) -> &mut Self {
        self.multiplier = Some(multiplier.into());
        self
    }

    pub fn with_scale(&mut self, scale: impl Into<Dimensionless>) -> &mut Self {
        self.scale = Some(scale.into());
        self
    }

    pub fn with_temp(&mut self, temp: impl Into<Celsius>) -> &mut Self {
        self.temp = Some(temp.into());
        self
    }

    pub fn with_delta_temp(&mut self, delta_temp: impl Into<Celsius>) -> &mut Self {
        self.delta_temp = Some(delta_temp.into());
        self
    }

    pub fn with_tc1(&mut self, tc1: impl Into<Dimensionless>) -> &mut Self {
        self.tc1 = Some(tc1.into());
        self
    }

    pub fn with_tc2(&mut self, tc2: impl Into<Dimensionless>) -> &mut Self {
        self.tc2 = Some(tc2.into());
        self
    }

    pub fn with_initial_condition(&mut self, ic: impl Into<Ampere>) -> &mut Self {
        self.ic = Some(ic.into());
        self
    }

    pub fn model(&self) -> &Arc<InductorModel> {
        &self.model
    }

    pub fn node_plus(&self) -> &NodeIdentifier {
        &self.node_plus
    }

    pub fn node_minus(&self) -> &NodeIdentifier {
        &self.node_minus
    }

    pub fn value(&self) -> Option<&Dynamic<Henry>> {
        self.value.as_ref()
    }

    pub fn number_turns(&self) -> Option<Dimensionless> {
        self.number_turns
    }

    pub fn multiplier(&self) -> Option<Dimensionless> {
        self.multiplier
    }

    pub fn scale(&self) -> Option<Dimensionless> {
        self.scale
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

    pub fn initial_condition(&self) -> Option<Ampere> {
        self.ic
    }
}

impl Component for Inductor {
    fn name(&self) -> &String {
        &self.name
    }
}

pub struct CoupledInductor {
    name: String,
    inductor_1: ComponentIdentifier,
    inductor_2: ComponentIdentifier,
    value: Dimensionless,
}

impl CoupledInductor {
    pub fn new(
        name: impl Into<String>,
        inductor_1: impl Into<ComponentIdentifier>,
        inductor_2: impl Into<ComponentIdentifier>,
        value: Dimensionless,
    ) -> Self {
        Self {
            name: name.into(),
            inductor_1: inductor_1.into(),
            inductor_2: inductor_2.into(),
            value,
        }
    }

    pub fn inductor_1(&self) -> &ComponentIdentifier {
        &self.inductor_1
    }

    pub fn inductor_2(&self) -> &ComponentIdentifier {
        &self.inductor_2
    }

    pub fn value(&self) -> Dimensionless {
        self.value
    }
}

impl Component for CoupledInductor {
    fn name(&self) -> &String {
        &self.name
    }
}

pub struct InductorModel {
    value: Option<Dynamic<Henry>>,
    cross_section: MeterSquared,
    coil_diameter: Meter,
    length: Meter,
    tc1: Dimensionless,
    tc2: Dimensionless,
    tnom: Celsius,
    number_turns: Dimensionless,
    magnetic_permeativity: Dimensionless,
}

impl Default for InductorModel {
    fn default() -> Self {
        Self {
            value: None,
            cross_section: 0.0,
            coil_diameter: 0.0,
            length: 0.0,
            tc1: 0.0,
            tc2: 0.0,
            tnom: 27.0,
            number_turns: 0.0,
            magnetic_permeativity: 1.0,
        }
    }
}

impl InductorModel {
    pub fn with_value(&mut self, value: impl Into<Dynamic<Henry>>) -> &mut Self {
        self.value = Some(value.into());
        self
    }

    pub fn with_cross_section(&mut self, cross_section: impl Into<MeterSquared>) -> &mut Self {
        self.cross_section = cross_section.into();
        self
    }

    pub fn with_coil_diameter(&mut self, coil_diameter: impl Into<Meter>) -> &mut Self {
        self.coil_diameter = coil_diameter.into();
        self
    }

    pub fn with_length(&mut self, length: impl Into<Meter>) -> &mut Self {
        self.length = length.into();
        self
    }

    pub fn with_tc1(&mut self, tc1: impl Into<Dimensionless>) -> &mut Self {
        self.tc1 = tc1.into();
        self
    }

    pub fn with_tc2(&mut self, tc2: impl Into<Dimensionless>) -> &mut Self {
        self.tc2 = tc2.into();
        self
    }

    pub fn with_tnom(&mut self, tnom: impl Into<Celsius>) -> &mut Self {
        self.tnom = tnom.into();
        self
    }

    pub fn with_number_turns(&mut self, number_turns: impl Into<Dimensionless>) -> &mut Self {
        self.number_turns = number_turns.into();
        self
    }

    pub fn with_magnetic_permeativity(&mut self, mu: impl Into<Dimensionless>) -> &mut Self {
        self.magnetic_permeativity = mu.into();
        self
    }

    pub fn value(&self) -> Option<&Dynamic<Henry>> {
        self.value.as_ref()
    }

    pub fn cross_section(&self) -> MeterSquared {
        self.cross_section
    }

    pub fn coil_diameter(&self) -> Meter {
        self.coil_diameter
    }

    pub fn length(&self) -> Meter {
        self.length
    }

    pub fn tc1(&self) -> Dimensionless {
        self.tc1
    }

    pub fn tc2(&self) -> Dimensionless {
        self.tc2
    }

    pub fn tnom(&self) -> Celsius {
        self.tnom
    }

    pub fn number_turns(&self) -> Dimensionless {
        self.number_turns
    }

    pub fn magnetic_permeativity(&self) -> Dimensionless {
        self.magnetic_permeativity
    }
}

impl Model for InductorModel {
    type ComponentType = Inductor;
}
