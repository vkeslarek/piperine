use std::collections::HashMap;

/// A globally unique identifier for an expanded net (flattened node).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

/// Identifies the domain of a net.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Domain {
    Analog,
    Digital,
}

/// Represents a fully expanded, flattened net in the simulation.
#[derive(Debug, Clone)]
pub struct IrNet {
    pub id: NodeId,
    /// The hierarchical path to this net in the original AST (for debugging)
    pub original_path: String,
    pub domain: Domain,
}

/// Represents an instantiated analog block, ready to be handed to the solver as an `AnalogRuntime`.
#[derive(Debug, Clone)]
pub struct AnalogIrInstance {
    pub instance_name: String,
    pub model_name: String,
    pub terminals: Vec<NodeId>,
    pub parameters: HashMap<String, f64>,
    pub str_parameters: HashMap<String, String>,
}

/// Represents an instantiated digital block, ready to be handed to the solver as a `DigitalRuntime`.
#[derive(Debug, Clone)]
pub struct DigitalIrInstance {
    pub instance_name: String,
    pub model_name: String,
    pub terminals: Vec<NodeId>,
    pub parameters: HashMap<String, f64>,
    pub str_parameters: HashMap<String, String>,
}

/// A cross-domain connect module instance automatically inserted by the elaborator.
#[derive(Debug, Clone)]
pub struct ConnectIrInstance {
    pub instance_name: String,
    pub connect_type: String, // e.g., "A2D" or "D2A"
    pub analog_port: NodeId,
    pub digital_port: NodeId,
}

/// The fully flattened intermediate representation of the design.
/// This structure acts as the bridge connecting the Parser/Elaborator phase 
/// directly to the `piperine-solver::Circuit`.
#[derive(Debug, Default, Clone)]
pub struct IrDesign {
    pub nets: HashMap<NodeId, IrNet>,
    pub analog_instances: Vec<AnalogIrInstance>,
    pub digital_instances: Vec<DigitalIrInstance>,
    pub connect_instances: Vec<ConnectIrInstance>,
}

impl IrDesign {
    pub fn new() -> Self {
        Self::default()
    }
}
