use crate::math::linear::AsIndex;
use bimap::BiMap;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum NodeIdentifier {
    Anonymous(usize),
    Gnd,
}

impl NodeIdentifier {
    pub fn is_ground(&self) -> bool {
        matches!(self, NodeIdentifier::Gnd)
    }
}

impl fmt::Display for NodeIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeIdentifier::Anonymous(n) => write!(f, "n{}", n),
            NodeIdentifier::Gnd => write!(f, "GND"),
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

impl From<String> for BranchIdentifier {
    fn from(val: String) -> Self {
        BranchIdentifier {
            component: val,
            name: None,
        }
    }
}

impl From<&str> for BranchIdentifier {
    fn from(val: &str) -> Self {
        BranchIdentifier {
            component: val.to_string(),
            name: None,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum AnalogVariable {
    Node(NodeIdentifier),
    Branch(BranchIdentifier),
    Time,
    Frequency,
    Iteration,
}

impl AnalogVariable {
    pub fn is_ground(&self) -> bool {
        match self {
            AnalogVariable::Node(identifier) => identifier.is_ground(),
            _ => false,
        }
    }

    pub fn is_branch(&self) -> bool {
        matches!(self, AnalogVariable::Branch(_))
    }

    pub fn is_node(&self) -> bool {
        matches!(self, AnalogVariable::Node(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AnalogReference {
    variable: Arc<AnalogVariable>,
    idx: Option<usize>,
}

impl AnalogReference {
    pub fn new(variable: Arc<AnalogVariable>, idx: usize) -> Self {
        Self {
            variable,
            idx: Some(idx),
        }
    }

    fn new_unmapped(variable: Arc<AnalogVariable>) -> Self {
        Self {
            variable,
            idx: None,
        }
    }

    /// Create a ground reference (idx = None, variable = GND).
    pub fn ground() -> Self {
        Self::new_unmapped(Arc::new(AnalogVariable::Node(NodeIdentifier::Gnd)))
    }

    pub fn variable(&self) -> &Arc<AnalogVariable> {
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

impl From<AnalogReference> for Arc<AnalogVariable> {
    fn from(val: AnalogReference) -> Self {
        val.variable
    }
}

impl AsIndex for AnalogReference {
    fn as_index(&self) -> Option<usize> {
        self.idx
    }
}

pub struct Netlist {
    circuit_map: BiMap<AnalogReference, Arc<AnalogVariable>>,
    last_seen_idx: AtomicUsize,
}

impl Default for Netlist {
    fn default() -> Self {
        Self::new()
    }
}

impl Netlist {
    pub fn new() -> Self {
        Self {
            circuit_map: BiMap::new(),
            last_seen_idx: AtomicUsize::new(0),
        }
    }

    pub fn connect_node(&mut self, node: NodeIdentifier) -> AnalogReference {
        let circuit_reference = AnalogVariable::Node(node);
        if let Some(existing_ref) = self.circuit_map.get_by_right(&circuit_reference) {
            return existing_ref.clone();
        }

        if circuit_reference.is_ground() {
            let reference = AnalogReference::new_unmapped(Arc::new(circuit_reference));
            self.circuit_map.insert(
                reference.clone(),
                Arc::new(AnalogVariable::Node(NodeIdentifier::Gnd)),
            );
            return reference;
        }

        let ref_arc = Arc::new(circuit_reference.clone());
        let idx = self.last_seen_idx.fetch_add(1, Ordering::SeqCst);
        let identifier = AnalogReference::new(ref_arc.clone(), idx);

        self.circuit_map.insert(identifier.clone(), ref_arc);

        identifier
    }

    pub fn connect_branch(&mut self, branch: BranchIdentifier) -> AnalogReference {
        let circuit_reference = AnalogVariable::Branch(branch);
        if let Some(existing_ref) = self.circuit_map.get_by_right(&circuit_reference) {
            return existing_ref.clone();
        }

        let ref_arc = Arc::new(circuit_reference.clone());
        let idx = self.last_seen_idx.fetch_add(1, Ordering::SeqCst);
        let identifier = AnalogReference::new(ref_arc.clone(), idx);

        self.circuit_map.insert(identifier.clone(), ref_arc);

        identifier
    }

    pub fn all_references(&self) -> Vec<&AnalogReference> {
        self.circuit_map.left_values().collect()
    }

    pub fn reference_for(&self, variable: &AnalogVariable) -> Option<&AnalogReference> {
        self.circuit_map.get_by_right(variable)
    }

    pub fn variable_for(&self, identifier: &AnalogReference) -> Option<&Arc<AnalogVariable>> {
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
