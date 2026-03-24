use crate::node::Node;
use crate::spice::{SpiceElement, ToSpiceNetlist};
use crate::subcircuit::SubCircuit;
use std::collections::HashSet;

/// A circuit: pure topology and physical properties.
///
/// Does NOT contain analysis configuration — that lives in separate
/// analysis types ([`OpAnalysis`](crate::analysis::OpAnalysis),
/// [`TranAnalysis`](crate::analysis::TranAnalysis), etc.).
///
/// # Node creation
///
/// Nodes are opaque handles created by the circuit. The user never needs
/// to know SPICE node names.
///
/// ```ignore
/// let mut circuit = Circuit::new("My Circuit");
/// let vdd = circuit.node();
/// let out = circuit.node_label("out");  // label is for debug only
/// ```
#[derive(Debug)]
pub struct Circuit {
    title: String,
    elements: Vec<Box<dyn SpiceElement>>,
    nodes: HashSet<Node>,
    params: Vec<(String, String)>,
    initial_conditions: Vec<(Node, f64)>,
    compose_counter: usize,
}

impl Circuit {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            elements: Vec::new(),
            nodes: HashSet::new(),
            params: Vec::new(),
            initial_conditions: Vec::new(),
            compose_counter: 0,
        }
    }

    // ===== Node creation =====

    /// Creates a new opaque node with an auto-generated name.
    pub fn node(&mut self) -> Node {
        let node = Node::auto();
        self.nodes.insert(node.clone());
        node
    }

    /// Creates a new node with a debug label.
    /// The label is used as the SPICE node name for readability.
    pub fn node_label(&mut self, label: &str) -> Node {
        let node = Node::named(label);
        self.nodes.insert(node.clone());
        node
    }

    // ===== Adding elements =====

    /// Adds a device to the circuit.
    pub fn add(&mut self, element: impl SpiceElement + 'static) -> &mut Self {
        self.elements.push(Box::new(element));
        self
    }

    /// Composes a [`SubCircuit`] into this circuit, flattening its elements
    /// inline with an explicit prefix for namespace isolation.
    pub fn compose_as(&mut self, prefix: &str, sc: SubCircuit) -> &mut Self {
        let prefixed = sc.flatten_with_prefix(prefix);
        self.elements.extend(prefixed);
        self
    }

    /// Composes a [`SubCircuit`] with an auto-generated prefix.
    pub fn compose(&mut self, sc: SubCircuit) -> &mut Self {
        self.compose_counter += 1;
        let prefix = format!("x{}", self.compose_counter);
        self.compose_as(&prefix, sc)
    }

    // ===== Parameters =====

    pub fn param(&mut self, name: &str, value: impl std::fmt::Display) -> &mut Self {
        self.params.push((name.to_string(), value.to_string()));
        self
    }

    // ===== Initial conditions =====

    pub fn ic(&mut self, node: Node, voltage: f64) -> &mut Self {
        self.initial_conditions.push((node, voltage));
        self
    }

}

impl ToSpiceNetlist for Circuit {
    fn to_spice_netlist(&self) -> Vec<String> {
        let mut lines = Vec::new();

        // Title
        lines.push(self.title.clone());

        // .param
        for (k, v) in &self.params {
            lines.push(format!(".param {k}={v}"));
        }

        // .model lines (collected from elements, deduplicated by name)
        let mut seen_models = HashSet::new();
        for elem in &self.elements {
            if let Some(model) = elem.spice_model() {
                let name = model.model_name().to_string();
                if seen_models.insert(name) {
                    lines.push(model.to_spice_model_line());
                }
            }
        }

        // Component instance lines
        for comp in &self.elements {
            lines.push(comp.into_spice());
        }

        // .ic
        if !self.initial_conditions.is_empty() {
            let ics: Vec<String> = self.initial_conditions.iter()
                .map(|(node, v)| format!("V({})={}", node, v))
                .collect();
            lines.push(format!(".ic {}", ics.join(" ")));
        }

        // .end
        lines.push(".end".to_string());

        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_circuit_structure() {
        let mut circuit = Circuit::new("Test Circuit");
        let _vdd = circuit.node_label("vdd");
        circuit.param("rval", "10k");

        let lines = circuit.to_netlist_lines();
        assert_eq!(lines[0], "Test Circuit");
        assert!(lines.contains(&".param rval=10k".to_string()));
        assert_eq!(lines.last().unwrap(), ".end");
    }

    #[test]
    fn initial_conditions() {
        let mut circuit = Circuit::new("IC Test");
        let out = circuit.node_label("out");
        circuit.ic(out, 2.5);

        let lines = circuit.to_netlist_lines();
        assert!(lines.iter().any(|l| l.starts_with(".ic V(out)=2.5")));
    }
}
