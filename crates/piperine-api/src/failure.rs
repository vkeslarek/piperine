//! `Failure`: every way a request of the host library fails, typed, with what a host needs to
//! report it (design "Error Handling Strategy"). A failure is data: the library prints nothing.

use std::path::PathBuf;

use piperine_lang::msl::Diagnostic;
use piperine_lang::pom::{Design, DesignDiagnostic};
use piperine_project::PackageError;

/// Why a request failed.
#[derive(Debug)]
pub enum Failure {
    /// The package or a dependency broke a manifest rule, or could not be located (`E0170` to
    /// `E0176`).
    Package(Vec<PackageError>),
    /// A stage of the language (`parse`, `resolve`, `typeck`, `elaborate`) reported diagnostics;
    /// `design` owns the sources they point into.
    Diagnostics { stage: &'static str, design: Box<Design>, found: Vec<DesignDiagnostic> },
    /// A run request that is malformed (`P0010`), at a span of `text`, the content of `file`.
    Request { diagnostic: Diagnostic, file: PathBuf, text: String },
    /// A host error with a code and no span: `P0010` without a span, `P0011`, `E0801`, `E0802`.
    Host { code: &'static str, message: String },
    /// The backend refused or could not build the design; the message is the backend's.
    Backend(String),
    /// The solver failed an analysis.
    Solver { analysis: &'static str, error: piperine_solver::prelude::Error },
    /// A file could not be read or written.
    Io { path: PathBuf, error: std::io::Error },
}

impl Failure {
    /// The stage that failed, as a user reads it.
    pub fn stage(&self) -> &'static str {
        match self {
            Failure::Package(_) => "package",
            Failure::Diagnostics { stage, .. } => stage,
            Failure::Request { .. } => "request",
            Failure::Host { .. } => "host",
            Failure::Backend(_) => "backend",
            Failure::Solver { .. } => "solver",
            Failure::Io { .. } => "io",
        }
    }
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Failure::Package(errors) => {
                let codes: Vec<&str> = errors.iter().map(|e| &*e.diagnostic.code).collect();
                write!(f, "the package failed with {} error(s): {}", errors.len(), codes.join(", "))
            }
            Failure::Diagnostics { stage, found, .. } => write!(f, "{stage} failed with {} diagnostic(s)", found.len()),
            Failure::Request { diagnostic, file, .. } => write!(f, "{}: {} ({})", diagnostic.code, diagnostic.message, file.display()),
            Failure::Host { code, message } => write!(f, "{code}: {message}"),
            Failure::Backend(message) => f.write_str(message),
            Failure::Solver { analysis, error } => write!(f, "the {analysis} analysis failed: {error}"),
            Failure::Io { path, error } => write!(f, "{}: {error}", path.display()),
        }
    }
}

impl std::error::Error for Failure {}
