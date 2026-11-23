//! The function class of a body (part I §3): what it touches decides where it may be called. The
//! checker infers the classes that restrict a call: a body that reads the environment of the solve
//! is solve-reading, one that reaches `emit!` is emitting; every other body is pure.

use super::check::Context;

/// The classes of a function or a closure; a body may belong to several, and the restrictions add
/// up (§3).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct FunctionClass {
    /// Reads `now()`, `freq()`, `gmin()`, `source_scale()` or `analysis(..)`.
    pub(super) solve_reading: bool,
    /// Reaches `emit!`.
    pub(super) emitting: bool,
}

impl FunctionClass {
    /// The class of a body that belongs to every class of `self` and of `other`.
    pub(super) fn union(self, other: FunctionClass) -> FunctionClass {
        FunctionClass { solve_reading: self.solve_reading || other.solve_reading, emitting: self.emitting || other.emitting }
    }

    /// The name of the first class of `self` that a call written in `context` does not allow
    /// (§3 table: a solve-reading body is callable where the solve is running, an emitting one in
    /// a reaction or in `analog`); `None` when the call is allowed.
    pub(super) fn refused_in(self, context: Context) -> Option<&'static str> {
        match context {
            Context::Const if self.solve_reading => Some("solve-reading"),
            Context::Const | Context::Module if self.emitting => Some("emitting"),
            _ => None,
        }
    }
}
