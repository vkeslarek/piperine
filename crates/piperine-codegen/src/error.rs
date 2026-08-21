//! The crate-wide error type: every unimplemented lowering or JIT-compile
//! failure is a *named* error — nothing ever silently compiles to `0.0`.

use thiserror::Error;

/// Errors from IR validation and JIT compilation.
#[derive(Debug, Error)]
pub enum CodegenError {
    #[error("module `{0}` not found")]
    ModuleNotFound(String),
    #[error("IR validation failed: {0}")]
    Invalid(String),
    #[error("Cranelift module error: {0}")]
    Module(String),
    #[error("unsupported construct: {0}")]
    Unsupported(String),
    #[error("constant evaluation failed: {0}")]
    ConstEval(String),
    #[error("function lowering failed: {0}")]
    Function(String),
}

impl CodegenError {
    pub fn unsupported(what: impl Into<String>) -> Self {
        Self::Unsupported(what.into())
    }

    /// A system task name (already stripped of its leading `$`) that the
    /// parser/elaborator accepted but this lowering arm does not implement
    /// — LRM §9.2 ("System Tasks and Functions"). Shared by the analog
    /// (`flatten::analog`) and digital (`emit::stmt`) `TaskCall` arms so the
    /// two paths cannot silently drift back apart (T13, LRMW2-09).
    pub fn unknown_task(name: &str) -> Self {
        Self::Unsupported(format!("unrecognized system task `${name}` (LRM §9.2)"))
    }
}
