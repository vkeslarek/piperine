use crate::component::ComponentBlueprint;
use crate::model::{AnyModel, ModelResolver};
use std::collections::HashMap;
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
            NodeIdentifier::Named(name) => name.to_uppercase() == "GND",
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

pub struct CircuitBuilder {
    title: String,
    model_resolver: ModelResolver,
    components: HashMap<String, Box<dyn ComponentBlueprint>>,
}

impl CircuitBuilder {
    pub(crate) fn insert_get<B: ComponentBlueprint>(
        &mut self,
        name: &str,
        component: impl ComponentBlueprint,
    ) -> Option<&mut B> {
        let name_str = name.to_string();

        self.components
            .insert(name_str.clone(), Box::new(component));

        self.components
            .get_mut(&name_str)
            .and_then(|b| b.as_any_mut().downcast_mut::<B>())
    }

    pub fn instantiate(&self) -> Circuit {
        Circuit::new(self.title.clone())
    }

    pub fn model(&mut self, name: &str, model: impl AnyModel) {
        self.model_resolver
            .insert(name.to_string(), Arc::new(model));
    }
}

pub struct Circuit {
    title: String,
    model_resolver: ModelResolver,
    components: HashMap<String, Box<dyn ComponentBlueprint>>,
}

impl Circuit {
    fn new(title: String) -> Self {
        Self {
            title: title.to_string(),
            model_resolver: ModelResolver::new(),
            components: Default::default(),
        }
    }
    pub fn build(name: &str, build_fn: fn(&mut CircuitBuilder)) -> Circuit {
        todo!()
    }
}
