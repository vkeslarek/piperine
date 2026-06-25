use crate::circuit::instance::CircuitInstance;
use crate::circuit::netlist::NodeIdentifier;
use crate::osdi::device::OsdiDevice;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

pub mod instance;
pub mod netlist;

pub struct Circuit {
    pub title: String,
    pub components: HashMap<String, OsdiDevice>,
    pub node_counter: AtomicUsize,
}

impl Circuit {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            components: HashMap::new(),
            node_counter: AtomicUsize::new(0),
        }
    }

    pub fn port(&self) -> NodeIdentifier {
        NodeIdentifier::Anonymous(self.node_counter.fetch_add(1, Ordering::Relaxed))
    }

    pub fn components(&self) -> &HashMap<String, OsdiDevice> {
        &self.components
    }

    pub fn components_mut(&mut self) -> &mut HashMap<String, OsdiDevice> {
        &mut self.components
    }

    pub fn builder<F: FnOnce(&mut Circuit)>(title: impl Into<String>, builder_fn: F) -> Circuit {
        let mut circuit = Circuit::new(title);
        builder_fn(&mut circuit);
        circuit
    }
}

impl Into<CircuitInstance> for Circuit {
    fn into(self) -> CircuitInstance {
        CircuitInstance::instantiate(&self).expect("Failed to instantiate circuit")
    }
}
