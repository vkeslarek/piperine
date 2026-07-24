//! Limits capability: `$limit` (pnjlim/fetlim) vold-slot bookkeeping.

use piperine_solver::abi::LimitReason;

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
    /// Per-slot `(limiter_name, reason)` catalog (phdl-introspection-attributes
    /// PIA-15/16), parallel to `branches`. The name is the `$limit` call-site
    /// `kind` (`"pnjlim"`/`"fetlim"`/`"limvds"`); the reason is inferred from
    /// the kind (`limvds` → `VdsStep`, the junction limiters → `VoltageStep`).
    /// Read by the device's `limiting_report` to name the slot that clamped
    /// instead of the hardcoded `"pnjlim"`.
    pub(super) catalog: Vec<(&'static str, LimitReason)>,
}

impl Limits {
    /// The `(limiter_name, reason)` entry a `$limit` call of `kind` contributes
    /// to the per-slot catalog (PIA-15/16). Reason is inferred from the kind —
    /// zero `$limit` signature change (an omitted reason defaults to the
    /// kind-inferred value, PIA-16/18). The `_` arm is defensive: emit's
    /// `emit_analog_limit` rejects unknown kinds before this is stored.
    pub(super) fn catalog_entry_for_kind(kind: &str) -> (&'static str, LimitReason) {
        let name: &'static str = match kind {
            "pnjlim" => "pnjlim",
            "fetlim" => "fetlim",
            "limvds" => "limvds",
            _ => "limit",
        };
        let reason = match kind {
            "limvds" => LimitReason::VdsStep,
            _ => LimitReason::VoltageStep,
        };
        (name, reason)
    }
}

impl AnalogCapability for Limits {
    fn count(&self) -> usize {
        self.branches.len()
    }
}
