use crate::parse::ast::{BinaryOp, Block, Expr, Literal, Stmt, UnaryOp};
use std::collections::HashMap;
use thiserror::Error;

/// A compile-time constant value. Used to evaluate const expressions,
/// array sizes, for-loop bounds, and structural `if` conditions during
/// elaboration.
#[derive(Debug, Clone, PartialEq)]
pub enum ConstVal {
    Int(i64),
    Nat(u64),
    Real(f64),
    Bool(bool),
    Str(String),
    EnumVariant(String, String),
}

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

/// A lexical-scope environment for compile-time constant evaluation.
/// Maintains a stack of scopes; `push`/`pop` manage nesting (e.g. for
/// loop bodies), `define` adds a binding to the innermost scope, and
/// `lookup` searches scopes from innermost outward.
pub struct ConstEnv {
    bindings: Vec<HashMap<String, ConstVal>>,
}

impl ConstEnv {
    /// Creates a new `ConstEnv` with a single empty scope.
    pub fn new() -> Self {
        Self { bindings: vec![HashMap::new()] }
    }

    /// Creates a new `ConstEnv` pre-populated with global constants.
    pub fn with_globals(globals: HashMap<String, ConstVal>) -> Self {
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
    pub fn define(&mut self, name: String, val: ConstVal) {
        self.bindings.last_mut().unwrap().insert(name, val);
    }

    /// Looks up `name` in the scope stack, from innermost outward.
    /// Returns `None` if the name is unbound.
    pub fn lookup(&self, name: &str) -> Option<&ConstVal> {
        self.bindings.iter().rev().find_map(|scope| scope.get(name))
    }

    /// Evaluates a compile-time expression to a `ConstVal`. Supports
    /// literals, identifiers (looked up in the scope stack), unary
    /// (`-`, `!`), binary (arithmetic/comparison/bitwise on `Nat`/`Int`/
    /// `Real`/`Bool`), const-if, and block expressions.
    pub fn eval(&self, expr: &Expr) -> Result<ConstVal, ConstEvalError> {
        match expr {
            Expr::Literal(lit) => Ok(match lit {
                Literal::Int(n) => ConstVal::Nat(*n),
                Literal::Real(r) => ConstVal::Real(*r),
                Literal::Bool(b) => ConstVal::Bool(*b),
                Literal::String(s) => ConstVal::Str(s.clone()),
                Literal::Quad(q) => {
                    return Err(ConstEvalError::NotConst(format!("quad literal 0q{}", q)));
                }
            }),

            Expr::Ident(name) => self
                .lookup(name)
                .cloned()
                .ok_or_else(|| ConstEvalError::Undefined(name.clone())),

            Expr::Path(path) => {
                if path.segments.len() == 2 {
                    Ok(ConstVal::EnumVariant(path.segments[0].clone(), path.segments[1].clone()))
                } else {
                    Err(ConstEvalError::NotConst(format!("{:?}", path)))
                }
            }

            Expr::Unary(op, inner) => {
                let val = self.eval(inner)?;
                match (op, val) {
                    (UnaryOp::Neg, ConstVal::Nat(n)) => Ok(ConstVal::Int(-(n as i64))),
                    (UnaryOp::Neg, ConstVal::Int(n)) => Ok(ConstVal::Int(-n)),
                    (UnaryOp::Neg, ConstVal::Real(r)) => Ok(ConstVal::Real(-r)),
                    (UnaryOp::Not, ConstVal::Bool(b)) => Ok(ConstVal::Bool(!b)),
                    (UnaryOp::Not, ConstVal::Nat(n)) => Ok(ConstVal::Nat(!n)),
                    _ => Err(ConstEvalError::TypeMismatch),
                }
            }

            Expr::Binary(lhs, op, rhs) => {
                let l = self.eval(lhs)?;
                let r = self.eval(rhs)?;
                self.eval_binary(op, l, r)
            }

            Expr::If { cond, then_body, else_body } => {
                let cond_val = self.eval(cond)?;
                let taken = match cond_val {
                    ConstVal::Bool(true) | ConstVal::Nat(1) => then_body,
                    ConstVal::Bool(false) | ConstVal::Nat(0) => else_body,
                    ConstVal::Nat(n) if n != 0 => then_body,
                    _ => else_body,
                };
                self.eval_block(taken)
            }

            Expr::Block(block) => self.eval_block(block),

            other => Err(ConstEvalError::NotConst(format!("{:?}", other))),
        }
    }

    /// Evaluates a block to a const value. For a block with no statements,
    /// the trailing expression is evaluated. `Return` statements are also
    /// handled.
    fn eval_block(&self, block: &Block) -> Result<ConstVal, ConstEvalError> {
        // Only evaluate blocks that consist solely of a trailing expression.
        // Var decls in const blocks are not supported in V1.
        if !block.stmts.is_empty() {
            // Allow simple return stmts
            for stmt in &block.stmts {
                match stmt {
                    Stmt::Return(e) => return self.eval(e),
                    _ => {}
                }
            }
        }
        match &block.expr {
            Some(e) => self.eval(e),
            None => Err(ConstEvalError::NotConst("block with no trailing expr".to_owned())),
        }
    }

    /// Evaluates a binary operation on two `ConstVal`s. Handles arithmetic
    /// (`+`, `-`, `*`, `/`, `%`) on `Nat`/`Int`/`Real`, comparisons on
    /// `Nat`/`Int`/`Bool`, and bitwise operations on `Nat`/`Bool`. Returns
    /// `TypeMismatch` for unsupported operand type combinations.
    fn eval_binary(
        &self,
        op: &BinaryOp,
        l: ConstVal,
        r: ConstVal,
    ) -> Result<ConstVal, ConstEvalError> {
        use BinaryOp::*;
        use ConstVal::*;

        match (op, l, r) {
            // Nat arithmetic
            (Add, Nat(a), Nat(b)) => Ok(Nat(a.wrapping_add(b))),
            (Sub, Nat(a), Nat(b)) => Ok(Nat(a.wrapping_sub(b))),
            (Mul, Nat(a), Nat(b)) => Ok(Nat(a.wrapping_mul(b))),
            (Div, Nat(_), Nat(0)) => Err(ConstEvalError::DivByZero),
            (Div, Nat(a), Nat(b)) => Ok(Nat(a / b)),
            (Rem, Nat(_), Nat(0)) => Err(ConstEvalError::DivByZero),
            (Rem, Nat(a), Nat(b)) => Ok(Nat(a % b)),

            // Int arithmetic
            (Add, Int(a), Int(b)) => Ok(Int(a.wrapping_add(b))),
            (Sub, Int(a), Int(b)) => Ok(Int(a.wrapping_sub(b))),
            (Mul, Int(a), Int(b)) => Ok(Int(a.wrapping_mul(b))),
            (Div, Int(_), Int(0)) => Err(ConstEvalError::DivByZero),
            (Div, Int(a), Int(b)) => Ok(Int(a / b)),
            (Rem, Int(_), Int(0)) => Err(ConstEvalError::DivByZero),
            (Rem, Int(a), Int(b)) => Ok(Int(a % b)),

            // Mixed Nat/Int
            (Add, Nat(a), Int(b)) => Ok(Int(a as i64 + b)),
            (Add, Int(a), Nat(b)) => Ok(Int(a + b as i64)),
            (Sub, Nat(a), Int(b)) => Ok(Int(a as i64 - b)),
            (Sub, Int(a), Nat(b)) => Ok(Int(a - b as i64)),
            (Mul, Nat(a), Int(b)) => Ok(Int(a as i64 * b)),
            (Mul, Int(a), Nat(b)) => Ok(Int(a * b as i64)),

            // Real arithmetic
            (Add, Real(a), Real(b)) => Ok(Real(a + b)),
            (Sub, Real(a), Real(b)) => Ok(Real(a - b)),
            (Mul, Real(a), Real(b)) => Ok(Real(a * b)),
            (Div, Real(a), Real(b)) => Ok(Real(a / b)),

            // Comparisons — Nat
            (Eq, Nat(a), Nat(b)) => Ok(Bool(a == b)),
            (Neq, Nat(a), Nat(b)) => Ok(Bool(a != b)),
            (Lt, Nat(a), Nat(b)) => Ok(Bool(a < b)),
            (Le, Nat(a), Nat(b)) => Ok(Bool(a <= b)),
            (Gt, Nat(a), Nat(b)) => Ok(Bool(a > b)),
            (Ge, Nat(a), Nat(b)) => Ok(Bool(a >= b)),

            // Comparisons — Int
            (Eq, Int(a), Int(b)) => Ok(Bool(a == b)),
            (Neq, Int(a), Int(b)) => Ok(Bool(a != b)),
            (Lt, Int(a), Int(b)) => Ok(Bool(a < b)),
            (Le, Int(a), Int(b)) => Ok(Bool(a <= b)),
            (Gt, Int(a), Int(b)) => Ok(Bool(a > b)),
            (Ge, Int(a), Int(b)) => Ok(Bool(a >= b)),

            // Comparisons — Bool
            (Eq, Bool(a), Bool(b)) => Ok(Bool(a == b)),
            (Neq, Bool(a), Bool(b)) => Ok(Bool(a != b)),

            // Comparisons — EnumVariant
            (Eq, EnumVariant(e1, v1), EnumVariant(e2, v2)) => Ok(Bool(e1 == e2 && v1 == v2)),
            (Neq, EnumVariant(e1, v1), EnumVariant(e2, v2)) => Ok(Bool(e1 != e2 || v1 != v2)),

            // Bitwise
            (BitAnd, Nat(a), Nat(b)) => Ok(Nat(a & b)),
            (BitOr, Nat(a), Nat(b)) => Ok(Nat(a | b)),
            (BitXor, Nat(a), Nat(b)) => Ok(Nat(a ^ b)),
            (BitAnd, Bool(a), Bool(b)) => Ok(Bool(a & b)),
            (BitOr, Bool(a), Bool(b)) => Ok(Bool(a | b)),
            (BitXor, Bool(a), Bool(b)) => Ok(Bool(a ^ b)),

            // Logical
            (And, Bool(a), Bool(b)) => Ok(Bool(a && b)),
            (Or, Bool(a), Bool(b)) => Ok(Bool(a || b)),

            _ => Err(ConstEvalError::TypeMismatch),
        }
    }

    /// Evaluates `expr` and returns the result as a `u64`. Accepts `Nat`
    /// values directly and non-negative `Int` values.
    pub fn eval_nat(&self, expr: &Expr) -> Result<u64, ConstEvalError> {
        match self.eval(expr)? {
            ConstVal::Nat(n) => Ok(n),
            ConstVal::Int(n) if n >= 0 => Ok(n as u64),
            _ => Err(ConstEvalError::TypeMismatch),
        }
    }

    /// Evaluates `expr` and returns the result as an `i64`. Accepts `Int`
    /// values directly and `Nat` values widened to `i64`.
    pub fn eval_int(&self, expr: &Expr) -> Result<i64, ConstEvalError> {
        match self.eval(expr)? {
            ConstVal::Int(n) => Ok(n),
            ConstVal::Nat(n) => Ok(n as i64),
            _ => Err(ConstEvalError::TypeMismatch),
        }
    }
}

impl Default for ConstEnv {
    fn default() -> Self {
        Self::new()
    }
}
