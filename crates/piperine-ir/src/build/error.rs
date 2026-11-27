//! What an [`IrBuilder`](crate::IrBuilder) refuses: an id or a slot the builder never handed out.

use std::fmt;

use crate::ids::{InstanceId, NetId};

/// A builder call that named an id or a slot outside what the builder holds. The ids come from the
/// builder itself, so this is a caller that mixed builders or kept a stale id, never a user's input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildError {
    /// A net id past the end of the net arena.
    NetOutOfRange { net: NetId, count: usize },
    /// An instance id past the end of the instance arena.
    InstanceOutOfRange { instance: InstanceId, count: usize },
    /// A node slot past the nodes of the instance at `instance` (its path).
    NodeSlotOutOfRange { instance: String, slot: u32, count: usize },
    /// A carrier slot past the nets bound to the instance at `instance` (its path).
    CarrierSlotOutOfRange { instance: String, carrier: u32, count: usize },
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::NetOutOfRange { net, count } => write!(f, "net id {} out of range ({count} nets)", net.0),
            BuildError::InstanceOutOfRange { instance, count } => write!(f, "instance id {} out of range ({count} instances)", instance.0),
            BuildError::NodeSlotOutOfRange { instance, slot, count } => write!(f, "instance {instance}: node slot {slot} out of range ({count} nodes)"),
            BuildError::CarrierSlotOutOfRange { instance, carrier, count } => write!(f, "instance {instance}: carrier {carrier} out of range ({count} nets bound)"),
        }
    }
}

impl std::error::Error for BuildError {}
