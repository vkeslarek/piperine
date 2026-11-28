//! `ConstEval`: a const expression of the subset evaluated to a [`Constant`] at elaboration.
//!
//! A second small evaluator beside the interpreter's, on purpose: this one reads POM
//! expressions, has no duals and no solver. `f64` is IEEE 754 throughout (`1.0 / 0.0` is `inf`,
//! never an error). Integers follow the rules the compiled code follows (`piperine-transpile`
//! `rt::int`): arithmetic wraps modulo `2^N` (§1 rule 6), `as` from `f64` saturates and maps NaN
//! to 0 (ROADMAP G4), a shift by `N` or more gives 0, or the sign for a signed `>>` (G5). An
//! integer division or remainder by zero, a shift by a negative amount and a `clamp` with its
//! bounds crossed are panics (§1 rule 11): `E0104` at elaboration.
//!
//! A value either has a constant at elaboration or it does not: every value is a value (§3.3.1),
//! so there is no third state. Where a read finds none (a probe, a carrier, an environment
//! function, a function-valued argument read as a number) the fault says so at that read
//! ([`ConstFault::Dynamic`], `E0321` at the position that needed a constant).
//!
//! A `const` item is evaluated once and remembered. A param is read through the caller's
//! [`Bindings`]; a read with no binding, an environment function, a probe or a node is *not
//! constant*, reported to the caller as such so that it can pick the code of its position
//! (`E0321`, or `P0001` for a param of the parent).
//!
//! The forms of phase A (mutation, loops, exits, patterns, closures, recursion) are `control.rs`:
//! a `const`, a `const fn`, a param default and a `where` evaluate them with their run-time value.
//! The aggregates of phase B (tuples, arrays, structs, enum variants with fields, `Option`s, ranges
//! and iterators, with their patterns, comparisons, field and element assignment and the rows of
//! Appendix H) are `composite.rs`; the public `folded`, `value` and `constant` give a scalar only, a
//! composite is given out as a [`ConstTree`] (`tree`, `constant_tree`). A module-body `let` written in generation folds
//! per iteration, and to the array of its iterations' values outside its loops (`generated.rs`).

mod bindings;
mod evaluator;
mod faults;

pub use bindings::Bindings;
pub use evaluator::{CompositeKind, ConstEval, ConstTree};
pub use faults::{ConstFault, Exit};
