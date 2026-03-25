use crate::devices::Component;
use crate::node::Node;
use crate::num::Dynamic;
use crate::spice::{ElementRef, SpiceComponent, SpiceElement};
use crate::units::{Dimensionless, Siemens};

/// Voltage-Controlled Current Source (`G`).
///
/// `GXXXX n+ n- nc+ nc- transconductance <m=val>`
/// See ngspice manual §4.2.1.
#[derive(Debug, Clone)]
pub struct Vccs {
    name: String,
    node_plus: Node,
    node_minus: Node,
    ctrl_plus: Node,
    ctrl_minus: Node,
    /// Transconductance (A/V = Siemens).
    transconductance: Dynamic<Siemens>,
    /// Optional multiplier.
    multiplier: Option<Dynamic<Dimensionless>>,
}

impl Vccs {
    pub const SYMBOL: &str = "G";

    pub fn new(
        name: impl Into<String>,
        node_plus: impl Into<Node>,
        node_minus: impl Into<Node>,
        ctrl_plus: impl Into<Node>,
        ctrl_minus: impl Into<Node>,
        transconductance: impl Into<Dynamic<Siemens>>,
    ) -> Self {
        Self {
            name: name.into(),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            ctrl_plus: ctrl_plus.into(),
            ctrl_minus: ctrl_minus.into(),
            transconductance: transconductance.into(),
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
    pub fn ctrl_plus(&self) -> &Node {
        &self.ctrl_plus
    }
    pub fn ctrl_minus(&self) -> &Node {
        &self.ctrl_minus
    }
    pub fn transconductance(&self) -> &Dynamic<Siemens> {
        &self.transconductance
    }
    pub fn multiplier(&self) -> Option<&Dynamic<Dimensionless>> {
        self.multiplier.as_ref()
    }
}

impl Component for Vccs {}

impl SpiceElement for Vccs {
    fn element_name(&self) -> &str {
        &self.name
    }

    fn element_ref(&self) -> ElementRef {
        ElementRef::new(Self::SYMBOL, &self.name)
    }
}

impl SpiceComponent for Vccs {
    fn into_spice(&self) -> String {
        let mut s = format!(
            "{}{} {} {} {} {} {}",
            Self::SYMBOL,
            self.name(),
            self.node_plus(),
            self.node_minus(),
            self.ctrl_plus(),
            self.ctrl_minus(),
            self.transconductance()
        );
        if let Some(m) = &self.multiplier {
            s.push_str(&format!(" m={}", m));
        }
        s
    }
}
