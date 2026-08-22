//! `limexp()` capability (LRM §4.5.13): per-call-site output-history state.
//!
//! Reuses the `$limit` state-slot shape (`limits.rs`): each unique
//! `limexp(x)` call site gets one state-bank slot holding its previous
//! *limited* output `yold`. Unlike `$limit` there is no vcrit seed value —
//! `AnalogInstance::new` seeds every slot to `-1.0` (see
//! `piperine_ir::limit::limexp_limit`'s doc comment for why not `0.0`) — no
//! branch/catalog bookkeeping (a `limexp` call's argument is any
//! expression, not a terminal-pair voltage), and no convergence veto (it
//! only needs to break the runaway feedback loop that makes a stiff
//! exponential diverge, not to name a clamped node in `limiting_report`).
//!
//! The `update` row is compiled from the *original* `limexp(x)` call
//! expression, the same trick `$limit`'s `update` uses: evaluating the call
//! with the current `yold` already in state produces exactly the new
//! limited value to write back.
//!
//! **Jacobian note:** `limexp`'s derivative is hard-coded transparent to
//! this state in `resolve/diff.rs` (mirroring `$limit`'s own rule) — the
//! Jacobian coefficient is always the raw, state-independent `exp(x)`,
//! never routed back through this capability. Composing the Jacobian
//! coefficient from a state-reading `limexp` recursion was a real,
//! reproducible non-convergence bug found during development (a corrupted
//! Jacobian row exactly when limiting was active); see `limexp_full`'s doc
//! comment in `piperine-ir`.

use super::{AnalogCapability, AnalogFn};

/// Compiled `limexp()` update rows.
pub(super) struct Limexp {
    /// Per-slot updated (limited) output `ylim`, written back into the
    /// state bank each Newton iteration.
    pub(super) update: AnalogFn,
    pub(super) count: usize,
}

impl AnalogCapability for Limexp {
    fn count(&self) -> usize {
        self.count
    }
}

mod wrappers {
    /// `limexp(x)`, whole (argument clamp + `exp` + iteration-history
    /// limit) in one native call.
    pub extern "C" fn limexp_full(x: f64, yold: f64) -> f64 {
        piperine_ir::limit::limexp_full(x, yold)
    }
}

/// `limexp`'s combined JIT symbol — the `LimitFn` shape (`limits.rs`),
/// applied to a one-entry table.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LimexpFn {
    pub(crate) name: &'static str,
    pub(crate) arity: usize,
    pub(crate) symbol: *const u8,
}

// The table is a read-only function pointer; sharing across threads is safe.
unsafe impl Sync for LimexpFn {}

pub(crate) const LIMEXP_FNS: &[LimexpFn] = &[
    LimexpFn { name: "__limexp_full", arity: 2, symbol: wrappers::limexp_full as *const u8 },
];
