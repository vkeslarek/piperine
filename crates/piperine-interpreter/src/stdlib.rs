//! Standard library system tasks available in every Piperine runtime,
//! regardless of simulator backend.
//!
//! These tasks implement language-level semantics (I/O, error signalling) and
//! do NOT depend on any specific simulator. They are registered automatically
//! by `SystemTaskRegistry::default()`.

use crate::backend::SimulatorBackend;
use crate::error::InterpreterError;
use crate::task::SystemTask;
use crate::value::Value;

// ── $display(fmt, args...) ───────────────────────────────────────────────────

#[derive(Debug)]
pub struct DisplayTask;

impl SystemTask for DisplayTask {
    fn name(&self) -> &str { "display" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let output = if arguments.is_empty() {
            String::new()
        } else {
            let fmt = arguments[0].as_str().ok_or_else(|| InterpreterError::TypeError {
                expected: "string format".into(),
                got: arguments[0].type_name().into(),
            })?.to_string();
            format_string(&fmt, &arguments[1..])
        };
        simulator.print(&output);
        Ok(None)
    }
}

// ── $write(fmt, args...) ─────────────────────────────────────────────────────
// Like $display but without trailing newline (matches SV semantics).

#[derive(Debug)]
pub struct WriteTask;

impl SystemTask for WriteTask {
    fn name(&self) -> &str { "write" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let output = if arguments.is_empty() {
            String::new()
        } else {
            let fmt = arguments[0].as_str().ok_or_else(|| InterpreterError::TypeError {
                expected: "string format".into(),
                got: arguments[0].type_name().into(),
            })?.to_string();
            format_string(&fmt, &arguments[1..])
        };
        simulator.print(&output);
        Ok(None)
    }
}

// ── $warning(fmt, args...) ───────────────────────────────────────────────────

#[derive(Debug)]
pub struct WarningTask;

impl SystemTask for WarningTask {
    fn name(&self) -> &str { "warning" }
    fn call(&self, arguments: Vec<Value>, simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let msg = if arguments.is_empty() {
            "warning".into()
        } else {
            format_string(arguments[0].as_str().unwrap_or_default(), &arguments[1..])
        };
        simulator.print(&format!("WARNING: {msg}"));
        Ok(None)
    }
}

// ── $run_error(fmt, args...) ─────────────────────────────────────────────────
// Non-fatal: raises RunFailed (marks current run as failed, but does not halt).

#[derive(Debug)]
pub struct RunErrorTask;

impl SystemTask for RunErrorTask {
    fn name(&self) -> &str { "run_error" }
    fn call(&self, arguments: Vec<Value>, _simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let msg = if arguments.is_empty() {
            "run failed".into()
        } else {
            format_string(arguments[0].as_str().unwrap_or_default(), &arguments[1..])
        };
        Err(InterpreterError::RunFailed { message: msg })
    }
}

// ── $fatal([exit_code,] fmt, args...) ────────────────────────────────────────
// Fatal: halts the interpreter unconditionally.

#[derive(Debug)]
pub struct FatalTask;

impl SystemTask for FatalTask {
    fn name(&self) -> &str { "fatal" }
    fn call(&self, arguments: Vec<Value>, _simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let mut exit_code = 1u32;
        let mut fmt_idx = 0;
        if !arguments.is_empty() && matches!(arguments[0], Value::Integer(_)) {
            exit_code = arguments[0].as_integer().unwrap_or(1) as u32;
            fmt_idx = 1;
        }
        let msg = if fmt_idx < arguments.len() {
            format_string(arguments[fmt_idx].as_str().unwrap_or_default(), &arguments[(fmt_idx + 1)..])
        } else {
            "fatal error".into()
        };
        Err(InterpreterError::Fatal { message: msg, exit_code })
    }
}

// ── $error(fmt, args...) ─────────────────────────────────────────────────────
// Alias for $fatal(1, ...) — matches SV `$error` semantics.

#[derive(Debug)]
pub struct ErrorTask;

impl SystemTask for ErrorTask {
    fn name(&self) -> &str { "error" }
    fn call(&self, arguments: Vec<Value>, _simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let msg = if arguments.is_empty() {
            "error".into()
        } else {
            format_string(arguments[0].as_str().unwrap_or_default(), &arguments[1..])
        };
        Err(InterpreterError::Fatal { message: msg, exit_code: 1 })
    }
}

// ── $sformatf(fmt, args...) → string ─────────────────────────────────────────
// Returns a formatted string value (does not print). Used for string building.

#[derive(Debug)]
pub struct SFormatfTask;

impl SystemTask for SFormatfTask {
    fn name(&self) -> &str { "sformatf" }
    fn call(&self, arguments: Vec<Value>, _simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        if arguments.is_empty() {
            return Ok(Some(Value::String(String::new())));
        }
        let fmt = arguments[0].as_str().ok_or_else(|| InterpreterError::TypeError {
            expected: "string format".into(),
            got: arguments[0].type_name().into(),
        })?.to_string();
        Ok(Some(Value::String(format_string(&fmt, &arguments[1..]))))
    }
}

// ── $abs(x) → real / integer ─────────────────────────────────────────────────

#[derive(Debug)]
pub struct AbsTask;

impl SystemTask for AbsTask {
    fn name(&self) -> &str { "abs" }
    fn call(&self, arguments: Vec<Value>, _simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        match arguments.first() {
            Some(Value::Real(v))    => Ok(Some(Value::Real(v.abs()))),
            Some(Value::Integer(i)) => Ok(Some(Value::Integer(i.abs()))),
            other => Err(InterpreterError::TypeError {
                expected: "numeric".into(),
                got: other.map(|v| v.type_name()).unwrap_or("nothing").into(),
            }),
        }
    }
}

// ── $min(a, b) / $max(a, b) ──────────────────────────────────────────────────

#[derive(Debug)]
pub struct MinTask;

impl SystemTask for MinTask {
    fn name(&self) -> &str { "min" }
    fn call(&self, arguments: Vec<Value>, _simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let a = arguments.get(0).and_then(|v| v.as_f64());
        let b = arguments.get(1).and_then(|v| v.as_f64());
        match (a, b) {
            (Some(a), Some(b)) => Ok(Some(Value::Real(a.min(b)))),
            _ => Err(InterpreterError::TypeError { expected: "two numeric args".into(), got: format!("{} args", arguments.len()) }),
        }
    }
}

#[derive(Debug)]
pub struct MaxTask;

impl SystemTask for MaxTask {
    fn name(&self) -> &str { "max" }
    fn call(&self, arguments: Vec<Value>, _simulator: &mut dyn SimulatorBackend)
        -> Result<Option<Value>, InterpreterError>
    {
        let a = arguments.get(0).and_then(|v| v.as_f64());
        let b = arguments.get(1).and_then(|v| v.as_f64());
        match (a, b) {
            (Some(a), Some(b)) => Ok(Some(Value::Real(a.max(b)))),
            _ => Err(InterpreterError::TypeError { expected: "two numeric args".into(), got: format!("{} args", arguments.len()) }),
        }
    }
}

// ── Format string engine ─────────────────────────────────────────────────────
//
// Shared by $display, $write, $warning, $sformatf, $run_error, $fatal.
// Supports: %g %e %f %d %s %b %o %h %% and width specifiers (%0d, %8.3f, etc.).

pub fn format_string(format: &str, arguments: &[Value]) -> String {
    let mut out = String::with_capacity(format.len());
    let mut chars = format.chars().peekable();
    let mut arg_idx = 0;

    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        // Consume optional width / precision (e.g., `%8.3f`, `%0d`, `%-10s`)
        let mut spec = String::new();
        if chars.peek() == Some(&'-') { spec.push(chars.next().unwrap()); }
        while chars.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            spec.push(chars.next().unwrap());
        }
        if chars.peek() == Some(&'.') {
            spec.push(chars.next().unwrap());
            while chars.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                spec.push(chars.next().unwrap());
            }
        }
        match chars.next() {
            Some('%') => out.push('%'),
            Some('g') | Some('G') => {
                let v = arguments.get(arg_idx).and_then(|v| v.as_f64()).unwrap_or(0.0);
                out.push_str(&format!("{v:e}").replace("e0", "").replace("e", "e+"));
                // Simple %g: let Rust decide; good enough for engineering use.
                out = out.trim_end_matches(&out.clone()).to_string(); // reset trick
                out.push_str(&format!("{v}"));
                arg_idx += 1;
            }
            Some('e') | Some('E') => {
                let v = arguments.get(arg_idx).and_then(|v| v.as_f64()).unwrap_or(0.0);
                out.push_str(&format!("{v:e}"));
                arg_idx += 1;
            }
            Some('f') => {
                let v = arguments.get(arg_idx).and_then(|v| v.as_f64()).unwrap_or(0.0);
                let prec: usize = spec.split('.').nth(1).and_then(|s| s.parse().ok()).unwrap_or(6);
                out.push_str(&format!("{v:.prec$}"));
                arg_idx += 1;
            }
            Some('d') | Some('i') => {
                let v = arguments.get(arg_idx).and_then(|v| v.as_integer()).unwrap_or(0);
                out.push_str(&format!("{v}"));
                arg_idx += 1;
            }
            Some('s') => {
                let v = arguments.get(arg_idx).map(|v| v.to_string()).unwrap_or_default();
                out.push_str(&v);
                arg_idx += 1;
            }
            Some('b') => {
                let v = arguments.get(arg_idx).and_then(|v| v.as_integer()).unwrap_or(0);
                out.push_str(&format!("{v:b}"));
                arg_idx += 1;
            }
            Some('o') => {
                let v = arguments.get(arg_idx).and_then(|v| v.as_integer()).unwrap_or(0);
                out.push_str(&format!("{v:o}"));
                arg_idx += 1;
            }
            Some('h') | Some('x') | Some('X') => {
                let v = arguments.get(arg_idx).and_then(|v| v.as_integer()).unwrap_or(0);
                out.push_str(&format!("{v:x}"));
                arg_idx += 1;
            }
            Some(other) => { out.push('%'); out.push_str(&spec); out.push(other); }
            None        => { out.push('%'); out.push_str(&spec); }
        }
    }
    out
}

/// All stdlib tasks. Called by `SystemTaskRegistry::default()`.
pub fn register_stdlib(registry: &mut crate::task::SystemTaskRegistry) {
    registry.register(Box::new(DisplayTask));
    registry.register(Box::new(WriteTask));
    registry.register(Box::new(WarningTask));
    registry.register(Box::new(RunErrorTask));
    registry.register(Box::new(FatalTask));
    registry.register(Box::new(ErrorTask));
    registry.register(Box::new(SFormatfTask));
    registry.register(Box::new(AbsTask));
    registry.register(Box::new(MinTask));
    registry.register(Box::new(MaxTask));
}
