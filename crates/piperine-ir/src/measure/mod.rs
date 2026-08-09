//! `Measure` — streaming reductions at the solver's rate (`ideal.md` §3.5,
//! DVT-13/15/16). A `Measure` is fed one `(t, v)` per **accepted** point,
//! holds O(1) state, and never stores a waveform — the same accumulator
//! reduces a stored `Waveform` post-run and a live transient online, and
//! both must agree (T13's own regression test).
//!
//! **`MeasureKind` is the extension point** (§3.7, §12.2), symmetric with
//! `Behavior`'s `BehaviorKind`: closed in Python, open in Rust. The shape
//! is `ideal.md`'s verbatim.
//!
//! Gating (`gate.rs`, T12) and the arm/read/disarm lifecycle (`handle.rs`,
//! T13) build on top of the catalog here (`kinds.rs`, T11) without
//! changing this trait.

pub mod gate;
pub mod handle;
pub mod kinds;

/// A streaming reduction: fed one accepted `(t, v)` per call, with the
/// local LTE at that point so the reduction can carry an honest error
/// bound (`ideal.md` §3.6) instead of a bare number.
pub trait MeasureKind {
    /// Fold in one accepted sample.
    fn update(&mut self, t: f64, v: f64, lte: f64);
    /// The reduction's current value.
    fn value(&self) -> f64;
    /// How far `value()` could still be off, derived from the local LTE
    /// this kind has seen.
    fn error_bound(&self) -> f64;
}
