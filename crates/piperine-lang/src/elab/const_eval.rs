use crate::eval::const_host::ConstHost;
use crate::eval::{EvalError, Interpreter};
use crate::parse::ast::Expr;
use crate::value::Value;
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during constant evaluation.
#[derive(Debug, Error)]
pub enum ConstEvalError {
    /// The expression cannot be reduced to a compile-time constant.
    #[error("expression is not a compile-time constant: {0}")]
    NotConst(String),
    /// Division or remainder by zero was attempted.
    #[error("division by zero")]
    DivByZero,
    /// A name used in the expression is not bound in the const environment.
    #[error("undefined name: {0}")]
    Undefined(String),
    /// The operands of a const expression have incompatible types.
    #[error("type mismatch in constant expression")]
    TypeMismatch,
}

/// Narrow a general evaluation error down to the smaller const-eval error
/// set. Anything not representable at compile time (an effectful system
/// task, a host-owned assignment, ...) is reported as `NotConst` — the same
/// outcome the legacy const-folder gave for any construct it didn't handle.
impl From<EvalError> for ConstEvalError {
    fn from(e: EvalError) -> Self {
        match e {
            EvalError::Undefined(name) => ConstEvalError::Undefined(name),
            EvalError::DivByZero => ConstEvalError::DivByZero,
            EvalError::TypeMismatch(_) => ConstEvalError::TypeMismatch,
            EvalError::NotConst(msg) => ConstEvalError::NotConst(msg),
            other => ConstEvalError::NotConst(other.to_string()),
        }
    }
}

/// A lexical-scope environment for compile-time constant evaluation.
/// Maintains a stack of scopes; `push`/`pop` manage nesting (e.g. for
/// loop bodies), `define` adds a binding to the innermost scope, and
/// `lookup` searches scopes from innermost outward.
///
/// `eval` delegates to the shared [`crate::eval::Interpreter`], run against
/// a pure [`ConstHost`] backed by this environment's binding stack — the
/// same engine a `bench` uses, restricted to constructs valid at
/// elaboration time (no analyses, no staging). Values are the one
/// [`Value`] type (SIMPLIFICATION.md P2); a result that isn't a
/// compile-time scalar is rejected here with `NotConst`.
pub struct ConstEnv {
    bindings: Vec<HashMap<String, Value>>,
}

impl ConstEnv {
    /// Creates a new `ConstEnv` with a single empty scope.
    pub fn new() -> Self {
        Self { bindings: vec![HashMap::new()] }
    }

    /// Creates a new `ConstEnv` pre-populated with global constants.
    pub fn with_globals(globals: HashMap<String, Value>) -> Self {
        Self { bindings: vec![globals] }
    }

    /// Pushes a new empty scope onto the bindings stack (e.g. before
    /// entering a loop body).
    pub fn push(&mut self) {
        self.bindings.push(HashMap::new());
    }

    /// Pops the innermost scope from the bindings stack (e.g. after
    /// leaving a loop body).
    pub fn pop(&mut self) {
        self.bindings.pop();
    }

    /// Defines a name in the current innermost scope, binding it to `val`.
    pub fn define(&mut self, name: String, val: Value) {
        self.bindings.last_mut().unwrap().insert(name, val);
    }

    /// Looks up `name` in the scope stack, from innermost outward.
    /// Returns `None` if the name is unbound.
    pub fn lookup(&self, name: &str) -> Option<&Value> {
        self.bindings.iter().rev().find_map(|scope| scope.get(name))
    }

    /// Evaluates a compile-time expression to a scalar [`Value`]. Supports
    /// the full `fn`-body grammar (SPEC Part I §9) evaluated by the shared
    /// interpreter; a non-scalar result (list, closure, ...) is `NotConst`.
    pub fn eval(&self, expr: &Expr) -> Result<Value, ConstEvalError> {
        let mut host = ConstHost::new(&self.bindings);
        let mut interp = Interpreter::new(&mut host);
        let value = interp.eval_expr(expr)?;
        if value.is_const_scalar() {
            Ok(value)
        } else {
            Err(ConstEvalError::NotConst(format!(
                "{} is not a compile-time constant",
                value.type_name()
            )))
        }
    }

    /// Evaluates `expr` and returns the result as a `u64`. Accepts `Nat`
    /// values directly and non-negative `Int` values.
    pub fn eval_nat(&self, expr: &Expr) -> Result<u64, ConstEvalError> {
        match self.eval(expr)? {
            Value::Nat(n) => Ok(n),
            Value::Int(n) if n >= 0 => Ok(n as u64),
            _ => Err(ConstEvalError::TypeMismatch),
        }
    }

    /// Evaluates `expr` and returns the result as an `i64`. Accepts `Int`
    /// values directly and `Nat` values widened to `i64`.
    pub fn eval_int(&self, expr: &Expr) -> Result<i64, ConstEvalError> {
        match self.eval(expr)? {
            Value::Int(n) => Ok(n),
            Value::Nat(n) => Ok(n as i64),
            _ => Err(ConstEvalError::TypeMismatch),
        }
    }
}

impl Default for ConstEnv {
    fn default() -> Self {
        Self::new()
    }
}
