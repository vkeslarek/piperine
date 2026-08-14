//! # Evaluator
//!
//! **MD-45 (2026-08-12):** the PHDL-`Stmt`/`Expr`-shaped tree-walking
//! interpreter (`interp::Interpreter`/`interp::Host`), its pure const-eval
//! host (`const_host::ConstHost`) and its system-task registry
//! (`tasks::TaskRegistry`) were deleted with `piperine-lang/src/parse/` —
//! they were built directly against the PHDL `fn`-body grammar (SPEC Part I
//! §9), which has no VAMS equivalent. `crate::elab::const_eval::ConstEnv`
//! now evaluates `crate::ast::Expr` directly (VAMS's own expression
//! grammar has no blocks/lambdas/statements to walk, so it needs no
//! host-trait indirection). A runtime evaluator for VAMS `analog
//! function`/`task` bodies, if one is needed beyond const-eval, is future
//! work — not reintroduced speculatively here.

pub mod error;
pub mod tasks;
pub use crate::value;

pub use error::EvalError;
pub use tasks::{
    Conversions, SimEnvironment, SystemTaskError, SystemTaskKind, SystemTasks, VpiTaskKind, VpiTaskRegistry,
    format_display,
};
pub use value::{Closure, InvokeClosure, Object, Value};
