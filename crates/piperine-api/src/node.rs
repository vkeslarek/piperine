use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

/// Global counter for auto-generated node IDs.
static AUTO_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Global interner for named nodes. Index = Named(idx).
static NAMED_NODES: RwLock<Vec<String>> = RwLock::new(Vec::new());

/// Internal node identifier. All variants are Copy.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum NodeId {
    /// Ground reference (SPICE "0").
    Ground,
    /// Auto-generated node. SPICE name = "net_{id}". No storage needed.
    Auto(u64),
    /// User-named node. SPICE name looked up in NAMED_NODES[idx].
    Named(u64),
}

/// An opaque, zero-cost circuit node handle.
///
/// `Node` is `Copy` — pass it around freely without `.clone()`.
/// The SPICE name is resolved lazily via `spice_name()`.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Node(NodeId);

impl Node {
    /// The ground reference node (SPICE "0").
    pub const GROUND: Node = Node(NodeId::Ground);

    /// Creates a node with an auto-generated unique name (`net_1`, `net_2`, …).
    pub(crate) fn auto() -> Self {
        let id = AUTO_COUNTER.fetch_add(1, Ordering::Relaxed);
        Node(NodeId::Auto(id))
    }

    /// Creates a node with a user-provided label (interned globally).
    pub(crate) fn named(label: impl Into<String>) -> Self {
        let name = label.into();
        let mut names = NAMED_NODES.write().unwrap();
        let idx = names.len() as u64;
        names.push(name);
        Node(NodeId::Named(idx))
    }

    /// Returns `true` if this is the ground node.
    pub fn is_ground(&self) -> bool {
        matches!(self.0, NodeId::Ground)
    }

    /// Returns the SPICE name of this node.
    ///
    /// - Ground → "0"
    /// - Auto(id) → "net_{id}"
    /// - Named(idx) → lookup in global interner
    pub fn spice_name(&self) -> String {
        match self.0 {
            NodeId::Ground => "0".to_string(),
            NodeId::Auto(id) => format!("net_{}", id),
            NodeId::Named(idx) => {
                let names = NAMED_NODES.read().unwrap();
                names[idx as usize].clone()
            }
        }
    }

    /// Creates a new node with a prefixed name. Ground is never prefixed.
    /// Used internally by SubCircuit flatten.
    pub(crate) fn with_prefix(&self, prefix: &str) -> Node {
        match self.0 {
            NodeId::Ground => Node::GROUND,
            NodeId::Auto(id) => Node::named(format!("{}_net_{}", prefix, id)),
            NodeId::Named(idx) => {
                let old_name = {
                    let names = NAMED_NODES.read().unwrap();
                    names[idx as usize].clone()
                };
                Node::named(format!("{}_{}", prefix, old_name))
            }
        }
    }
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.spice_name())
    }
}

// --- Ergonomic conversions ---

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
        if n == 0 {
            Node::GROUND
        } else {
            Node::named(n.to_string())
        }
    }
}

// Node is Copy, so From<&Node> just copies.
impl From<&Node> for Node {
    fn from(n: &Node) -> Self {
        *n
    }
}

/// Convenience alias.
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
        let n = Node::named("out");
        assert_eq!(n.to_string(), "out");
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
        let prefixed = n.with_prefix("inv1");
        assert_eq!(prefixed.to_string(), "inv1_net_1");
        assert_eq!(Node::GROUND.with_prefix("inv1"), Node::GROUND);
    }

    #[test]
    fn node_is_copy() {
        let a = Node::auto();
        let b = a; // Copy, not move
        assert_eq!(a, b); // both still valid
    }
}
