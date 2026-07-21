//! Forces capability: `V`/`I`-source branch forces (`E_i(V)`) plus their
//! inductor-flux and series-impedance companions and AC stimulus rows.

use crate::resolve::NodeId;

use super::{AnalogCapability, AnalogFn};

/// Compiled force-source rows. Present (`Some`) exactly when the analog body
/// declares at least one `V`/`I` branch force.
pub(super) struct Forces {
    /// Branch terminals `(plus, minus)` per force row.
    pub(super) terminals: Vec<(NodeId, NodeId)>,
    /// Force source values `E_i(V)` (`num_forces × n` row-major Jacobian).
    pub(super) value: AnalogFn,
    pub(super) jacobian: AnalogFn,
    /// Per-force AC stimulus magnitude/phase rows; `None` when no force
    /// carries a stimulus.
    pub(super) ac_mag: Option<AnalogFn>,
    pub(super) ac_phase: Option<AnalogFn>,
    /// Inductor flux coefficient rows; `None` when no force is reactive.
    pub(super) flux: Option<AnalogFn>,
    /// Per flux term: `(force_idx, target_plus, target_minus)`.
    pub(super) flux_meta: Vec<(usize, NodeId, NodeId)>,
    /// Series-impedance coefficient rows; `None` when no force value reads a
    /// branch current.
    pub(super) current: Option<AnalogFn>,
    /// Per current term: `(force_idx, target_plus, target_minus)`.
    pub(super) current_meta: Vec<(usize, NodeId, NodeId)>,
}

impl AnalogCapability for Forces {
    fn count(&self) -> usize {
        self.terminals.len()
    }
}
