use crate::component::Components;
use crate::measure::Measure;
use std::collections::HashMap;
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
            NodeIdentifier::Named(name) => name.to_uppercase() == "GND",
            NodeIdentifier::Gnd => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct BranchIdentifier {
    pub(crate) component: String,
    pub(crate) name: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum CircuitReference {
    Node(NodeIdentifier, usize),
    Branch(BranchIdentifier, usize),
}

impl CircuitReference {
    pub fn get_id(&self) -> usize {
        match self {
            CircuitReference::Node(_, id) => *id,
            CircuitReference::Branch(_, id) => *id,
        }
    }

    pub fn is_ground(&self) -> bool {
        self.get_id() == 0
    }
}

pub struct Netlist {
    nodes: HashMap<NodeIdentifier, usize>,
    branches: HashMap<BranchIdentifier, usize>,
    id_seq: AtomicUsize,
}

impl Netlist {
    pub(crate) fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            branches: HashMap::new(),
            id_seq: AtomicUsize::new(1),
        }
    }

    pub fn connect_node(&mut self, node: NodeIdentifier) -> CircuitReference {
        if node.is_ground() {
            return CircuitReference::Node(NodeIdentifier::Gnd, 0);
        }

        let id = self
            .nodes
            .entry(node.clone())
            .or_insert_with(|| self.id_seq.fetch_add(1, Ordering::SeqCst));

        CircuitReference::Node(node, *id)
    }

    pub fn connect_branch(&mut self, branch: BranchIdentifier) -> CircuitReference {
        let id = self
            .branches
            .entry(branch.clone())
            .or_insert_with(|| self.id_seq.fetch_add(1, Ordering::SeqCst));

        CircuitReference::Branch(branch, *id)
    }

    pub fn all_nodes(&self) -> Vec<(NodeIdentifier, CircuitReference)> {
        self.nodes
            .iter()
            .map(|(identifier, id)| {
                (
                    identifier.clone(),
                    CircuitReference::Node(identifier.clone(), *id),
                )
            })
            .collect()
    }

    pub fn all_branches(&self) -> Vec<(BranchIdentifier, CircuitReference)> {
        self.branches
            .iter()
            .map(|(identifier, id)| {
                (
                    identifier.clone(),
                    CircuitReference::Branch(identifier.clone(), *id),
                )
            })
            .collect()
    }
}

pub struct Circuit {
    pub components: Components,
    pub netlist: Netlist,
    pub measures: Vec<Measure>,
}

impl Circuit {
    pub fn new(components: Components, netlist: Netlist, measures: Vec<Measure>) -> Self {
        Self {
            components,
            netlist,
            measures,
        }
    }

    pub fn components(&self) -> &Components {
        &self.components
    }

    pub fn components_mut(&mut self) -> &mut Components {
        &mut self.components
    }

    pub fn netlist(&self) -> &Netlist {
        &self.netlist
    }

    pub fn netlist_mut(&mut self) -> &mut Netlist {
        &mut self.netlist
    }

    pub fn measures(&self) -> &Vec<Measure> {
        &self.measures
    }

    pub fn measures_mut(&mut self) -> &mut Vec<Measure> {
        &mut self.measures
    }
}
