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
//! Python (`pip.Behavior.*` is what ships), open in Rust — implementing
//! this trait outside this module composes exactly like a shipped kind, no
//! plugin and no build ceremony. The shape is `ideal.md`'s verbatim.

pub mod kinds;
pub mod table;

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
pub trait BehaviorKind {
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
