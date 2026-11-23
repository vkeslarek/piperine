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
    /// `run` named no run of the project (`asked`), or named none while the project has no run
    /// or several: a usage error, with the project's run names.
    NoRun { asked: Option<String>, runs: Vec<String> },
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
            Failure::Request { .. } | Failure::NoRun { .. } => "request",
            Failure::Host { .. } => "host",
            Failure::Backend(_) => "backend",
            Failure::Solver { .. } => "solver",
            Failure::Io { .. } => "io",
        }
    }
}

/// What a model panic under known control looks like in the text of a solver fault: the panic's own
/// message, the instance that ran it and, when the fault is the digital engine's, its tick (the
/// one decoding place of a device fault, design D19).
struct PanicFault {
    /// The panic's message, without the `panic: ` the ABI's thunk puts before it.
    operation: String,
    /// The path of the instance whose function panicked.
    instance: String,
    /// The tick of a digital fault, which the analog engine's fault does not carry.
    tick: Option<String>,
}

impl PanicFault {
    /// The marker the ABI's thunk writes before the text of a panic (`piperine-abi` `thunk.rs`).
    const MARKER: &'static str = "panic: ";

    /// The panic `text` carries, or `None` when the fault is not a model panic.
    fn read(text: &str) -> Option<PanicFault> {
        let (before, after) = text.split_once(Self::MARKER)?;
        let operation = after.lines().next().unwrap_or(after).trim().to_string();
        let instance = Self::between(before, "instance `", "`").or_else(|| Self::between(before, "in `", "`")).unwrap_or_else(|| "an unknown instance".to_string());
        let tick = Self::between(before, " at tick ", ":");
        Some(PanicFault { operation, instance, tick })
    }

    /// The text of `text` after the first `start` up to the next `end`.
    fn between(text: &str, start: &str, end: &str) -> Option<String> {
        let rest = text.split_once(start)?.1;
        Some(rest.split_once(end)?.0.to_string())
    }

    /// A spec code the panic's message starts with (`E0705: ..`), and the message without it.
    fn leading_code(&self) -> Option<(&'static str, &str)> {
        // the codes a model panic may carry as its own: `E0705` is `table_model` outside its table under `E`
        const CODES: [&str; 2] = ["E0105", "E0705"];
        let code = CODES.iter().find(|code| self.operation.starts_with(&format!("{code}: ")))?;
        Some((code, &self.operation[code.len() + 2..]))
    }
}

impl Failure {
    /// The failure of `analysis`, which stopped with the solver's `error`. A device fault that is a
    /// model panic under known control is `E0105` naming the operation, the instance and the time;
    /// a panic whose message starts with a spec code keeps that code (design D19). `time` is the
    /// simulated time the run had reached, when the analysis has one; a digital fault gives its
    /// own tick. Any other error is the solver's, as it was.
    pub fn from_fault(analysis: &'static str, error: piperine_solver::prelude::Error, time: Option<f64>) -> Failure {
        let Some(panic) = PanicFault::read(&error.to_string()) else { return Failure::Solver { analysis, error } };
        let (code, operation) = panic.leading_code().unwrap_or(("E0105", panic.operation.as_str()));
        let at = match (&panic.tick, time) {
            (Some(tick), _) => format!("tick {tick}"),
            (None, Some(seconds)) => format!("t = {seconds:e} s"),
            (None, None) => format!("the {analysis} analysis"),
        };
        Failure::Host { code, message: format!("a model panicked under known control in `{}` at {at}: {operation}", panic.instance) }
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
            Failure::NoRun { asked, runs } => {
                let listed = if runs.is_empty() { "the project has no run".to_string() } else { format!("the runs are: {}", runs.join(", ")) };
                match asked {
                    Some(name) => write!(f, "no run named `{name}`: {listed}"),
                    None if runs.is_empty() => f.write_str("no run to choose: the project has no run"),
                    None => write!(f, "several runs, name one: {listed}"),
                }
            }
            Failure::Host { code, message } => write!(f, "{code}: {message}"),
            Failure::Backend(message) => f.write_str(message),
            Failure::Solver { analysis, error } => write!(f, "the {analysis} analysis failed: {error}"),
            Failure::Io { path, error } => write!(f, "{}: {error}", path.display()),
        }
    }
}

impl std::error::Error for Failure {}
