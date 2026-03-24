use crate::devices::Component;
use crate::node::Node;
use crate::spice::{SpiceElement, ElementRef, SpiceComponent};

/// Subcircuit instance (`X`).
///
/// `XXXXX n1 n2 ... subckt_name <param1=val param2=val ...>`
#[derive(Debug, Clone)]
pub struct SubCircuitInstance {
    name: String,
    /// Subcircuit definition name.
    subcircuit_name: String,
    /// Connection nodes (order must match subcircuit port order).
    nodes: Vec<Node>,
    /// Optional parameter overrides.
    params: Vec<(String, String)>,
}

impl SubCircuitInstance {
    pub const SYMBOL: &str = "X";

    pub fn new(
        name: impl Into<String>,
        subcircuit_name: impl Into<String>,
        nodes: Vec<Node>,
    ) -> Self {
        Self {
            name: name.into(),
            subcircuit_name: subcircuit_name.into(),
            nodes,
            params: Vec::new(),
        }
    }

    pub fn with_param(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.params.push((key.into(), value.into()));
        self
    }

    pub fn name(&self) -> &str { &self.name }
    pub fn subcircuit_name(&self) -> &str { &self.subcircuit_name }
    pub fn nodes(&self) -> &[Node] { &self.nodes }
    pub fn params(&self) -> &[(String, String)] { &self.params }
}

impl Component for SubCircuitInstance {}

impl SpiceElement for SubCircuitInstance {
    fn element_name(&self) -> &str {
        &self.name
    }

    fn element_ref(&self) -> ElementRef {
        ElementRef::new(Self::SYMBOL, &self.name)
    }
}

impl SpiceComponent for SubCircuitInstance {
    fn into_spice(&self) -> String {
        let nodes_str: Vec<String> = self.nodes.iter().map(|n| n.to_string()).collect();
        let mut s = format!(
            "{}{} {} {}",
            Self::SYMBOL, self.name(),
            nodes_str.join(" "),
            self.subcircuit_name()
        );
        for (k, v) in &self.params {
            s.push_str(&format!(" {}={}", k, v));
        }
        s
    }
}
