use crate::devices::Component;
use crate::node::Node;
use crate::num::Dynamic;
use crate::spice::{ElementRef, SpiceComponent, SpiceElement};
use crate::units::Dimensionless;

/// Current-Controlled Current Source (`F`).
///
/// `FXXXX n+ n- vname gain <m=val>`
/// See ngspice manual §4.2.3.
#[derive(Debug, Clone)]
pub struct Cccs {
    name: String,
    node_plus: Node,
    node_minus: Node,
    /// Name of the voltage source used as ammeter.
    v_source: String,
    /// Current gain (dimensionless).
    gain: Dynamic<Dimensionless>,
    /// Optional multiplier.
    multiplier: Option<Dynamic<Dimensionless>>,
}

impl Cccs {
    pub const SYMBOL: &str = "F";

    pub fn new(
        name: impl Into<String>,
        node_plus: impl Into<Node>,
        node_minus: impl Into<Node>,
        v_source: impl Into<String>,
        gain: impl Into<Dynamic<Dimensionless>>,
    ) -> Self {
        Self {
            name: name.into(),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            v_source: v_source.into(),
            gain: gain.into(),
            multiplier: None,
        }
    }

    pub fn with_multiplier(&mut self, m: impl Into<Dynamic<Dimensionless>>) -> &mut Self {
        self.multiplier = Some(m.into());
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
    pub fn v_source(&self) -> &str {
        &self.v_source
    }
    pub fn gain(&self) -> &Dynamic<Dimensionless> {
        &self.gain
    }
    pub fn multiplier(&self) -> Option<&Dynamic<Dimensionless>> {
        self.multiplier.as_ref()
    }
}

impl Component for Cccs {}

impl SpiceElement for Cccs {
    fn element_name(&self) -> &str {
        &self.name
    }

    fn element_ref(&self) -> ElementRef {
        ElementRef::new(Self::SYMBOL, &self.name)
    }
}

impl SpiceComponent for Cccs {
    fn into_spice(&self) -> String {
        let mut s = format!(
            "{}{} {} {} {} {}",
            Self::SYMBOL,
            self.name(),
            self.node_plus(),
            self.node_minus(),
            self.v_source(),
            self.gain()
        );
        if let Some(m) = &self.multiplier {
            s.push_str(&format!(" m={}", m));
        }
        s
    }
}
