use crate::math::linear::Symbol;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum NodeIdentifier {
    Named(String),
    Indexed(usize),
    Gnd,
}

impl NodeIdentifier {
    pub fn is_ground(&self) -> bool {
        match self {
            NodeIdentifier::Gnd => true,
            _ => false,
        }
    }
}

pub trait IntoNodeIdentifier: Into<NodeIdentifier> {}
impl<T> IntoNodeIdentifier for T where T: Into<NodeIdentifier> {}

impl Into<NodeIdentifier> for usize {
    fn into(self) -> NodeIdentifier {
        NodeIdentifier::Indexed(self)
    }
}

impl From<&str> for NodeIdentifier {
    fn from(name: &str) -> Self {
        if name.to_uppercase() == "GND" {
            NodeIdentifier::Gnd
        } else {
            NodeIdentifier::Named(name.to_string())
        }
    }
}

impl Into<NodeIdentifier> for String {
    fn into(self) -> NodeIdentifier {
        if self == "GND" {
            NodeIdentifier::Gnd
        } else {
            NodeIdentifier::Named(self)
        }
    }
}

pub const GND: NodeIdentifier = NodeIdentifier::Gnd;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct BranchIdentifier {
    pub component: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum CircuitReference {
    Node(NodeIdentifier),
    Branch(BranchIdentifier),
}

impl CircuitReference {
    pub fn is_ground(&self) -> bool {
        match self {
            CircuitReference::Node(identifier) => identifier.is_ground(),
            _ => false,
        }
    }
}

impl Symbol for CircuitReference {}

pub struct Netlist {
    circuit_references: HashSet<CircuitReference>,
}

impl Netlist {
    pub fn new() -> Self {
        Self {
            circuit_references: HashSet::new(),
        }
    }

    pub fn connect_node(&mut self, node: NodeIdentifier) -> CircuitReference {
        if node.is_ground() {
            return CircuitReference::Node(NodeIdentifier::Gnd);
        }

        let circuit_reference = CircuitReference::Node(node);
        self.circuit_references.insert(circuit_reference.clone());

        circuit_reference
    }

    pub fn connect_branch(&mut self, branch: BranchIdentifier) -> CircuitReference {
        let circuit_reference = CircuitReference::Branch(branch);
        self.circuit_references.insert(circuit_reference.clone());
        circuit_reference
    }

    pub fn all_references(&self) -> Vec<CircuitReference> {
        self.circuit_references.iter().cloned().collect()
    }
}
