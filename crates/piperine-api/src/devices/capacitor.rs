use crate::devices::Component;
use crate::node::Node;
use crate::num::Dynamic;
use crate::spice::{ElementRef, SpiceComponent, SpiceElement};
use crate::units::{Celsius, Dimensionless, Farad, Meter, PerCelsius, PerCelsiusSquared, Volt};
use std::sync::Arc;

/// Two-terminal capacitor (`n+`, `n-`).
///
/// Supports both fixed-value and semiconductor (geometric) capacitors.
/// All parameters match the ngspice manual §3.3.6–3.3.7.
#[derive(Debug)]
pub struct Capacitor {
    name: String,
    node_plus: Node,
    node_minus: Node,
    /// Capacitance value in Farads.
    capacitance: Dynamic<Farad>,
    /// Optional model for semiconductor capacitor.
    model: Option<Arc<dyn crate::models::capacitor::CapacitorModel + Send + Sync>>,
    /// Initial condition: voltage across capacitor at t=0 (requires `uic` on `.tran`).
    ic: Option<Dynamic<Volt>>,
    /// Physical length (semiconductor capacitor).
    length: Option<Dynamic<Meter>>,
    /// Physical width (semiconductor capacitor). Default in models: DEFW = 1e-6.
    width: Option<Dynamic<Meter>>,
    /// Geometric scaling factor.
    scale: Dynamic<Dimensionless>,
    /// Instance multiplier (parallel instances).
    multiplier: Dynamic<Dimensionless>,
    /// Absolute operating temperature.
    temp: Option<Dynamic<Celsius>>,
    /// Relative temperature offset from circuit temperature.
    delta_temp: Option<Dynamic<Celsius>>,
    /// First-order temperature coefficient (F/°C).
    tc1: Option<Dynamic<PerCelsius>>,
    /// Second-order temperature coefficient (F/°C²).
    tc2: Option<Dynamic<PerCelsiusSquared>>,
}

impl Clone for Capacitor {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            node_plus: self.node_plus,
            node_minus: self.node_minus,
            capacitance: self.capacitance.clone(),
            model: self.model.as_ref().map(Arc::clone),
            ic: self.ic.clone(),
            length: self.length.clone(),
            width: self.width.clone(),
            scale: self.scale.clone(),
            multiplier: self.multiplier.clone(),
            temp: self.temp.clone(),
            delta_temp: self.delta_temp.clone(),
            tc1: self.tc1.clone(),
            tc2: self.tc2.clone(),
        }
    }
}

impl Capacitor {
    pub const SYMBOL: &str = "C";
    pub const DEFAULT_SCALE: Dimensionless = 1.0;
    pub const DEFAULT_MULTIPLIER: Dimensionless = 1.0;

    /// Creates a new capacitor with required capacitance value.
    pub fn new(
        name: impl Into<String>,
        node_plus: impl Into<Node>,
        node_minus: impl Into<Node>,
        capacitance: impl Into<Dynamic<Farad>>,
    ) -> Self {
        Self {
            name: name.into(),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            capacitance: capacitance.into(),
            model: None,
            ic: None,
            length: None,
            width: None,
            scale: Self::DEFAULT_SCALE.into(),
            multiplier: Self::DEFAULT_MULTIPLIER.into(),
            temp: None,
            delta_temp: None,
            tc1: None,
            tc2: None,
        }
    }

    /// Sets the model for semiconductor capacitor.
    pub fn with_model(
        &mut self,
        model: Arc<dyn crate::models::capacitor::CapacitorModel + Send + Sync>,
    ) -> &mut Self {
        self.model = Some(model);
        self
    }

    /// Sets the initial condition voltage (effective only with `uic` on `.tran`).
    pub fn with_ic(&mut self, voltage: impl Into<Dynamic<Volt>>) -> &mut Self {
        self.ic = Some(voltage.into());
        self
    }

    /// Sets physical dimensions for semiconductor capacitor.
    pub fn with_dimensions(
        &mut self,
        length: impl Into<Dynamic<Meter>>,
        width: impl Into<Dynamic<Meter>>,
    ) -> &mut Self {
        self.length = Some(length.into());
        self.width = Some(width.into());
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

    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn node_plus(&self) -> &Node {
        &self.node_plus
    }
    pub fn node_minus(&self) -> &Node {
        &self.node_minus
    }
    pub fn nodes(&self) -> (&Node, &Node) {
        (&self.node_plus, &self.node_minus)
    }
    pub fn capacitance(&self) -> &Dynamic<Farad> {
        &self.capacitance
    }
    pub fn model_name(&self) -> Option<&str> {
        self.model.as_ref().map(|m| m.model_name())
    }
    pub fn ic(&self) -> Option<&Dynamic<Volt>> {
        self.ic.as_ref()
    }
    pub fn length(&self) -> Option<&Dynamic<Meter>> {
        self.length.as_ref()
    }
    pub fn width(&self) -> Option<&Dynamic<Meter>> {
        self.width.as_ref()
    }
    pub fn scale(&self) -> &Dynamic<Dimensionless> {
        &self.scale
    }
    pub fn multiplier(&self) -> &Dynamic<Dimensionless> {
        &self.multiplier
    }
    pub fn temp(&self) -> Option<&Dynamic<Celsius>> {
        self.temp.as_ref()
    }
    pub fn delta_temp(&self) -> Option<&Dynamic<Celsius>> {
        self.delta_temp.as_ref()
    }
    pub fn tc1(&self) -> Option<&Dynamic<PerCelsius>> {
        self.tc1.as_ref()
    }
    pub fn tc2(&self) -> Option<&Dynamic<PerCelsiusSquared>> {
        self.tc2.as_ref()
    }
}

impl Component for Capacitor {}

impl SpiceElement for Capacitor {
    fn element_name(&self) -> &str {
        &self.name
    }

    fn element_ref(&self) -> ElementRef {
        ElementRef::new(Self::SYMBOL, &self.name)
    }

    fn spice_model(&self) -> Option<Arc<dyn crate::spice::SpiceModel>> {
        self.model
            .as_ref()
            .map(|m| Arc::clone(m) as Arc<dyn crate::spice::SpiceModel>)
    }
}

impl SpiceComponent for Capacitor {
    fn into_spice(&self) -> String {
        let mut s = format!(
            "{}{} {} {} {}",
            Self::SYMBOL,
            self.name(),
            self.node_plus(),
            self.node_minus(),
            self.capacitance()
        );

        if let Some(model_name) = self.model.as_ref().map(|m| m.model_name()) {
            s.push_str(&format!(" {}", model_name));
        }
        if let Some(l) = &self.length {
            s.push_str(&format!(" L={}", l));
        }
        if let Some(w) = &self.width {
            s.push_str(&format!(" W={}", w));
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
