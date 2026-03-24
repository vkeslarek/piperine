use crate::node::Node;
use crate::spice::SpiceElement;
use std::collections::HashSet;

/// A composable group of circuit elements that flattens when composed into a Circuit.
///
/// `SubCircuit` mirrors the [`Circuit`](crate::circuit::Circuit) interface so the
/// experience of building a subcircuit is nearly identical to building a circuit.
///
/// `SubCircuit` does NOT generate `.subckt` in SPICE — it is purely a Rust-level
/// composition mechanism. Use regular functions to create reusable subcircuits:
///
/// ```ignore
/// fn half_bridge(high: Node, mid: Node, low: Node) -> SubCircuit {
///     let mut sc = SubCircuit::new();
///     sc.add(Mosfet::new("mh", mid.clone(), /* ... */))
///       .add(Mosfet::new("ml", low.clone(), /* ... */));
///     sc
/// }
///
/// circuit.compose_as("hb1", half_bridge(vdd, sw, gnd));
/// ```
#[derive(Debug)]
pub struct SubCircuit {
    elements: Vec<Box<dyn SpiceElement>>,
    internal_nodes: HashSet<Node>,
    params: Vec<(String, String)>,
    initial_conditions: Vec<(Node, f64)>,
    node_counter: usize,
    compose_counter: usize,
}

impl SubCircuit {
    pub fn new() -> Self {
        Self {
            elements: Vec::new(),
            internal_nodes: HashSet::new(),
            params: Vec::new(),
            initial_conditions: Vec::new(),
            node_counter: 0,
            compose_counter: 0,
        }
    }

    // ===== Node creation =====

    /// Generates an internal node with a unique name.
    ///
    /// Internal nodes are automatically prefixed when this SubCircuit is
    /// composed into a Circuit or another SubCircuit via `compose_as`.
    pub fn internal_node(&mut self) -> Node {
        self.node_counter += 1;
        let node = Node::named(format!("n{}", self.node_counter));
        self.internal_nodes.insert(node.clone());
        node
    }

    // ===== Adding elements =====

    /// Adds a device to this subcircuit.
    pub fn add(&mut self, element: impl SpiceElement + 'static) -> &mut Self {
        self.elements.push(Box::new(element));
        self
    }

    /// Composes another SubCircuit into this one with an explicit prefix.
    ///
    /// Element names and internal nodes of `other` are prefixed with `prefix_`.
    /// External nodes (not in `other.internal_nodes`) are left unchanged.
    pub fn compose_as(&mut self, prefix: &str, other: SubCircuit) -> &mut Self {
        let prefixed = other.flatten_with_prefix(prefix);
        self.elements.extend(prefixed);
        self
    }

    /// Composes another SubCircuit with an auto-generated prefix.
    pub fn compose(&mut self, other: SubCircuit) -> &mut Self {
        self.compose_counter += 1;
        let prefix = format!("sc{}", self.compose_counter);
        self.compose_as(&prefix, other)
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

    // ===== Accessors =====

    /// Returns the internal nodes tracked by this subcircuit.
    pub fn internal_nodes(&self) -> &HashSet<Node> {
        &self.internal_nodes
    }

    pub fn params(&self) -> &[(String, String)] {
        &self.params
    }

    pub fn initial_conditions(&self) -> &[(Node, f64)] {
        &self.initial_conditions
    }

    /// Consumes this SubCircuit and returns its elements with prefixed names/nodes.
    pub(crate) fn flatten_with_prefix(self, prefix: &str) -> Vec<Box<dyn SpiceElement>> {
        self.elements.into_iter().map(|elem| {
            let wrapper = PrefixedElement {
                inner: elem,
                prefix: prefix.to_string(),
                internal_nodes: self.internal_nodes.clone(),
            };
            Box::new(wrapper) as Box<dyn SpiceElement>
        }).collect()
    }

    /// Consumes this SubCircuit and returns its raw elements (no prefix).
    pub(crate) fn into_elements(self) -> Vec<Box<dyn SpiceElement>> {
        self.elements
    }
}

impl Default for SubCircuit {
    fn default() -> Self {
        Self::new()
    }
}

/// Wrapper that applies a namespace prefix to an element's SPICE output.
struct PrefixedElement {
    inner: Box<dyn SpiceElement>,
    prefix: String,
    internal_nodes: HashSet<Node>,
}

impl std::fmt::Debug for PrefixedElement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrefixedElement")
            .field("prefix", &self.prefix)
            .field("inner", &self.inner)
            .finish()
    }
}

impl crate::spice::SpiceComponent for PrefixedElement {
    fn into_spice(&self) -> String {
        // Get original SPICE line and prefix the element name.
        // The element name is always the first token (e.g. "RR1 in out 10k").
        let original = self.inner.into_spice();
        let original_name = self.inner.element_name();
        let symbol = self.inner.element_ref().symbol;

        // Replace the first occurrence of "Symbol+Name" with "Symbol+Prefix_Name"
        let full_original = format!("{}{}", symbol, original_name);
        let full_prefixed = format!("{}{}_{}", symbol, self.prefix, original_name);
        let line = original.replacen(&full_original, &full_prefixed, 1);

        // Prefix internal nodes
        let mut result = line;
        for internal in &self.internal_nodes {
            let old_name = internal.spice_name();
            let new_name = format!("{}_{}", self.prefix, old_name);
            result = result.replace(old_name, &new_name);
        }

        result
    }
}

impl SpiceElement for PrefixedElement {
    fn element_name(&self) -> &str {
        // Note: this returns the UN-prefixed name. The prefixed name is in into_spice().
        self.inner.element_name()
    }

    fn spice_model(&self) -> Option<std::sync::Arc<dyn crate::spice::SpiceModel>> {
        self.inner.spice_model()
    }

    fn element_ref(&self) -> crate::spice::ElementRef {
        let inner_ref = self.inner.element_ref();
        crate::spice::ElementRef {
            symbol: inner_ref.symbol,
            name: format!("{}_{}", self.prefix, inner_ref.name),
        }
    }
}
