//! Emit-and-validation contract (SPEC §11).
//!
//! The emitter must produce only what the codegen implements; validation is
//! the checked half of that contract. Currently a stub — full POM `Stmt`
//! validation will be re-added as needed.

use super::pom::LoweredBody;

/// How bad a validation finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticKind {
    Error,
    Warning,
}

/// One validation finding.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub message: String,
}

impl LoweredBody {
    /// Validate this body. Currently returns no diagnostics (stub).
    pub fn validate(&self) -> Vec<Diagnostic> {
        Vec::new()
    }
}
