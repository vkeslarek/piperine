//! Static dependencies and the affine test (IR-09): what a body's addends, `limit` sites,
//! `ac_stim` sites and `where` checks read, and whether a branch's direct part is affine in the
//! node potentials. Every backend declares its reads and its partial pattern from these sets.
//!
//! One abstract walk per body, memoized on first use. Every value carries the set of sources it
//! reads and a class on the lattice `Constant < Affine < Nonlinear` (in the node potentials):
//!
//! - constants, params and environment reads are `Constant`; a probe is `Affine`;
//! - a `limit` read is the site alone, `Affine` (IR-10): the backend holds the limited iterate as an
//!   input that stands in for its input `x` (abi.md A2), so `x`, the function and the seed are
//!   the site's own dependencies (`Target::Limit`), not its readers';
//! - a `limit` under a condition also reads what its `enabled` reads, and a `bound_step` what its
//!   `when` reads (`Target::Limit`, `Target::BoundStep`);
//! - an `ac_stim` read is the site plus its magnitude and phase, which are evaluated where the
//!   stimulus appears; it is `Constant` when they are;
//! - `+`, `-` and unary `-` join; `*` by a `Constant` operand and `/` by a `Constant` divisor
//!   keep the other operand's class; any other operator, cast or method touching a
//!   non-`Constant` operand gives `Nonlinear`;
//! - a call reads its arguments and what the callee reads itself (walked with its parameters
//!   bound to the arguments, once per distinct function and argument values); it is `Constant`
//!   when every argument is, `Nonlinear` otherwise;
//! - an `if` reads its condition and both arms; it joins the arms when the condition is
//!   `Constant`, and is `Nonlinear` otherwise (IR-10);
//! - a contribution under an `if` also reads what the conditions of its enclosing arms read, and
//!   a condition that is not `Constant` makes its branch non-affine (a switch).
//!
//! - a carrier read is `Constant` in the node potentials and reads the carrier: a discrete value
//!   holds between events (`Body::carrier_reads` lists them; `Deps` keeps its M1 fields).
//!
//! A reaction reads every source of every expression its statements evaluate, conditions and
//! the values and delays of its commits included.
//!
//! Locals are bound by scope, as `Ir::check` reads them: a `let` until its block ends, a
//! closure's parameters in its body. The walk assumes an IR that passes `Ir::check`, which names every form the walk cannot give
//! a meaning to (a local read before its `let`, a cycle); on an IR that fails
//! it the walk panics with the origin, never answers a default.
//!
//! Mutation, loops and recursion (`walk/control.rs`) stay inside the finite lattice: an assignment
//! joins the assigned value into the local's, a loop's body is walked again until no local
//! changes, and a recursive call reads the function's result so far, which the function's own
//! walk grows until it holds still. A loop's trip count and the conditions its exits sit under
//! reach every local the loop assigns, and a function value is walked as a call whose
//! arguments are unknown.

mod dependency_set;
mod queries;
mod results;
mod sources;
mod target;
mod walk;

pub(crate) use results::{BodyDeps, DepsMemo};
pub use dependency_set::Deps;
use sources::{Class, Sources, Value};
pub use target::{BranchIndex, ParamIndex, SiteIndex, Target};
