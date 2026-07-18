//! [`ConstHost`] — the pure [`Host`] backing [`crate::elab::const_eval::ConstEnv`].
//!
//! No POM access, no staging, no effectful tasks: only the persistent
//! binding stack `ConstEnv` maintains across `push`/`pop`/`define` calls and
//! the shared pure task registry (`$assert`, diagnostics, math).

use std::collections::HashMap;

use crate::parse::ast::Expr;

use super::error::EvalError;
use super::interp::Host;
use super::tasks::{dispatch_pure, TaskRegistry};
use super::value::Value;

pub struct ConstHost<'a> {
    scopes: &'a [HashMap<String, Value>],
    tasks: TaskRegistry,
}

impl<'a> ConstHost<'a> {
    pub fn new(scopes: &'a [HashMap<String, Value>]) -> Self {
        Self { scopes, tasks: TaskRegistry::with_builtins() }
    }
}

impl Host for ConstHost<'_> {
    fn context_name(&self) -> &'static str {
        "a compile-time constant expression"
    }

    fn lookup(&mut self, name: &str) -> Option<Value> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name).cloned())
    }

    fn syscall(&mut self, name: &str, args: Vec<Value>) -> Result<Value, EvalError> {
        dispatch_pure(&self.tasks, name, args)
            .unwrap_or_else(|| Err(EvalError::TaskUnavailable { name: name.to_string(), context: self.context_name() }))
    }

    fn assign(&mut self, _target: &Expr, _value: &Value) -> Result<bool, EvalError> {
        // Constant expressions never mutate anything; not host-owned.
        Ok(false)
    }
}
