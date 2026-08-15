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
    /// `"vdslim"` (LRM Table E.2's own preferred spelling, T58) reports
    /// with its own name in the catalog, not silently renamed to
    /// `"limvds"`, while resolving to the same `VdsStep` reason.
    pub(super) fn catalog_entry_for_kind(kind: &str) -> (&'static str, LimitReason) {
        let name: &'static str = match kind {
            "pnjlim" => "pnjlim",
            "fetlim" => "fetlim",
            "limvds" => "limvds",
            "vdslim" => "vdslim",
            _ => "limit",
        };
        let reason = match kind {
            "limvds" | "vdslim" => LimitReason::VdsStep,
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

/// One `$limit` kind's JIT-linkable symbol: IR name, arity, and the native
/// symbol the JIT links against — the `MathFn` shape
/// (`piperine-lang/src/math.rs`), applied to a second table.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LimitFn {
    pub(crate) name: &'static str,
    pub(crate) arity: usize,
    pub(crate) symbol: *const u8,
}

// The table is read-only function pointers; sharing across threads is safe.
unsafe impl Sync for LimitFn {}

mod wrappers {
    pub extern "C" fn pnjlim(vnew: f64, vold: f64, vte: f64, vcrit: f64) -> f64 {
        piperine_ir::limit::pnjlim(vnew, vold, vte, vcrit)
    }
    pub extern "C" fn fetlim(vnew: f64, vold: f64, vto: f64) -> f64 {
        piperine_ir::limit::fetlim(vnew, vold, vto)
    }
    pub extern "C" fn limvds(vnew: f64, vold: f64) -> f64 {
        piperine_ir::limit::limvds(vnew, vold)
    }
}

/// The three `$limit` kinds' JIT-linkable symbols, registered alongside
/// `MATH_FNS` in the analog JIT only — `$limit` is an analog-only construct.
pub(crate) const LIMIT_FNS: &[LimitFn] = &[
    LimitFn { name: "pnjlim", arity: 4, symbol: wrappers::pnjlim as *const u8 },
    LimitFn { name: "fetlim", arity: 3, symbol: wrappers::fetlim as *const u8 },
    LimitFn { name: "limvds", arity: 2, symbol: wrappers::limvds as *const u8 },
];
