//! Compile-time constant evaluation over `crate::ast::Expr`.
//!
//! **MD-45 note:** before the PHDL→VAMS frontend swap this delegated to
//! `crate::eval::Interpreter` (a general tree-walking evaluator shared with
//! runtime const-folding). That evaluator was PHDL-`Stmt`/`Expr`-shaped and
//! died with `parse/`. VAMS's own `Expr` (LRM ch. 4, `crate::ast::
//! Expr`) is self-contained — no blocks, lambdas, or statements to walk —
//! so this is a small direct recursive evaluator instead of a host-trait
//! indirection; `crate::eval::interp` is rebuilt separately (T24) for
//! whatever richer runtime const-folding VAMS actually needs (e.g.
//! `analog function` bodies), not reintroduced here speculatively.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::ast::{BinOp, Expr, UnOp};
use crate::lexer::NumberLit;
use thiserror::Error;

use crate::value::logic::{Base, LogicVec};
use crate::value::Value;

/// Errors that can occur during constant evaluation.
#[derive(Debug, Clone, Error)]
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
    /// A based literal's digit is illegal for its declared base (e.g. `'d1F`).
    /// LRMC-C1-01's full per-base validation lands at the lexer (a later
    /// task); this is the const-eval-level backstop so an illegal digit
    /// never silently becomes `0` in the meantime.
    #[error("illegal digit for base in literal `{0}` (LRM §2.6.1)")]
    IllegalBasedDigit(String),
    /// A four-state (`x`/`z`) value flowed into a context with no defined
    /// four-state numeric semantics (array bound, bit width, generate
    /// bound) — LRM §2.6.1 defines `x`/`z` only for logic/net contexts, not
    /// as an integer. Never silently truncated to a two-state value (spec
    /// Edge Case).
    #[error("a four-state (x/z) value has no defined numeric interpretation here (LRM §2.6.1) — `{0}`")]
    FourStateInNumericContext(String),
    /// An assignment-pattern array initializer's element count doesn't
    /// match its declared dimension — LRMC-C1-06 (§3.4.4/§3.4.8): shape
    /// mismatch fails loud, never truncates/pads silently.
    #[error("array initializer has {found} element(s), expected {expected} (LRM §3.4.4/§3.4.8)")]
    ArityMismatch { expected: u64, found: usize },
}

/// A lexical-scope environment for compile-time constant evaluation.
/// Maintains a stack of scopes; `push`/`pop` manage nesting (e.g. for
/// generate-loop bodies), `define` adds a binding to the innermost scope,
/// and `lookup` searches scopes from innermost outward.
#[derive(Clone)]
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
    /// entering a generate-loop body).
    pub fn push(&mut self) {
        self.bindings.push(HashMap::new());
    }

    /// Pops the innermost scope from the bindings stack.
    pub fn pop(&mut self) {
        self.bindings.pop();
    }

    /// Defines a name in the current innermost scope, binding it to `val`.
    pub fn define(&mut self, name: String, val: Value) {
        self.bindings.last_mut().unwrap().insert(name, val);
    }

    /// Looks up `name` in the scope stack, from innermost outward.
    pub fn lookup(&self, name: &str) -> Option<&Value> {
        self.bindings.iter().rev().find_map(|scope| scope.get(name))
    }

    /// Evaluates a compile-time expression to a scalar [`Value`].
    pub fn eval(&self, expr: &Expr) -> Result<Value, ConstEvalError> {
        match expr {
            Expr::Number { value, .. } => number_lit_to_value(value),
            Expr::Str { value, .. } => Ok(Value::Str(value.clone())),
            Expr::Ident { name, .. } => self
                .lookup(name)
                .cloned()
                .ok_or_else(|| ConstEvalError::Undefined(name.clone())),
            Expr::Unary { op, operand, .. } => self.eval_unary(*op, operand),
            Expr::Binary { op, lhs, rhs, .. } => self.eval_binary(*op, lhs, rhs),
            Expr::Ternary { cond, then_branch, else_branch, .. } => {
                match self.eval(cond)? {
                    Value::Bool(true) => self.eval(then_branch),
                    Value::Bool(false) => self.eval(else_branch),
                    Value::Nat(n) => self.eval(if n != 0 { then_branch } else { else_branch }),
                    Value::Int(n) => self.eval(if n != 0 { then_branch } else { else_branch }),
                    _ => Err(ConstEvalError::TypeMismatch),
                }
            }
            Expr::Call { name, args, .. } => self.eval_call(name, args),
            Expr::Concat { .. }
            | Expr::Replication { .. }
            | Expr::Index { .. }
            | Expr::PartSelect { .. }
            | Expr::AccessFunction { .. }
            | Expr::PortAccess { .. }
            | Expr::SystemFunctionCall { .. } => Err(ConstEvalError::NotConst(
                "not yet supported by the elaboration-constant evaluator".into(),
            )),
            // LRMC-C1-06's FUNCTIONALITY half (parameter arrays, string
            // valid-value lists, multidimensional initializers) is a
            // follow-up task — this scalar `eval` is not where it lands
            // (assignment patterns are array-shaped, not scalar). Fail
            // loud rather than silently mis-evaluating as a scalar.
            Expr::AssignmentPattern { .. } => Err(ConstEvalError::NotConst(
                "assignment patterns are not yet supported by the scalar elaboration-constant evaluator".into(),
            )),
        }
    }

    fn eval_unary(&self, op: UnOp, operand: &Expr) -> Result<Value, ConstEvalError> {
        let v = self.eval(operand)?;
        match (op, v) {
            (UnOp::Neg, Value::Real(r)) => Ok(Value::Real(-r)),
            (UnOp::Neg, Value::Int(i)) => Ok(Value::Int(-i)),
            (UnOp::Neg, Value::Nat(n)) => Ok(Value::Int(-(n as i64))),
            (UnOp::Plus, v @ (Value::Real(_) | Value::Int(_) | Value::Nat(_))) => Ok(v),
            (UnOp::LogNot, Value::Bool(b)) => Ok(Value::Bool(!b)),
            (UnOp::BitNot, Value::Int(i)) => Ok(Value::Int(!i)),
            (UnOp::BitNot, Value::Nat(n)) => Ok(Value::Int(!(n as i64))),
            _ => Err(ConstEvalError::TypeMismatch),
        }
    }

    fn eval_binary(&self, op: BinOp, lhs: &Expr, rhs: &Expr) -> Result<Value, ConstEvalError> {
        let l = self.eval(lhs)?;
        let r = self.eval(rhs)?;
        // Logical/relational ops share a numeric coercion; arithmetic ops
        // stay Real if either side is Real, else Int/Nat.
        let as_f64 = |v: &Value| -> Result<f64, ConstEvalError> {
            match v {
                Value::Real(r) => Ok(*r),
                Value::Int(i) => Ok(*i as f64),
                Value::Nat(n) => Ok(*n as f64),
                _ => Err(ConstEvalError::TypeMismatch),
            }
        };
        match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod | BinOp::Pow => {
                if matches!(l, Value::Real(_)) || matches!(r, Value::Real(_)) {
                    let (a, b) = (as_f64(&l)?, as_f64(&r)?);
                    Ok(Value::Real(match op {
                        BinOp::Add => a + b,
                        BinOp::Sub => a - b,
                        BinOp::Mul => a * b,
                        BinOp::Div => a / b,
                        BinOp::Mod => a % b,
                        BinOp::Pow => a.powf(b),
                        _ => unreachable!(),
                    }))
                } else {
                    let (a, b) = (int_of(&l)?, int_of(&r)?);
                    match op {
                        BinOp::Div if b == 0 => Err(ConstEvalError::DivByZero),
                        BinOp::Mod if b == 0 => Err(ConstEvalError::DivByZero),
                        BinOp::Add => Ok(Value::Int(a + b)),
                        BinOp::Sub => Ok(Value::Int(a - b)),
                        BinOp::Mul => Ok(Value::Int(a * b)),
                        BinOp::Div => Ok(Value::Int(a / b)),
                        BinOp::Mod => Ok(Value::Int(a % b)),
                        BinOp::Pow => Ok(Value::Int(a.pow(b.try_into().unwrap_or(0)))),
                        _ => unreachable!(),
                    }
                }
            }
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::Eq | BinOp::Ne
            | BinOp::CaseEq | BinOp::CaseNe => {
                let (a, b) = (as_f64(&l)?, as_f64(&r)?);
                Ok(Value::Bool(match op {
                    BinOp::Lt => a < b,
                    BinOp::Le => a <= b,
                    BinOp::Gt => a > b,
                    BinOp::Ge => a >= b,
                    BinOp::Eq | BinOp::CaseEq => a == b,
                    BinOp::Ne | BinOp::CaseNe => a != b,
                    _ => unreachable!(),
                }))
            }
            BinOp::LogAnd | BinOp::LogOr => {
                let (a, b) = (truthy(&l)?, truthy(&r)?);
                Ok(Value::Bool(match op {
                    BinOp::LogAnd => a && b,
                    BinOp::LogOr => a || b,
                    _ => unreachable!(),
                }))
            }
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::BitXnor => {
                let (a, b) = (int_of(&l)?, int_of(&r)?);
                Ok(Value::Int(match op {
                    BinOp::BitAnd => a & b,
                    BinOp::BitOr => a | b,
                    BinOp::BitXor => a ^ b,
                    BinOp::BitXnor => !(a ^ b),
                    _ => unreachable!(),
                }))
            }
            BinOp::Shl | BinOp::Shr => {
                let (a, b) = (int_of(&l)?, int_of(&r)?);
                Ok(Value::Int(match op {
                    BinOp::Shl => a << b,
                    BinOp::Shr => a >> b,
                    _ => unreachable!(),
                }))
            }
        }
    }

    /// Evaluates a `name(args)` call (LRM §4.3 math functions/§4.5
    /// time-domain functions/§5.9 user functions — `Expr::Call` doesn't
    /// distinguish which at parse time, see its doc comment) in constant
    /// context.
    ///
    /// **Scope boundary**: only the built-in math functions
    /// (`crate::math::MATH_FNS`, e.g. `pow`/`sin`/`sqrt`/...) are
    /// evaluable here — the same libm wrappers the JIT links against
    /// (`crate::math::eval_const_math`), reused rather than duplicated so
    /// compile-time and runtime results stay bit-identical (MD-13 rule 2/3).
    /// A user-declared `analog function`/`function`, a time-domain operator
    /// (`ddt`/`idt`/...), or any other call name is not a compile-time
    /// constant here and fails loud naming the call, never silently
    /// returning a wrong value — evaluating a user function body would need
    /// its own recursive interpreter (statements, `return`, local
    /// variables), which is out of scope for this elaboration-constant
    /// evaluator (VAMS expressions otherwise have no blocks/lambdas to
    /// walk, per this module's own `//!` header).
    fn eval_call(&self, name: &str, args: &[Expr]) -> Result<Value, ConstEvalError> {
        let arg_vals = args.iter().map(|a| value_to_f64(&self.eval(a)?)).collect::<Result<Vec<_>, _>>()?;
        crate::math::eval_const_math(name, &arg_vals).map(Value::Real).ok_or_else(|| {
            ConstEvalError::NotConst(format!(
                "call to `{name}` is not a compile-time constant (only built-in math functions \
                 are evaluable in constant expressions; user-declared functions are not)"
            ))
        })
    }

    /// Evaluates `expr` and returns the result as a `u64`. Accepts `Nat`
    /// values directly and non-negative `Int` values.
    pub fn eval_nat(&self, expr: &Expr) -> Result<u64, ConstEvalError> {
        match self.eval(expr)? {
            Value::Nat(n) => Ok(n),
            Value::Int(n) if n >= 0 => Ok(n as u64),
            // §4.2.1.1: implicit real->integer conversion rounds to the
            // nearest integer, ties away from zero (`f64::round`'s own
            // documented tie-breaking rule matches the LRM's exactly) --
            // LRMC-C1-11. A round result that's still negative isn't a
            // legal Natural and fails loud below, same as a negative Int.
            Value::Real(r) if r.round() >= 0.0 => Ok(r.round() as u64),
            Value::Logic(lv) => match lv.to_i64_exact() {
                Some(n) if n >= 0 => Ok(n as u64),
                _ => Err(ConstEvalError::FourStateInNumericContext(
                    "a natural-valued context (array bound, bit width, generate bound) requires \
                     a fully-determined non-negative value"
                        .into(),
                )),
            },
            _ => Err(ConstEvalError::TypeMismatch),
        }
    }

    /// Evaluates `expr` and returns the result as an `i64`.
    pub fn eval_int(&self, expr: &Expr) -> Result<i64, ConstEvalError> {
        match self.eval(expr)? {
            Value::Int(n) => Ok(n),
            Value::Nat(n) => Ok(n as i64),
            // §4.2.1.1, LRMC-C1-11: round to nearest, ties away from zero.
            Value::Real(r) => Ok(r.round() as i64),
            Value::Logic(lv) => lv.to_i64_exact().ok_or_else(|| {
                ConstEvalError::FourStateInNumericContext(
                    "an integer-valued context (array bound, bit width, generate bound) requires \
                     a fully-determined value"
                        .into(),
                )
            }),
            _ => Err(ConstEvalError::TypeMismatch),
        }
    }

    /// Evaluates `expr` and returns the result as an `f64` (widens
    /// `Int`/`Nat`) — used by `Real`-typed elaboration constants (`abstol`,
    /// port ranges, ...).
    pub fn eval_f64(&self, expr: &Expr) -> Result<f64, ConstEvalError> {
        match self.eval(expr)? {
            Value::Real(r) => Ok(r),
            Value::Int(n) => Ok(n as f64),
            Value::Nat(n) => Ok(n as f64),
            _ => Err(ConstEvalError::TypeMismatch),
        }
    }

    /// Evaluates an assignment-pattern initializer (`'{...}`) shaped by
    /// `dims` (outermost dimension first, one width per `[msb:lsb]` a
    /// `parameter`/`var` declared, §3.4.4 one dim / §3.4.8 multiple) into a
    /// `Value::List`. `dims == []` means "no array dimension" — falls back
    /// to plain scalar [`Self::eval`] (so a scalar `param` default keeps
    /// using the ordinary path; this method exists only for the array
    /// case, LRMC-C1-06's FUNCTIONALITY half). Each level's element count
    /// must match its `dims` entry exactly — [`ConstEvalError::ArityMismatch`]
    /// on any shape mismatch, never silent truncation/padding.
    pub fn eval_array(&self, expr: &Expr, dims: &[u64]) -> Result<Value, ConstEvalError> {
        let Some((&want, rest)) = dims.split_first() else {
            return self.eval(expr);
        };
        let parts = self.assignment_pattern_parts(expr)?;
        if parts.len() as u64 != want {
            return Err(ConstEvalError::ArityMismatch { expected: want, found: parts.len() });
        }
        let items = parts.iter().map(|p| self.eval_array(p, rest)).collect::<Result<Vec<_>, _>>()?;
        Ok(Value::List(Rc::new(RefCell::new(items))))
    }

    /// Evaluates a *flat* assignment-pattern into its element values, with
    /// no expected-length check — the shape used by an enumerated
    /// `from`/`exclude '{...}` valid-value list (§3.4.6), where there is no
    /// separately-declared dimension to match against (unlike
    /// [`Self::eval_array`]'s array-initializer use).
    pub fn eval_value_list(&self, expr: &Expr) -> Result<Vec<Value>, ConstEvalError> {
        self.assignment_pattern_parts(expr)?.iter().map(|p| self.eval(p)).collect()
    }

    /// Expands an `Expr::AssignmentPattern` into its flat element list,
    /// resolving the replicated form (`` '{count{expr, ...}} ``, §4.2.14)
    /// by evaluating `count` as an elaboration constant and repeating
    /// `parts` that many times. Anything other than an assignment pattern
    /// is a [`ConstEvalError::NotConst`] — an array-shaped initializer must
    /// actually be array syntax, not e.g. a bare scalar expression standing
    /// in for one.
    fn assignment_pattern_parts(&self, expr: &Expr) -> Result<Vec<Expr>, ConstEvalError> {
        match expr {
            Expr::AssignmentPattern { replicate: None, parts, .. } => Ok(parts.clone()),
            Expr::AssignmentPattern { replicate: Some(count), parts, .. } => {
                let n = self.eval_int(count)?;
                if n < 0 {
                    return Err(ConstEvalError::TypeMismatch);
                }
                let mut out = Vec::with_capacity(parts.len() * n as usize);
                for _ in 0..n {
                    out.extend(parts.iter().cloned());
                }
                Ok(out)
            }
            _ => Err(ConstEvalError::NotConst(
                "expected an assignment pattern '{...} for an array initializer (LRM §3.4.4/§3.4.8)".into(),
            )),
        }
    }
}

fn value_to_f64(v: &Value) -> Result<f64, ConstEvalError> {
    match v {
        Value::Real(r) => Ok(*r),
        Value::Int(i) => Ok(*i as f64),
        Value::Nat(n) => Ok(*n as f64),
        _ => Err(ConstEvalError::TypeMismatch),
    }
}

fn int_of(v: &Value) -> Result<i64, ConstEvalError> {
    match v {
        Value::Int(i) => Ok(*i),
        Value::Nat(n) => Ok(*n as i64),
        _ => Err(ConstEvalError::TypeMismatch),
    }
}

fn truthy(v: &Value) -> Result<bool, ConstEvalError> {
    match v {
        Value::Bool(b) => Ok(*b),
        Value::Int(i) => Ok(*i != 0),
        Value::Nat(n) => Ok(*n != 0),
        Value::Real(r) => Ok(*r != 0.0),
        _ => Err(ConstEvalError::TypeMismatch),
    }
}

fn number_lit_to_value(lit: &NumberLit) -> Result<Value, ConstEvalError> {
    match lit {
        NumberLit::Integer(i) => Ok(Value::Int(*i)),
        NumberLit::Real(r) => Ok(Value::Real(*r)),
        NumberLit::Based(b) => {
            let digits: String = b.digits.chars().filter(|c| *c != '_').collect();
            let base = match b.base {
                crate::lexer::NumberBase::Decimal => Base::Decimal,
                crate::lexer::NumberBase::Binary => Base::Binary,
                crate::lexer::NumberBase::Octal => Base::Octal,
                crate::lexer::NumberBase::Hex => Base::Hex,
            };
            let lv = LogicVec::from_based_literal(base, &digits, b.size).ok_or_else(|| {
                let size_desc = b.size.map(|s| s.to_string()).unwrap_or_else(|| "<unsized>".to_string());
                ConstEvalError::IllegalBasedDigit(format!("{size_desc}'{:?} {}", b.base, b.digits))
            })?;
            if lv.has_unknown() {
                // LRMC-C1-02: represent the four-state value — never
                // silently coerce to 0.
                return Ok(Value::Logic(lv));
            }
            // Fully-determined (two-state) literal: preserve the prior
            // Int/Nat representation so every existing two-state consumer
            // is unaffected (`to_i64_exact` always succeeds here since
            // `has_unknown` is false).
            let magnitude = lv.to_i64_exact().expect("two-state LogicVec always converts exactly");
            if b.signed {
                Ok(Value::Int(magnitude))
            } else {
                Ok(Value::Nat(magnitude as u64))
            }
        }
    }
}

impl Default for ConstEnv {
    fn default() -> Self {
        Self::new()
    }
}
