use crate::devices::Component;
use crate::node::Node;
use crate::num::Dynamic;
use crate::spice::{SpiceElement, ElementRef, SpiceComponent};
use crate::units::{Ampere, Celsius, Dimensionless, Henry, PerCelsius, PerCelsiusSquared};
use std::sync::Arc;

/// Two-terminal inductor (`n+`, `n-`).
///
/// All parameters match the ngspice manual §3.3.10.
#[derive(Debug)]
pub struct Inductor {
    name: String,
    node_plus: Node,
    node_minus: Node,
    /// Inductance value in Henries.
    inductance: Dynamic<Henry>,
    /// Optional model.
    model: Option<Arc<dyn crate::models::inductor::InductorModel + Send + Sync>>,
    /// Initial condition: current through inductor at t=0 (requires `uic` on `.tran`).
    ic: Option<Dynamic<Ampere>>,
    /// Number of turns (used with models for geometric inductance).
    nt: Option<Dynamic<Dimensionless>>,
    /// Geometric scaling factor.
    scale: Dynamic<Dimensionless>,
    /// Instance multiplier (parallel instances). Note: Lnom = value·scale/m.
    multiplier: Dynamic<Dimensionless>,
    /// Absolute operating temperature.
    temp: Option<Dynamic<Celsius>>,
    /// Relative temperature offset from circuit temperature.
    delta_temp: Option<Dynamic<Celsius>>,
    /// First-order temperature coefficient (H/°C).
    tc1: Option<Dynamic<PerCelsius>>,
    /// Second-order temperature coefficient (H/°C²).
    tc2: Option<Dynamic<PerCelsiusSquared>>,
}

impl Clone for Inductor {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            node_plus: self.node_plus.clone(),
            node_minus: self.node_minus.clone(),
            inductance: self.inductance.clone(),
            model: self.model.as_ref().map(Arc::clone),
            ic: self.ic.clone(),
            nt: self.nt.clone(),
            scale: self.scale.clone(),
            multiplier: self.multiplier.clone(),
            temp: self.temp.clone(),
            delta_temp: self.delta_temp.clone(),
            tc1: self.tc1.clone(),
            tc2: self.tc2.clone(),
        }
    }
}

impl Inductor {
    pub const SYMBOL: &str = "L";
    pub const DEFAULT_SCALE: Dimensionless = 1.0;
    pub const DEFAULT_MULTIPLIER: Dimensionless = 1.0;

    /// Creates a new inductor with required inductance value.
    pub fn new(
        name: impl Into<String>,
        node_plus: impl Into<Node>,
        node_minus: impl Into<Node>,
        inductance: impl Into<Dynamic<Henry>>,
    ) -> Self {
        Self {
            name: name.into(),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            inductance: inductance.into(),
            model: None,
            ic: None,
            nt: None,
            scale: Self::DEFAULT_SCALE.into(),
            multiplier: Self::DEFAULT_MULTIPLIER.into(),
            temp: None,
            delta_temp: None,
            tc1: None,
            tc2: None,
        }
    }

    /// Sets the model for geometric inductor.
    pub fn with_model(&mut self, model: Arc<dyn crate::models::inductor::InductorModel + Send + Sync>) -> &mut Self {
        self.model = Some(model);
        self
    }

    /// Sets the initial condition current (effective only with `uic` on `.tran`).
    pub fn with_ic(&mut self, current: impl Into<Dynamic<Ampere>>) -> &mut Self {
        self.ic = Some(current.into());
        self
    }

    /// Sets the number of turns (used with models).
    pub fn with_nt(&mut self, turns: impl Into<Dynamic<Dimensionless>>) -> &mut Self {
        self.nt = Some(turns.into());
        self
    }

    /// Sets the scale factor.
    pub fn with_scale(&mut self, value: impl Into<Dynamic<Dimensionless>>) -> &mut Self {
        self.scale = value.into();
        self
    }

    /// Sets the instance multiplier.
    pub fn with_multiplier(&mut self, value: impl Into<Dynamic<Dimensionless>>) -> &mut Self {
        self.multiplier = value.into();
        self
    }

    /// Sets the absolute operating temperature.
    pub fn with_temp(&mut self, value: impl Into<Dynamic<Celsius>>) -> &mut Self {
        self.temp = Some(value.into());
        self
    }

    /// Sets the relative temperature offset.
    pub fn with_delta_temp(&mut self, value: impl Into<Dynamic<Celsius>>) -> &mut Self {
        self.delta_temp = Some(value.into());
        self
    }

    /// Sets the temperature coefficients TC1 and TC2.
    pub fn with_temperature_coefficients(
        &mut self,
        tc1: impl Into<Dynamic<PerCelsius>>,
        tc2: impl Into<Dynamic<PerCelsiusSquared>>,
    ) -> &mut Self {
        self.tc1 = Some(tc1.into());
        self.tc2 = Some(tc2.into());
        self
    }

    pub fn name(&self) -> &str { &self.name }
    pub fn node_plus(&self) -> &Node { &self.node_plus }
    pub fn node_minus(&self) -> &Node { &self.node_minus }
    pub fn nodes(&self) -> (&Node, &Node) { (&self.node_plus, &self.node_minus) }
    pub fn inductance(&self) -> &Dynamic<Henry> { &self.inductance }
    pub fn model_name(&self) -> Option<&str> { self.model.as_ref().map(|m| m.model_name()) }
    pub fn ic(&self) -> Option<&Dynamic<Ampere>> { self.ic.as_ref() }
    pub fn nt(&self) -> Option<&Dynamic<Dimensionless>> { self.nt.as_ref() }
    pub fn scale(&self) -> &Dynamic<Dimensionless> { &self.scale }
    pub fn multiplier(&self) -> &Dynamic<Dimensionless> { &self.multiplier }
    pub fn temp(&self) -> Option<&Dynamic<Celsius>> { self.temp.as_ref() }
    pub fn delta_temp(&self) -> Option<&Dynamic<Celsius>> { self.delta_temp.as_ref() }
    pub fn tc1(&self) -> Option<&Dynamic<PerCelsius>> { self.tc1.as_ref() }
    pub fn tc2(&self) -> Option<&Dynamic<PerCelsiusSquared>> { self.tc2.as_ref() }
}

impl Component for Inductor {}

impl SpiceElement for Inductor {
    fn element_name(&self) -> &str {
        &self.name
    }

    fn element_ref(&self) -> ElementRef {
        ElementRef::new(Self::SYMBOL, &self.name)
    }

    fn spice_model(&self) -> Option<Arc<dyn crate::spice::SpiceModel>> {
        self.model.as_ref().map(|m| Arc::clone(m) as Arc<dyn crate::spice::SpiceModel>)
    }
}

impl SpiceComponent for Inductor {
    fn into_spice(&self) -> String {
        let mut s = format!(
            "{}{} {} {} {}",
            Self::SYMBOL, self.name(), self.node_plus(), self.node_minus(), self.inductance()
        );

        if let Some(model_name) = self.model.as_ref().map(|m| m.model_name()) {
            s.push_str(&format!(" {}", model_name));
        }
        if let Some(nt) = &self.nt {
            s.push_str(&format!(" NT={}", nt));
        }
        if let Dynamic::Literal(v) = &self.multiplier {
            if *v != Self::DEFAULT_MULTIPLIER {
                s.push_str(&format!(" M={}", self.multiplier));
            }
        }
        if let Dynamic::Literal(v) = &self.scale {
            if *v != Self::DEFAULT_SCALE {
                s.push_str(&format!(" SCALE={}", self.scale));
            }
        }
        if let Some(temp) = &self.temp {
            s.push_str(&format!(" TEMP={}", temp));
        }
        if let Some(dt) = &self.delta_temp {
            s.push_str(&format!(" DTEMP={}", dt));
        }
        if let Some(tc1) = &self.tc1 {
            s.push_str(&format!(" TC1={}", tc1));
        }
        if let Some(tc2) = &self.tc2 {
            s.push_str(&format!(" TC2={}", tc2));
        }
        if let Some(ic) = &self.ic {
            s.push_str(&format!(" IC={}", ic));
        }

        s
    }
}
