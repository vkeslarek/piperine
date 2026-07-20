//! Limits capability: `$limit` (pnjlim/fetlim) vold-slot bookkeeping.

use super::{AnalogCapability, AnalogFn};

/// Compiled `$limit` rows. Present (`Some`) exactly when the analog body
/// contains at least one `$limit` call; one vold slot per row, appended to
/// the state bank after the module's runtime-state slots.
pub(super) struct Limits {
    /// Per-slot updated value `vlim`; the device writes these back into the
    /// state bank to seed the next Newton iteration.
    pub(super) update: AnalogFn,
    /// Per-slot seed value `vcrit`, for initializing the vold slots at
    /// device creation (ngspice MODEINITJCT).
    pub(super) seed: AnalogFn,
    /// Per-slot raw (unlimited) `vnew`, used with `branches` to detect the
    /// branch polarity when building the limited Norton linearization point.
    pub(super) vnew: AnalogFn,
    /// Per-slot junction branch as terminal slot indices `(plus, minus)`
    /// (`None` slot = ground); the outer `None` means the branch was not
    /// uniquely identifiable and the raw voltage is used.
    pub(super) branches: Vec<Option<(Option<usize>, Option<usize>)>>,
}

impl AnalogCapability for Limits {
    fn count(&self) -> usize {
        self.branches.len()
    }
}
