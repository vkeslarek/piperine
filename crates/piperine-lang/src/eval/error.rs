//! [`EvalError`] — everything that can go wrong while walking an [`Expr`]/[`Stmt`]
//! tree, whether in const-eval or in a `bench`.
//!
//! [`Expr`]: crate::parse::ast::Expr
//! [`Stmt`]: crate::parse::ast::Stmt

use thiserror::Error;

#[derive(Debug, Error)]
pub enum EvalError {
    /// A name used in the expression is not bound.
    #[error("undefined name: {0}")]
    Undefined(String),
    /// The operands of an expression have incompatible types.
    #[error("type mismatch: {0}")]
    TypeMismatch(String),
    /// Division or remainder by zero was attempted.
    #[error("division by zero")]
    DivByZero,
    /// The expression cannot be reduced to a compile-time constant
    /// (surfaced by [`crate::elab::const_eval::ConstEnv`]).
    #[error("expression is not a compile-time constant: {0}")]
    NotConst(String),
    /// A system task was called from a context that does not have it
    /// (e.g. `$op()` from pure/const-eval code, or an unimplemented task).
    #[error("`${name}` is not available in {context}")]
    TaskUnavailable { name: String, context: &'static str },
    /// `$assert` failed.
    #[error("assertion failed: {0}")]
    AssertFailed(String),
    /// `$fatal` or `$error` raised explicitly.
    #[error("{0}")]
    Fatal(String),
    /// An error raised by a [`Host`][super::interp::Host] implementation
    /// (POM resolution, staging, solver failures, ...).
    #[error("{0}")]
    Host(String),
}
