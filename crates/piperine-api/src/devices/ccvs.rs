use crate::devices::Component;
use crate::node::Node;
use crate::num::Dynamic;
use crate::spice::{SpiceElement, ElementRef, SpiceComponent};
use crate::units::Ohm;

/// Current-Controlled Voltage Source (`H`).
///
/// `HXXXX n+ n- vname transresistance`
/// See ngspice manual §4.2.4.
#[derive(Debug, Clone)]
pub struct Ccvs {
    name: String,
    node_plus: Node,
    node_minus: Node,
    /// Name of the voltage source used as ammeter.
    v_source: String,
    /// Transresistance (Ω = V/A).
    transresistance: Dynamic<Ohm>,
}

impl Ccvs {
    pub const SYMBOL: &str = "H";

    pub fn new(
        name: impl Into<String>,
        node_plus: impl Into<Node>,
        node_minus: impl Into<Node>,
        v_source: impl Into<String>,
        transresistance: impl Into<Dynamic<Ohm>>,
    ) -> Self {
        Self {
            name: name.into(),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            v_source: v_source.into(),
            transresistance: transresistance.into(),
        }
    }

    pub fn name(&self) -> &str { &self.name }
    pub fn node_plus(&self) -> &Node { &self.node_plus }
    pub fn node_minus(&self) -> &Node { &self.node_minus }
    pub fn v_source(&self) -> &str { &self.v_source }
    pub fn transresistance(&self) -> &Dynamic<Ohm> { &self.transresistance }
}

impl Component for Ccvs {}

impl SpiceElement for Ccvs {
    fn element_name(&self) -> &str {
        &self.name
    }

    fn element_ref(&self) -> ElementRef {
        ElementRef::new(Self::SYMBOL, &self.name)
    }
}

impl SpiceComponent for Ccvs {
    fn into_spice(&self) -> String {
        format!(
            "{}{} {} {} {} {}",
            Self::SYMBOL, self.name(),
            self.node_plus(), self.node_minus(),
            self.v_source(), self.transresistance()
        )
    }
}
