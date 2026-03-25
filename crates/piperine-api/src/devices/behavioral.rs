use crate::devices::Component;
use crate::node::Node;
use crate::spice::{ElementRef, SpiceComponent, SpiceElement};
use crate::units::Celsius;

/// Behavioral source output type.
#[derive(Debug, Clone)]
pub enum BehavioralKind {
    /// `V=expression` — voltage source.
    Voltage(String),
    /// `I=expression` — current source.
    Current(String),
}

/// Non-linear dependent source / behavioral source (`B`).
///
/// `BXXXX n+ n- <i=expr> <v=expr> <tc1=val> <tc2=val> <temp=val> <dtemp=val>`
/// See ngspice manual Chapter 5, §5.1.
#[derive(Debug, Clone)]
pub struct BehavioralSource {
    name: String,
    node_plus: Node,
    node_minus: Node,
    /// V= or I= expression.
    kind: BehavioralKind,
    /// TC1: First order temperature coefficient.
    tc1: Option<f64>,
    /// TC2: Second order temperature coefficient.
    tc2: Option<f64>,
    /// TEMP: Absolute temperature.
    temp: Option<Celsius>,
    /// DTEMP: Relative temperature offset.
    delta_temp: Option<Celsius>,
}

impl BehavioralSource {
    pub const SYMBOL: &str = "B";

    /// Creates a behavioral voltage source.
    pub fn voltage(
        name: impl Into<String>,
        node_plus: impl Into<Node>,
        node_minus: impl Into<Node>,
        expression: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            kind: BehavioralKind::Voltage(expression.into()),
            tc1: None,
            tc2: None,
            temp: None,
            delta_temp: None,
        }
    }

    /// Creates a behavioral current source.
    pub fn current(
        name: impl Into<String>,
        node_plus: impl Into<Node>,
        node_minus: impl Into<Node>,
        expression: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            kind: BehavioralKind::Current(expression.into()),
            tc1: None,
            tc2: None,
            temp: None,
            delta_temp: None,
        }
    }

    pub fn with_tc(&mut self, tc1: f64, tc2: f64) -> &mut Self {
        self.tc1 = Some(tc1);
        self.tc2 = Some(tc2);
        self
    }

    pub fn with_temp(&mut self, temp: Celsius) -> &mut Self {
        self.temp = Some(temp);
        self
    }

    pub fn with_delta_temp(&mut self, dt: Celsius) -> &mut Self {
        self.delta_temp = Some(dt);
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
    pub fn kind(&self) -> &BehavioralKind {
        &self.kind
    }
    pub fn tc1(&self) -> Option<f64> {
        self.tc1
    }
    pub fn tc2(&self) -> Option<f64> {
        self.tc2
    }
    pub fn temp(&self) -> Option<Celsius> {
        self.temp
    }
    pub fn delta_temp(&self) -> Option<Celsius> {
        self.delta_temp
    }
}

impl Component for BehavioralSource {}

impl SpiceElement for BehavioralSource {
    fn element_name(&self) -> &str {
        &self.name
    }

    fn element_ref(&self) -> ElementRef {
        ElementRef::new(Self::SYMBOL, &self.name)
    }
}

impl SpiceComponent for BehavioralSource {
    fn into_spice(&self) -> String {
        let mut s = format!(
            "{}{} {} {}",
            Self::SYMBOL,
            self.name(),
            self.node_plus(),
            self.node_minus()
        );

        match &self.kind {
            BehavioralKind::Voltage(expr) => s.push_str(&format!(" V={}", expr)),
            BehavioralKind::Current(expr) => s.push_str(&format!(" I={}", expr)),
        }

        if let Some(tc1) = self.tc1 {
            s.push_str(&format!(" tc1={}", tc1));
        }
        if let Some(tc2) = self.tc2 {
            s.push_str(&format!(" tc2={}", tc2));
        }
        if let Some(temp) = self.temp {
            s.push_str(&format!(" temp={}", temp));
        }
        if let Some(dt) = self.delta_temp {
            s.push_str(&format!(" dtemp={}", dt));
        }

        s
    }
}
