//! Bench-only system tasks: effectful, need a [`SimSession`] to run. Each
//! is a unit struct implementing [`SimTask`] — the bench-side counterpart
//! to [`piperine_lang::eval::Task`] (the shared pure registry, consulted as
//! a fallback by [`crate::host::SimHost::syscall`]).

use std::collections::HashMap;

use piperine_lang::eval::{EvalError, Value};

use crate::session::{SimSession, SolverConfig};

/// A bench-only system task (`$op`, and later `$tran`/`$ac`/`$noise`).
pub trait SimTask {
    fn name(&self) -> &'static str;
    fn run(&self, args: Vec<Value>, session: &SimSession) -> Result<Value, EvalError>;
}

struct Op;
impl SimTask for Op {
    fn name(&self) -> &'static str {
        "op"
    }
    fn run(&self, _args: Vec<Value>, session: &SimSession) -> Result<Value, EvalError> {
        // Milestone 1: no config-bundle argument yet — always the default
        // solver configuration (SPEC_BENCH.md §5.1 `OpConfig {}`).
        let result = session.run_op(&SolverConfig::default()).map_err(EvalError::from)?;
        Ok(Value::Object(std::rc::Rc::new(result)))
    }
}

struct Tran;
impl SimTask for Tran {
    fn name(&self) -> &'static str {
        "tran"
    }
    fn run(&self, args: Vec<Value>, session: &SimSession) -> Result<Value, EvalError> {
        // Milestone 1: positional `(stop, step)`, not yet the `TranConfig`
        // bundle SPEC_BENCH.md §5.1 describes — `step` is required (no
        // adaptive-step "auto" sentinel yet).
        let stop = as_real(args.first())?;
        let step = as_real(args.get(1))?;
        let trace = session.run_tran(stop, step, &SolverConfig::default()).map_err(EvalError::from)?;
        Ok(Value::Object(std::rc::Rc::new(trace)))
    }
}

fn as_real(v: Option<&Value>) -> Result<f64, EvalError> {
    match v {
        Some(Value::Real(r)) => Ok(*r),
        Some(Value::Nat(n)) => Ok(*n as f64),
        Some(Value::Int(n)) => Ok(*n as f64),
        _ => Err(EvalError::TypeMismatch("expected a Real argument".into())),
    }
}

/// The bench-only task registry.
pub struct SimTaskRegistry(HashMap<&'static str, Box<dyn SimTask>>);

impl SimTaskRegistry {
    pub fn with_builtins() -> Self {
        let tasks: Vec<Box<dyn SimTask>> = vec![Box::new(Op), Box::new(Tran)];
        Self(tasks.into_iter().map(|t| (t.name(), t)).collect())
    }

    pub fn lookup(&self, name: &str) -> Option<&dyn SimTask> {
        self.0.get(name).map(|b| b.as_ref())
    }
}
