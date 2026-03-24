use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

/// Global node counter for auto-generated names.
static NODE_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Internal representation of a node identifier.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum NodeId {
    /// Ground reference (SPICE "0").
    Ground,
    /// Named node — either user-labeled or auto-generated.
    Named(String),
}

/// An opaque circuit node handle.
///
/// Nodes are the connection points between devices. They are created by
/// [`Circuit::node()`] or [`SubCircuit::internal_node()`] and passed to
/// device constructors. The user never needs to know the underlying SPICE
/// name — it is auto-generated internally.
///
/// # Ground
///
/// Use [`Node::GROUND`] for the ground reference node.
///
/// # Ergonomics
///
/// `From<&str>` and `From<usize>` are provided for convenience (e.g. in tests
/// or when interfacing with external `.lib` files), but the canonical way to
/// create nodes is via the circuit/subcircuit builders.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Node(NodeId);

impl Node {
    /// The ground reference node (SPICE "0").
    pub const GROUND: Node = Node(NodeId::Ground);

    /// Creates a node with an auto-generated unique name (`net_1`, `net_2`, …).
    pub(crate) fn auto() -> Self {
        let id = NODE_COUNTER.fetch_add(1, Ordering::Relaxed);
        Node(NodeId::Named(format!("net_{}", id)))
    }

    /// Creates a node with a user-provided label (used as the SPICE name).
    pub(crate) fn named(label: impl Into<String>) -> Self {
        Node(NodeId::Named(label.into()))
    }

    /// Returns `true` if this is the ground node.
    pub fn is_ground(&self) -> bool {
        matches!(self.0, NodeId::Ground)
    }

    /// Returns the SPICE name of this node.
    pub fn spice_name(&self) -> &str {
        match &self.0 {
            NodeId::Ground => "0",
            NodeId::Named(name) => name,
        }
    }

    /// Creates a new node with a prefixed name. Ground is never prefixed.
    pub(crate) fn with_prefix(&self, prefix: &str) -> Node {
        match &self.0 {
            NodeId::Ground => Node::GROUND,
            NodeId::Named(name) => Node::named(format!("{}_{}", prefix, name)),
        }
    }
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.spice_name())
    }
}

// --- Ergonomic conversions (for tests, external lib interop) ---

impl From<&str> for Node {
    fn from(s: &str) -> Self {
        match s {
            "0" | "gnd" | "GND" | "Gnd" => Node::GROUND,
            _ => Node::named(s),
        }
    }
}

impl From<String> for Node {
    fn from(s: String) -> Self {
        match s.as_str() {
            "0" | "gnd" | "GND" | "Gnd" => Node::GROUND,
            _ => Node::named(s),
        }
    }
}

impl From<usize> for Node {
    fn from(n: usize) -> Self {
        if n == 0 { Node::GROUND } else { Node::named(n.to_string()) }
    }
}

impl From<&Node> for Node {
    fn from(n: &Node) -> Self {
        n.clone()
    }
}

/// Convenience alias — deprecated, prefer `Node::GROUND`.
pub const GND: Node = Node::GROUND;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ground_variants() {
        assert_eq!(Node::from("0"), Node::GROUND);
        assert_eq!(Node::from("GND"), Node::GROUND);
        assert_eq!(Node::from("gnd"), Node::GROUND);
        assert_eq!(Node::from(0usize), Node::GROUND);
    }

    #[test]
    fn display() {
        assert_eq!(Node::GROUND.to_string(), "0");
        assert_eq!(Node::named("out").to_string(), "out");
    }

    #[test]
    fn auto_nodes_unique() {
        let a = Node::auto();
        let b = Node::auto();
        assert_ne!(a, b);
    }

    #[test]
    fn prefix() {
        let n = Node::named("net_1");
        assert_eq!(n.with_prefix("inv1").to_string(), "inv1_net_1");
        assert_eq!(Node::GROUND.with_prefix("inv1"), Node::GROUND);
    }
}
