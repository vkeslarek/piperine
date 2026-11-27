//! The sites of the analog block: the places a body's code asks the engine for a limit, a stimulus, a derivative, a delay, a noise source or a trace.

use serde::{Deserialize, Serialize};

use crate::ids::ExprId;
use crate::model::{Origin};

/// A `limit` or `ac_stim` site; its index in the body's list is its identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Site {
    pub name: String,
    pub origin: Origin,
    /// A `limit` read under a condition: the conjunction of the enclosing conditions, a `bool`
    /// lowered standalone (outside every condition). When it is false the limiter does not run
    /// and the held value stays (abi.md A3, §14.7.4). `None` for a `limit` at the top of the
    /// `analog` block and for every `ac_stim` site, which needs none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<ExprId>,
}

/// A derivative site: a `ddt` whose result is read anywhere but as a whole addend of an
/// unconditional contribution (abi.md A1). Its rate is the private continuous pin `d<k>.rate`,
/// an unknown that is 0 in an equilibrium solve, the integrator's derivative of `state` in a
/// transient step and `jω·state` in a sweep; `Expr::Derivative(k)` reads it. Its index in the
/// body's list is its identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Derivative {
    pub name: String,
    /// The differentiated quantity, an `f64`. It is evaluated at every accepted solution whatever
    /// control flow surrounds the `ddt` (§14 rule 9), so it reads nothing a condition set.
    pub state: ExprId,
    /// `ddt(x, abstol)`: the absolute tolerance of the state, an `f64` that reads no probe,
    /// carrier or clock (a constant or a param expression).
    pub abstol: Option<ExprId>,
    pub origin: Origin,
}

/// A `delay(x, td, max)` (§14.6): its output is the private continuous pin `delay<k>.value`, an
/// unknown that is `x` in an equilibrium solve, `x(t - td)` read from history in a transient step
/// and `exp(-jω·td)·x` in a sweep; `Expr::Delay(k)` reads it. Its index in the body's list is its
/// identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DelaySite {
    pub name: String,
    /// The delayed quantity, an `f64`, evaluated at every accepted solution (§14 rule 9).
    pub input: ExprId,
    /// The delay time, an `f64` in seconds.
    pub time: ExprId,
    /// `delay(x, td, max)`: the longest delay the history keeps, an `f64` on the parameter cone.
    /// `None` bounds the history by `time` itself.
    pub maximum: Option<ExprId>,
    pub origin: Origin,
}

/// A `noise(psd, name)` (§14.8): the `NOISE` input `noise:<k>`. Its value is 0 in every solve; a
/// noise sweep injects the partials against it with the power density `density`. Its index in
/// the body's list is its identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoiseSource {
    pub name: String,
    /// The group the library reports the source's density under: the literal `name` argument
    /// found after inlining, `None` for an unnamed source (reported by instance path).
    pub group: Option<String>,
    /// The power spectral density, an `f64` evaluated only by a noise sweep (at its frequency).
    pub density: ExprId,
    pub origin: Origin,
}

/// A session parameter a body reads (`simparam`, §14.4): the `GLOBAL NAMED` pin `name`, one per
/// name in a body. Its index in the body's list is its identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionParameter {
    pub name: String,
    pub origin: Origin,
}

/// An `emit!` in the `analog` block (§19 rule 4): one `TRACE` watch, written at every Newton
/// evaluation that executes the statement. Its index in the body's list is its identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceSite {
    /// The record stream the site writes (its index in `Body::streams`).
    pub stream: u32,
    pub origin: Origin,
}

/// A `ddx` the elaborator differentiated (§14.2): the expression it stands for is already the
/// derivative, built by `Differentiation`; this records where it was written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartialDerivative {
    /// The probe or flow `ddx` differentiated against, as the source spells it.
    pub name: String,
    pub origin: Origin,
}

/// A module placed in an expression whose node or `analog` statements joined this body (Appendix
/// C.1.1): `name` is the placement's name in the body (`lag`, `Lag$1`), `module` the placed
/// module. The IR keeps the walkable record so a reader sees what was placed where.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hoisted {
    pub name: String,
    pub module: String,
    pub origin: Origin,
}
