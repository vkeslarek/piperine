//! The `Behavior` family: value from circuit state, with a closed-form
//! Jacobian. `table` (`table.rs`) is extracted first, as the design
//! precedent — a pure piecewise function whose derivative is free.
//!
//! `ideal.md` §3.3/§3.7: a `Behavior` is `f(v₁ … vₙ)` — it depends on the
//! solution, so it contributes **cross Jacobian entries** (`∂f/∂vᵢ`), not
//! just an RHS. Each kind ships as native Rust with a closed-form value and
//! a closed-form derivative, which is what buys zero host calls inside
//! Newton while still reacting to the circuit at every iteration.
//!
//! **`BehaviorKind` is the extension point** (§3.7, §12.2): closed in
//! Python (`pip.Behavior.*` ships), open in Rust — implementing this
//! trait elsewhere composes like a shipped kind, no plugin, no build
//! ceremony; the shape is `ideal.md`'s verbatim.
pub mod fd_oracle;
pub mod kinds;
pub mod table;
pub mod table_model;

use crate::events::CrossDir;

/// A discontinuity a `Behavior` kind reports so the integrator can land on
/// the edge instead of interpolating across it, rather than smoothing over
/// it (`ideal.md` §3.3: "a genuinely discontinuous kind ... registers a
/// crossing"). Reuses [`CrossDir`] rather than inventing a parallel
/// vocabulary — the crate's one interface for "a predicate flips", shared
/// with [`crate::events`] (tier 1) and its tier-2 extension.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Crossing {
    /// The controlling quantity's current value (e.g. `a - b` for a
    /// two-terminal kind).
    pub value: f64,
    /// The threshold `value` is compared against.
    pub threshold: f64,
    /// Which direction of sign change (of `value - threshold`) matters.
    pub dir: CrossDir,
}

/// A `Behavior` kind: closed-form value and closed-form Jacobian over an
/// ordered slice of controlling inputs. Smooth is the default (a
/// `tanh`-shaped `comparator` is differentiable and convergence-friendly);
/// a genuinely discontinuous kind overrides [`crossings`](Self::crossings)
/// instead of faking a derivative across its own jump.
// SPEC_DEVIATION (dv-tester T16/T18): `Send + Sync` supertrait — a
// `Behavior` in a `HostElement` reaches the solver as `Box<dyn Element>`,
// which requires it unconditionally; declaring it here (the `Box<dyn Error
// + Send + Sync>` idiom) makes `Box<dyn BehaviorKind>` satisfy it too.
// `Hysteresis`'s memory cell (`kinds.rs`) was the one non-conforming kind.
pub trait BehaviorKind: Send + Sync {
    /// `f(inputs)`.
    fn value(&self, inputs: &[f64]) -> f64;
    /// `∂f/∂inputs[i]`, written into `out` (same length as `inputs`).
    fn jacobian(&self, inputs: &[f64], out: &mut [f64]);
    /// A discontinuity this kind wants the integrator to land on, if one is
    /// active at `inputs`. `None` for every smooth kind (the default).
    fn crossings(&self, inputs: &[f64]) -> Option<Crossing> {
        let _ = inputs;
        None
    }
}
