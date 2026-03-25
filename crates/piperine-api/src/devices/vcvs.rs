use crate::devices::Component;
use crate::node::Node;
use crate::num::Dynamic;
use crate::spice::{ElementRef, SpiceComponent, SpiceElement};
use crate::units::Dimensionless;

/// Voltage-Controlled Voltage Source (`E`).
///
/// `EXXXX n+ n- nc+ nc- gain`
/// See ngspice manual §4.2.2.
#[derive(Debug, Clone)]
pub struct Vcvs {
    name: String,
    node_plus: Node,
    node_minus: Node,
    ctrl_plus: Node,
    ctrl_minus: Node,
    /// Voltage gain (dimensionless).
    gain: Dynamic<Dimensionless>,
}

impl Vcvs {
    pub const SYMBOL: &str = "E";

    pub fn new(
        name: impl Into<String>,
        node_plus: impl Into<Node>,
        node_minus: impl Into<Node>,
        ctrl_plus: impl Into<Node>,
        ctrl_minus: impl Into<Node>,
        gain: impl Into<Dynamic<Dimensionless>>,
    ) -> Self {
        Self {
            name: name.into(),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            ctrl_plus: ctrl_plus.into(),
            ctrl_minus: ctrl_minus.into(),
            gain: gain.into(),
        }
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
    pub fn gain(&self) -> &Dynamic<Dimensionless> {
        &self.gain
    }
}

impl Component for Vcvs {}

impl SpiceElement for Vcvs {
    fn element_name(&self) -> &str {
        &self.name
    }

    fn element_ref(&self) -> ElementRef {
        ElementRef::new(Self::SYMBOL, &self.name)
    }
}

impl SpiceComponent for Vcvs {
    fn into_spice(&self) -> String {
        format!(
            "{}{} {} {} {} {} {}",
            Self::SYMBOL,
            self.name(),
            self.node_plus(),
            self.node_minus(),
            self.ctrl_plus(),
            self.ctrl_minus(),
            self.gain()
        )
    }
}
