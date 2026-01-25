use crate::math::linear::AsIndex;
use bimap::BiMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

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

impl BranchIdentifier {
    pub fn new(component_name: impl Into<String>, branch_name: impl Into<String>) -> Self {
        Self {
            component: component_name.into(),
            name: Some(branch_name.into()),
        }
    }

    pub fn from_component(component_name: impl Into<String>) -> Self {
        Self {
            component: component_name.into(),
            name: None,
        }
    }
}

impl Into<BranchIdentifier> for String {
    fn into(self) -> BranchIdentifier {
        BranchIdentifier {
            component: self,
            name: None,
        }
    }
}

impl Into<BranchIdentifier> for &str {
    fn into(self) -> BranchIdentifier {
        BranchIdentifier {
            component: self.to_string(),
            name: None,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum CircuitVariable {
    Node(NodeIdentifier),
    Branch(BranchIdentifier),
    Time,
    Frequency,
    Iteration,
}

impl CircuitVariable {
    pub fn is_ground(&self) -> bool {
        match self {
            CircuitVariable::Node(identifier) => identifier.is_ground(),
            _ => false,
        }
    }

    pub fn is_branch(&self) -> bool {
        match self {
            CircuitVariable::Branch(_) => true,
            _ => false,
        }
    }

    pub fn is_node(&self) -> bool {
        match self {
            CircuitVariable::Node(_) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CircuitReference {
    variable: Arc<CircuitVariable>,
    idx: Option<usize>,
}

impl CircuitReference {
    fn new(variable: Arc<CircuitVariable>, idx: usize) -> Self {
        Self {
            variable,
            idx: Some(idx),
        }
    }

    fn new_unmapped(variable: Arc<CircuitVariable>) -> Self {
        Self {
            variable,
            idx: None,
        }
    }

    pub fn variable(&self) -> &Arc<CircuitVariable> {
        &self.variable
    }

    pub fn idx(&self) -> Option<usize> {
        self.idx
    }

    pub fn is_branch(&self) -> bool {
        self.variable.is_branch()
    }

    pub fn is_node(&self) -> bool {
        self.variable.is_node()
    }
}

impl Into<Arc<CircuitVariable>> for CircuitReference {
    fn into(self) -> Arc<CircuitVariable> {
        self.variable
    }
}

impl AsIndex for CircuitReference {
    fn as_index(&self) -> Option<usize> {
        self.idx
    }
}

pub struct Netlist {
    circuit_map: BiMap<CircuitReference, Arc<CircuitVariable>>,
    last_seen_idx: AtomicUsize,
}

impl Netlist {
    pub fn new() -> Self {
        Self {
            circuit_map: BiMap::new(),
            last_seen_idx: AtomicUsize::new(0),
        }
    }

    pub fn connect_node(&mut self, node: NodeIdentifier) -> CircuitReference {
        let circuit_reference = CircuitVariable::Node(node);
        if let Some(existing_ref) = self.circuit_map.get_by_right(&circuit_reference) {
            return existing_ref.clone();
        }

        if circuit_reference.is_ground() {
            let reference = CircuitReference::new_unmapped(Arc::new(circuit_reference));
            self.circuit_map.insert(
                reference.clone(),
                Arc::new(CircuitVariable::Node(NodeIdentifier::Gnd)),
            );
            return reference;
        }

        let ref_arc = Arc::new(circuit_reference.clone());
        let idx = self.last_seen_idx.fetch_add(1, Ordering::SeqCst);
        let identifier = CircuitReference::new(ref_arc.clone(), idx);

        self.circuit_map.insert(identifier.clone(), ref_arc);

        identifier
    }

    pub fn connect_branch(&mut self, branch: BranchIdentifier) -> CircuitReference {
        let circuit_reference = CircuitVariable::Branch(branch);
        if let Some(existing_ref) = self.circuit_map.get_by_right(&circuit_reference) {
            return existing_ref.clone();
        }

        let ref_arc = Arc::new(circuit_reference.clone());
        let idx = self.last_seen_idx.fetch_add(1, Ordering::SeqCst);
        let identifier = CircuitReference::new(ref_arc.clone(), idx);

        self.circuit_map.insert(identifier.clone(), ref_arc);

        identifier
    }

    pub fn all_references(&self) -> Vec<&CircuitReference> {
        self.circuit_map.left_values().collect()
    }

    pub fn reference_for(&self, variable: &CircuitVariable) -> Option<&CircuitReference> {
        self.circuit_map.get_by_right(variable)
    }

    pub fn variable_for(&self, identifier: &CircuitReference) -> Option<&Arc<CircuitVariable>> {
        self.circuit_map.get_by_left(identifier)
    }

    pub fn max_index(&self) -> Option<usize> {
        let mut mapped_vars: Vec<_> = self
            .all_references()
            .into_iter()
            .filter(|id| id.idx().is_some())
            .collect();

        mapped_vars.sort_by_key(|id| id.idx().unwrap());

        mapped_vars.last().map(|id| id.idx().unwrap())
    }
}
