//! `ac_stim` capability: per-source AC stimulus magnitude/phase rows.

use crate::resolve::NodeId;

use super::{AnalogCapability, AnalogFn};

/// Compiled `ac_stim` sources. Present (`Some`) exactly when the analog body
/// declares at least one `ac_stim` contribution.
pub(super) struct AcStim {
    /// Terminals `(plus, minus)` per `ac_stim` source.
    pub(super) terminals: Vec<(NodeId, NodeId)>,
    /// Magnitude and phase (radians) per source.
    pub(super) mag: AnalogFn,
    pub(super) phase: AnalogFn,
}

impl AnalogCapability for AcStim {
    fn count(&self) -> usize {
        self.terminals.len()
    }
}
