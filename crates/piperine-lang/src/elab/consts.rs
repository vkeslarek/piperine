//! `ConstEval`: a const expression of the subset evaluated to a [`Constant`] at elaboration.
//!
//! A second small evaluator beside the interpreter's, on purpose: this one reads POM
//! expressions, has no duals and no solver. `f64` and `bool` only, IEEE 754 throughout
//! (`1.0 / 0.0` is `inf`, never an error), so nothing the subset can write panics.
//!
//! A `const` item is evaluated once and remembered. A param is read through the caller's
//! [`Bindings`]; a read with no binding, an environment function, a probe or a node is *not
//! constant*, reported to the caller as such so that it can pick the code of its position
//! (`E0321`, or `P0001` for a param of the parent).

use std::collections::BTreeMap;

use piperine_ir::{Constant, Real};

use crate::pom::{BinOp, BlockNode, ConstData, Design, DeclId, DeclKind, ExprId, ExprKind, LitKind, PatKind, Resolution, StmtId, StmtKind, UnOp};
use crate::types::{Callee, Method, Types};

/// The most nested calls of functions an evaluation follows.
const CALL_DEPTH: usize = 256;

/// The bound value of each param declaration an evaluation may read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Bindings {
    values: BTreeMap<DeclId, Constant>,
}

impl Bindings {
    /// No param bound: the bindings of a `const` item.
    pub fn new() -> Self {
        Bindings::default()
    }

    pub fn bind(&mut self, param: DeclId, value: Constant) {
        self.values.insert(param, value);
    }

    pub fn get(&self, param: DeclId) -> Option<Constant> {
        self.values.get(&param).copied()
    }
}

/// Why an expression has no constant value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstFault {
    /// A read of a param with no binding: the caller says whether it is a param of the parent
    /// (`P0001`) or a param no value was given to (`E0321`).
    UnboundParam { expr: ExprId, param: DeclId },
    /// Something only a run can give: a probe, an environment function, a node, a module.
    Dynamic { expr: ExprId, what: String },
    /// A form of the subset the evaluator does not fold.
    Unsupported { expr: ExprId, what: String },
    /// A `const` item with no value (a declaration in a trait): nothing to fold.
    NoValue { decl: DeclId },
}

impl ConstFault {
    /// The expression the fault sits on; none for a `const` item without a value.
    pub fn expr(&self) -> Option<ExprId> {
        match self {
            ConstFault::UnboundParam { expr, .. } | ConstFault::Dynamic { expr, .. } | ConstFault::Unsupported { expr, .. } => Some(*expr),
            ConstFault::NoValue { .. } => None,
        }
    }
}

/// The evaluator: one per elaboration run, so a `const` item is folded once per run.
pub struct ConstEval<'d, 't> {
    design: &'d Design,
    types: &'t Types<'d>,
    consts: BTreeMap<DeclId, Constant>,
    evaluations: usize,
    locals: BTreeMap<DeclId, Constant>,
    depth: usize,
}

impl<'d, 't> ConstEval<'d, 't> {
    pub fn new(design: &'d Design, types: &'t Types<'d>) -> Self {
        ConstEval { design, types, consts: BTreeMap::new(), evaluations: 0, locals: BTreeMap::new(), depth: 0 }
    }

    /// How many `const` items this evaluator has folded, each counted once however many times
    /// it was read.
    pub fn evaluations(&self) -> usize {
        self.evaluations
    }

    /// The value of a `const` item, folded the first time it is asked for.
    pub fn constant(&mut self, decl: DeclId) -> Result<Constant, ConstFault> {
        if let Some(value) = self.consts.get(&decl) {
            return Ok(*value);
        }
        let DeclKind::Const(ConstData { value: Some(expr), .. }) = &self.design.decl_data(decl).kind else {
            return Err(ConstFault::NoValue { decl });
        };
        let expr = *expr;
        let outer = std::mem::take(&mut self.locals);
        let value = self.value(expr, &Bindings::new());
        self.locals = outer;
        let value = value?;
        self.evaluations += 1;
        self.consts.insert(decl, value);
        Ok(value)
    }

    /// The value of `expr` with the params of `bindings` bound.
    pub fn value(&mut self, expr: ExprId, bindings: &Bindings) -> Result<Constant, ConstFault> {
        let kind = self.design.expr_node(expr).kind.clone();
        match &kind {
            ExprKind::Lit(lit) => Self::literal(expr, &lit.kind),
            ExprKind::Paren(inner) => self.value(*inner, bindings),
            ExprKind::Path(_) => self.path(expr, bindings),
            ExprKind::Unary { op, expr: operand } => self.unary(expr, *op, *operand, bindings),
            ExprKind::Binary { op, lhs, rhs } => self.binary(expr, *op, *lhs, *rhs, bindings),
            ExprKind::If { cond, then, else_ } => self.conditional(expr, *cond, then, *else_, bindings),
            ExprKind::Block(block) => self.block(expr, &block.stmts, bindings),
            ExprKind::Call { args, .. } => self.call(expr, &args.iter().map(|a| a.value).collect::<Vec<_>>(), bindings),
            ExprKind::MethodCall { recv, .. } => self.method(expr, *recv, bindings),
            _ => Err(ConstFault::Unsupported { expr, what: "this form in a const expression".to_string() }),
        }
    }

    /// The `f64` of a value, or the fault of an ill-typed read (the type checker rules it out).
    fn real(expr: ExprId, value: Constant) -> Result<f64, ConstFault> {
        match value {
            Constant::Real(r) => Ok(r.get()),
            Constant::Bool(_) => Err(ConstFault::Unsupported { expr, what: "a `bool` where an `f64` is required".to_string() }),
            Constant::Int(_) => Err(ConstFault::Unsupported { expr, what: "an integer where an `f64` is required".to_string() }),
        }
    }

    fn boolean(expr: ExprId, value: Constant) -> Result<bool, ConstFault> {
        match value {
            Constant::Bool(b) => Ok(b),
            Constant::Real(_) => Err(ConstFault::Unsupported { expr, what: "an `f64` where a `bool` is required".to_string() }),
            Constant::Int(_) => Err(ConstFault::Unsupported { expr, what: "an integer where a `bool` is required".to_string() }),
        }
    }

    fn literal(expr: ExprId, kind: &LitKind) -> Result<Constant, ConstFault> {
        match kind {
            LitKind::Float { value, .. } => Ok(Constant::Real(Real::new(*value))),
            LitKind::Bool(b) => Ok(Constant::Bool(*b)),
            LitKind::Int(text) => {
                let digits: String = text.trim_end_matches("f64").chars().filter(|c| *c != '_').collect();
                match digits.parse::<f64>() {
                    Ok(value) if text.ends_with("f64") => Ok(Constant::Real(Real::new(value))),
                    _ => Err(ConstFault::Unsupported { expr, what: "an integer literal".to_string() }),
                }
            }
            LitKind::Str { .. } | LitKind::Unknown => Err(ConstFault::Unsupported { expr, what: "this literal".to_string() }),
        }
    }

    fn path(&mut self, expr: ExprId, bindings: &Bindings) -> Result<Constant, ConstFault> {
        let ExprKind::Path(path) = &self.design.expr_node(expr).kind else {
            return Err(ConstFault::Unsupported { expr, what: "a name".to_string() });
        };
        let res = self.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res);
        let Resolution::Decl(decl) = res else {
            return Err(ConstFault::Dynamic { expr, what: "a built-in name".to_string() });
        };
        match &self.design.decl_data(decl).kind {
            DeclKind::Const(_) => self.constant(decl).map_err(|fault| match fault {
                ConstFault::NoValue { .. } => ConstFault::Unsupported { expr, what: "a `const` without a value".to_string() },
                other => other,
            }),
            DeclKind::Param(_) => bindings.get(decl).ok_or(ConstFault::UnboundParam { expr, param: decl }),
            DeclKind::FnParam(_) | DeclKind::Binding(_) => self.locals.get(&decl).copied().ok_or(ConstFault::Dynamic { expr, what: "a name with no value yet".to_string() }),
            DeclKind::Port(_) | DeclKind::Signal(_) => Err(ConstFault::Dynamic { expr, what: "a node".to_string() }),
            _ => Err(ConstFault::Unsupported { expr, what: "this name in a const expression".to_string() }),
        }
    }

    fn unary(&mut self, expr: ExprId, op: UnOp, operand: ExprId, bindings: &Bindings) -> Result<Constant, ConstFault> {
        let value = self.value(operand, bindings)?;
        match op {
            UnOp::Neg => Ok(Constant::Real(Real::new(-Self::real(operand, value)?))),
            UnOp::Not => Ok(Constant::Bool(!Self::boolean(operand, value)?)),
            UnOp::Deref | UnOp::Ref => Err(ConstFault::Unsupported { expr, what: "a reference or a dereference".to_string() }),
        }
    }

    fn binary(&mut self, expr: ExprId, op: BinOp, lhs: ExprId, rhs: ExprId, bindings: &Bindings) -> Result<Constant, ConstFault> {
        let left = self.value(lhs, bindings)?;
        // `&&` and `||` short-circuit, as in the source: the right side is not read.
        match op {
            BinOp::And if !Self::boolean(lhs, left)? => return Ok(Constant::Bool(false)),
            BinOp::Or if Self::boolean(lhs, left)? => return Ok(Constant::Bool(true)),
            _ => {}
        }
        let right = self.value(rhs, bindings)?;
        let real = |value: f64| Ok(Constant::Real(Real::new(value)));
        match op {
            BinOp::And | BinOp::Or => Ok(Constant::Bool(Self::boolean(rhs, right)?)),
            BinOp::Eq | BinOp::Ne => {
                let equal = match (left, right) {
                    (Constant::Real(a), Constant::Real(b)) => a.get() == b.get(),
                    (Constant::Bool(a), Constant::Bool(b)) => a == b,
                    _ => return Err(ConstFault::Unsupported { expr, what: "a comparison of an `f64` and a `bool`".to_string() }),
                };
                Ok(Constant::Bool(equal == (op == BinOp::Eq)))
            }
            _ => {
                let (a, b) = (Self::real(lhs, left)?, Self::real(rhs, right)?);
                match op {
                    BinOp::Add => real(a + b),
                    BinOp::Sub => real(a - b),
                    BinOp::Mul => real(a * b),
                    BinOp::Div => real(a / b),
                    BinOp::Lt => Ok(Constant::Bool(a < b)),
                    BinOp::Le => Ok(Constant::Bool(a <= b)),
                    BinOp::Gt => Ok(Constant::Bool(a > b)),
                    BinOp::Ge => Ok(Constant::Bool(a >= b)),
                    _ => Err(ConstFault::Unsupported { expr, what: "this operator".to_string() }),
                }
            }
        }
    }

    fn conditional(&mut self, expr: ExprId, cond: ExprId, then: &BlockNode, else_: Option<ExprId>, bindings: &Bindings) -> Result<Constant, ConstFault> {
        let taken = Self::boolean(cond, self.value(cond, bindings)?)?;
        match (taken, else_) {
            (true, _) => self.block(expr, &then.stmts, bindings),
            (false, Some(else_)) => self.value(else_, bindings),
            (false, None) => Err(ConstFault::Unsupported { expr, what: "an `if` without an `else` as a value".to_string() }),
        }
    }

    /// The value of a block: its last statement, an expression without `;`.
    fn block(&mut self, expr: ExprId, stmts: &[StmtId], bindings: &Bindings) -> Result<Constant, ConstFault> {
        let mut value = None;
        for stmt in stmts {
            value = None;
            match self.design.stmt_node(*stmt).kind.clone() {
                StmtKind::Let { pat, init: Some(init), else_: None, .. } => {
                    let bound = self.value(init, bindings)?;
                    match &self.design.pat_node(pat).kind {
                        PatKind::Ident { binding, sub: None, .. } => {
                            self.locals.insert(*binding, bound);
                        }
                        _ => return Err(ConstFault::Unsupported { expr, what: "a `let` pattern that is not a name".to_string() }),
                    }
                }
                StmtKind::Expr { expr: inner, semi } => {
                    let inner_value = self.value(inner, bindings)?;
                    if !semi {
                        value = Some(inner_value);
                    }
                }
                StmtKind::Empty => {}
                _ => return Err(ConstFault::Unsupported { expr, what: "this statement in a const expression".to_string() }),
            }
        }
        value.ok_or(ConstFault::Unsupported { expr, what: "a block without a value".to_string() })
    }

    /// A call of a function of the reachable set: its body evaluated over its arguments. Every
    /// other callee needs a run.
    fn call(&mut self, expr: ExprId, args: &[ExprId], bindings: &Bindings) -> Result<Constant, ConstFault> {
        let Some(Callee::Function(function)) = self.types.callee(expr) else {
            return Err(ConstFault::Dynamic { expr, what: "a call that is not a function".to_string() });
        };
        let mut values = Vec::new();
        for arg in args {
            values.push(self.value(*arg, bindings)?);
        }
        let data = self.design.decl_data(function);
        let DeclKind::Fn(f) = &data.kind else {
            return Err(ConstFault::Unsupported { expr, what: "a call of a name that is not a function".to_string() });
        };
        let Some(body) = &f.body else {
            return Err(ConstFault::Unsupported { expr, what: "a function without a body".to_string() });
        };
        if self.depth >= CALL_DEPTH {
            return Err(ConstFault::Unsupported { expr, what: format!("calls nested deeper than {CALL_DEPTH}") });
        }
        let params = data.children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::FnParam(_)));
        let mut frame = BTreeMap::new();
        for (param, value) in params.zip(values) {
            frame.insert(param, value);
        }
        let outer = std::mem::replace(&mut self.locals, frame);
        self.depth += 1;
        // A function reads no param of the caller: its body is evaluated with none bound.
        let result = self.block(expr, &body.stmts, &Bindings::new());
        self.depth -= 1;
        self.locals = outer;
        result
    }

    /// A row of the Appendix H catalogue applied to its receiver.
    fn method(&mut self, expr: ExprId, recv: ExprId, bindings: &Bindings) -> Result<Constant, ConstFault> {
        let Some(method) = self.types.method(expr) else {
            return Err(ConstFault::Unsupported { expr, what: "a method outside the catalogue".to_string() });
        };
        let x = Self::real(recv, self.value(recv, bindings)?)?;
        let result = match method {
            Method::Exp => x.exp(),
            Method::Ln => x.ln(),
            Method::Sqrt => x.sqrt(),
            Method::Abs => x.abs(),
            // Folded from msl-digital T23 on.
            Method::Min | Method::Max | Method::Clamp | Method::Floor => {
                return Err(ConstFault::Unsupported { expr, what: "the method `min`, `max`, `clamp` or `floor`".to_string() });
            }
        };
        Ok(Constant::Real(Real::new(result)))
    }
}
