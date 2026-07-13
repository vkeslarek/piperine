//! System tasks available in every context: `$assert`, the diagnostic
//! family, and (via the interpreter's `Call` handling, not this registry)
//! the bare-name math catalog.
//!
//! Bench-only tasks (`$op`, ...) live in `piperine-bench`, implementing
//! their own `BenchTask` trait and falling back to [`dispatch_pure`] for
//! anything not effectful.

use std::collections::HashMap;

use super::error::EvalError;
use super::value::Value;

/// A system task, callable via `$name(args)`.
pub trait Task {
    fn name(&self) -> &'static str;
    fn eval(&self, args: Vec<Value>) -> Result<Value, EvalError>;
}

/// Names reachable from a pure fn/method body (bench spec §1/§11):
/// diagnostics, asserts, and `$display`, but never an analysis or staging
/// task.
pub fn is_pure(name: &str) -> bool {
    matches!(name, "assert" | "info" | "warn" | "error" | "fatal" | "display")
}

/// Names a `bench` may call today (bench spec §7/§11 availability table):
/// the pure diagnostics plus the four analyses and `$write`. What remains
/// (`$plot`, `extract`) is recognized syntax but not yet implemented —
/// elaboration rejects a bench that calls it (fail-loud, never a silent
/// no-op).
pub fn bench_task_implemented(name: &str) -> bool {
    is_pure(name) || matches!(name, "op" | "tran" | "ac" | "noise" | "write")
}

struct Assert;
impl Task for Assert {
    fn name(&self) -> &'static str { "assert" }
    fn eval(&self, args: Vec<Value>) -> Result<Value, EvalError> {
        let (cond, msg) = two_args(args)?;
        if cond.is_truthy() {
            Ok(Value::Unit)
        } else {
            Err(EvalError::AssertFailed(display(&msg)))
        }
    }
}

struct Diagnostic {
    name: &'static str,
    prefix: &'static str,
    fatal: bool,
}
impl Task for Diagnostic {
    fn name(&self) -> &'static str { self.name }
    fn eval(&self, args: Vec<Value>) -> Result<Value, EvalError> {
        let message = args.iter().map(display).collect::<Vec<_>>().join(" ");
        if self.fatal {
            return Err(EvalError::Fatal(message));
        }
        println!("[{}] {}", self.prefix, message);
        Ok(Value::Unit)
    }
}

/// `$display(args…)` — the Verilog-family print: arguments rendered and
/// joined by a space, no severity prefix, trailing newline.
struct Display;
impl Task for Display {
    fn name(&self) -> &'static str { "display" }
    fn eval(&self, args: Vec<Value>) -> Result<Value, EvalError> {
        println!("{}", args.iter().map(display).collect::<Vec<_>>().join(" "));
        Ok(Value::Unit)
    }
}

fn two_args(mut args: Vec<Value>) -> Result<(Value, Value), EvalError> {
    if args.len() != 2 {
        return Err(EvalError::TypeMismatch(format!("expected 2 arguments, got {}", args.len())));
    }
    let msg = args.remove(1);
    let cond = args.remove(0);
    Ok((cond, msg))
}

fn display(v: &Value) -> String {
    let join = |xs: &[Value]| xs.iter().map(display).collect::<Vec<_>>().join(", ");
    match v {
        Value::Unit => "()".into(),
        Value::Str(s) => s.clone(),
        Value::Real(r) => r.to_string(),
        Value::Nat(n) => n.to_string(),
        Value::Int(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Complex(re, im) => format!("{re}{im:+}j"),
        Value::EnumVariant(e, variant) => format!("{e}::{variant}"),
        Value::Tuple(xs) => format!("({})", join(xs)),
        Value::List(xs) => format!("[{}]", join(&xs.borrow())),
        Value::Map(kvs) => {
            let body = kvs
                .borrow()
                .iter()
                .map(|(k, v)| format!("{}: {}", display(k), display(v)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("Map {{ {body} }}")
        }
        Value::Set(items) => {
            let body = items.borrow().iter().map(display).collect::<Vec<_>>().join(", ");
            format!("Set {{ {body} }}")
        }
        Value::Result(Ok(v)) => format!("Ok({})", display(v)),
        Value::Result(Err(e)) => format!("Err({})", display(e)),
        Value::Option(None) => "None".into(),
        Value::Option(Some(x)) => format!("Some({})", display(x)),
        Value::Object(o) => o.render(),
        other => format!("<{}>", other.type_name()),
    }
}

/// The shared registry of context-independent tasks.
pub struct TaskRegistry(HashMap<&'static str, Box<dyn Task>>);

impl TaskRegistry {
    pub fn with_builtins() -> Self {
        let tasks: Vec<Box<dyn Task>> = vec![
            Box::new(Assert),
            Box::new(Display),
            Box::new(Diagnostic { name: "info", prefix: "info", fatal: false }),
            Box::new(Diagnostic { name: "warn", prefix: "warn", fatal: false }),
            Box::new(Diagnostic { name: "error", prefix: "error", fatal: true }),
            Box::new(Diagnostic { name: "fatal", prefix: "fatal", fatal: true }),
        ];
        Self(tasks.into_iter().map(|t| (t.name(), t)).collect())
    }

    pub fn lookup(&self, name: &str) -> Option<&dyn Task> {
        self.0.get(name).map(|b| b.as_ref())
    }
}

/// Try the shared pure registry, then the bare-name math catalog
/// (`crate::math`) called with a `$`-prefix (e.g. `$sqrt`).
/// Returns `None` for anything neither recognizes, letting a `Host` report
/// `TaskUnavailable`.
pub fn dispatch_pure(registry: &TaskRegistry, name: &str, args: Vec<Value>) -> Option<Result<Value, EvalError>> {
    if let Some(task) = registry.lookup(name) {
        return Some(task.eval(args));
    }
    if crate::math::math_fn(name).is_some() {
        let floats: Result<Vec<f64>, EvalError> = args
            .iter()
            .map(|v| match v {
                Value::Real(r) => Ok(*r),
                Value::Nat(n) => Ok(*n as f64),
                Value::Int(n) => Ok(*n as f64),
                other => Err(EvalError::TypeMismatch(format!("expected a Real, got {}", other.type_name()))),
            })
            .collect();
        return Some(floats.and_then(|floats| {
            crate::math::eval_const_math(name, &floats)
                .map(Value::Real)
                .ok_or_else(|| EvalError::TypeMismatch(format!("bad arguments to ${name}")))
        }));
    }
    None
}
