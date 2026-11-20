//! `ConstEval`: a const expression of the subset evaluated to a [`Constant`] at elaboration.
//!
//! A second small evaluator beside the interpreter's, on purpose: this one reads POM
//! expressions, has no duals and no solver. `f64` is IEEE 754 throughout (`1.0 / 0.0` is `inf`,
//! never an error). Integers follow the rules the compiled code follows (`piperine-transpile`
//! `rt::int`): arithmetic wraps modulo `2^N` (§1 rule 6), `as` from `f64` saturates and maps NaN
//! to 0 (ROADMAP G4), a shift by `N` or more gives 0, or the sign for a signed `>>` (G5). An
//! integer division or remainder by zero, a shift by a negative amount and a `clamp` with its
//! bounds crossed are panics (§1 rule 11): `E0104` at elaboration.
//!
//! A value may be unknown (§3.3): [`Folded`] is a known [`Constant`] or `Unknown`, and the
//! operators fold it by the rules of the compiled code (`rt::Flagged`, §3.3.2): all-or-nothing
//! for arithmetic, comparison and `^`, per level for `&`, `|`, `&&` and `||` (a known all-false
//! operand decides `&`, a known all-true operand decides `|`), a shift of an unknown value by `N`
//! or more is known 0. An `if` on an unknown condition is the merge of its arms (§3.3.3), a panic
//! on one of them making it unknown (§1 rule 13). [`ConstEval::value`] gives a [`Constant`], so a
//! result that is unknown is [`ConstFault::Unknown`]; [`ConstEval::folded`] gives the result as
//! it is, for a caller that accepts an unknown.
//!
//! A `const` item is evaluated once and remembered. A param is read through the caller's
//! [`Bindings`]; a read with no binding, an environment function, a probe or a node is *not
//! constant*, reported to the caller as such so that it can pick the code of its position
//! (`E0321`, or `P0001` for a param of the parent).

use std::collections::BTreeMap;

use piperine_ir::{Constant, Real};

use super::compound::EnumLayout;
use crate::msl::lexer;
use crate::pom::{BinOp, BlockNode, ConstData, Design, DeclId, DeclKind, ExprId, ExprKind, LitKind, PatKind, Resolution, StmtId, StmtKind, UnOp};
use crate::types::{Builtin, Callee, Intrinsic, Method, Ty, Types};

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

    /// Whether no param is bound.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn get(&self, param: DeclId) -> Option<Constant> {
        self.values.get(&param).copied()
    }

    /// These bindings with those of `other` added: `other` wins where both bind a name.
    pub(super) fn overlaid(&self, other: &Bindings) -> Bindings {
        let mut merged = self.clone();
        merged.values.extend(other.values.iter().map(|(decl, value)| (*decl, *value)));
        merged
    }

    /// Forgets the value of `decl`; the value it had, if any.
    pub(super) fn unbind(&mut self, decl: DeclId) -> Option<Constant> {
        self.values.remove(&decl)
    }
}

/// What an expression folds to: a known value, or `Unknown` (§3.3). One flag per value, as the
/// compiled code has it (G6, provisional): the per-level spec (`unknown-semantics-per-level`)
/// adds a partial form here.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Folded {
    /// A known value.
    Known(Constant),
    /// Every level unknown.
    Unknown,
}

impl Folded {
    /// Whether `self` and `other` are the same value, a NaN being the same as itself (§3.3.9).
    fn same(self, other: Folded) -> bool {
        match (self, other) {
            (Folded::Unknown, Folded::Unknown) => true,
            (Folded::Known(Constant::Real(a)), Folded::Known(Constant::Real(b))) => a.get().to_bits() == b.get().to_bits(),
            (Folded::Known(a), Folded::Known(b)) => a == b,
            _ => false,
        }
    }

    /// The merge of two paths (§3.3.3, §22.1.2): equal known values are kept, anything else is
    /// unknown.
    fn merge(self, other: Folded) -> Folded {
        if matches!(self, Folded::Known(_)) && self.same(other) { self } else { Folded::Unknown }
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
    /// A panic of the evaluation (§1 rule 11): an integer division or remainder by zero, a shift
    /// by a negative amount, a `clamp` whose bounds are crossed. `E0104` at elaboration.
    Panic { expr: ExprId, what: String },
    /// The value is unknown (§3.3) where a known one is required: the caller says what it
    /// decides (`E0151` when it fixes structure, §3.3.8).
    Unknown { expr: ExprId },
}

impl ConstFault {
    /// The expression the fault sits on; none for a `const` item without a value.
    pub fn expr(&self) -> Option<ExprId> {
        match self {
            ConstFault::UnboundParam { expr, .. } | ConstFault::Dynamic { expr, .. } | ConstFault::Unsupported { expr, .. } | ConstFault::Panic { expr, .. } | ConstFault::Unknown { expr } => Some(*expr),
            ConstFault::NoValue { .. } => None,
        }
    }
}

/// The evaluator: one per elaboration run, so a `const` item is folded once per run.
pub struct ConstEval<'d, 't> {
    design: &'d Design,
    types: &'t Types<'d>,
    consts: BTreeMap<DeclId, Folded>,
    evaluations: usize,
    locals: BTreeMap<DeclId, Folded>,
    depth: usize,
    /// The module-body `let`s being evaluated, innermost last: a read of one of them is a cycle.
    open_lets: Vec<DeclId>,
}

impl<'d, 't> ConstEval<'d, 't> {
    pub fn new(design: &'d Design, types: &'t Types<'d>) -> Self {
        ConstEval { design, types, consts: BTreeMap::new(), evaluations: 0, locals: BTreeMap::new(), depth: 0, open_lets: Vec::new() }
    }

    /// How many `const` items this evaluator has folded, each counted once however many times
    /// it was read.
    pub fn evaluations(&self) -> usize {
        self.evaluations
    }

    /// The value of a `const` item, folded the first time it is asked for. A `const` that is
    /// unknown is [`ConstFault::Unknown`] on its initializer.
    pub fn constant(&mut self, decl: DeclId) -> Result<Constant, ConstFault> {
        match self.folded_constant(decl)? {
            Folded::Known(value) => Ok(value),
            Folded::Unknown => match &self.design.decl_data(decl).kind {
                DeclKind::Const(ConstData { value: Some(expr), .. }) => Err(ConstFault::Unknown { expr: *expr }),
                _ => Err(ConstFault::NoValue { decl }),
            },
        }
    }

    /// What a `const` item folds to, `Unknown` included, folded the first time it is asked for.
    pub fn folded_constant(&mut self, decl: DeclId) -> Result<Folded, ConstFault> {
        if let Some(value) = self.consts.get(&decl) {
            return Ok(*value);
        }
        let DeclKind::Const(ConstData { value: Some(expr), .. }) = &self.design.decl_data(decl).kind else {
            return Err(ConstFault::NoValue { decl });
        };
        let expr = *expr;
        let outer = std::mem::take(&mut self.locals);
        let value = self.folded(expr, &Bindings::new());
        self.locals = outer;
        let value = value?;
        self.evaluations += 1;
        self.consts.insert(decl, value);
        Ok(value)
    }

    /// The value of `expr` with the params of `bindings` bound; an unknown result is
    /// [`ConstFault::Unknown`].
    pub fn value(&mut self, expr: ExprId, bindings: &Bindings) -> Result<Constant, ConstFault> {
        match self.folded(expr, bindings)? {
            Folded::Known(value) => Ok(value),
            Folded::Unknown => Err(ConstFault::Unknown { expr }),
        }
    }

    /// What `expr` folds to with the params of `bindings` bound, `Unknown` included.
    pub fn folded(&mut self, expr: ExprId, bindings: &Bindings) -> Result<Folded, ConstFault> {
        let kind = self.design.expr_node(expr).kind.clone();
        match &kind {
            ExprKind::Lit(lit) => self.literal(expr, &lit.kind),
            ExprKind::Paren(inner) => self.folded(*inner, bindings),
            ExprKind::Path(_) => self.path(expr, bindings),
            ExprKind::Unary { op, expr: operand } => self.unary(expr, *op, *operand, bindings),
            ExprKind::Binary { op, lhs, rhs } => self.binary(expr, *op, *lhs, *rhs, bindings),
            ExprKind::If { cond, then, else_ } => self.conditional(expr, *cond, then, *else_, bindings),
            ExprKind::Block(block) => self.block(expr, &block.stmts, bindings),
            ExprKind::Call { args, .. } => self.call(expr, &args.iter().map(|a| a.value).collect::<Vec<_>>(), bindings),
            ExprKind::MethodCall { recv, args, .. } => self.method(expr, *recv, &args.iter().map(|a| a.value).collect::<Vec<_>>(), bindings),
            ExprKind::Cast { expr: operand, .. } => self.cast(expr, *operand, bindings),
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

    /// The integer type the checker gave `expr`, as the width and sign the evaluator wraps to.
    fn width(&self, expr: ExprId) -> Option<Width> {
        self.types.ty(expr).and_then(Width::of)
    }

    /// A literal as the constant it is: a number of its type's width, a bool, a real. `Unknown`
    /// folds to `Folded::Unknown`.
    fn literal(&self, expr: ExprId, kind: &LitKind) -> Result<Folded, ConstFault> {
        let known = |value: Constant| Ok(Folded::Known(value));
        match kind {
            LitKind::Float { value, .. } => known(Constant::Real(Real::new(*value))),
            LitKind::Bool(b) => known(Constant::Bool(*b)),
            LitKind::Int(text) if text.ends_with("f64") => {
                let digits: String = text.trim_end_matches("f64").chars().filter(|c| *c != '_').collect();
                match digits.parse::<f64>() {
                    Ok(value) => known(Constant::Real(Real::new(value))),
                    Err(_) => Err(ConstFault::Unsupported { expr, what: "an integer literal".to_string() }),
                }
            }
            LitKind::Int(text) => match (self.width(expr), lexer::int_value(text)) {
                // An `i8` literal `128` is the operand of a unary `-` (the checker allowed it): it
                // wraps to -128 here and the minus wraps it back.
                (Some(width), Some(value)) => known(width.wrap(value as i128)),
                _ => Err(ConstFault::Unsupported { expr, what: "an integer literal".to_string() }),
            },
            LitKind::Unknown => Ok(Folded::Unknown),
            LitKind::Str { .. } => Err(ConstFault::Unsupported { expr, what: "this literal".to_string() }),
        }
    }

    /// A name as the constant it is bound to (a `const`, a param, a binding of the call), or the
    /// level count of a `T::WIDTH`; a built-in name is dynamic.
    fn path(&mut self, expr: ExprId, bindings: &Bindings) -> Result<Folded, ConstFault> {
        // `T::WIDTH`: the level count of `T`, which the checker recorded (§2, §22.1)
        if let Some(levels) = self.types.width(expr) {
            return Ok(Folded::Known(Constant::Int(i128::from(levels))));
        }
        let ExprKind::Path(path) = &self.design.expr_node(expr).kind else {
            return Err(ConstFault::Unsupported { expr, what: "a name".to_string() });
        };
        let res = self.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res);
        let Resolution::Decl(decl) = res else {
            return Err(ConstFault::Dynamic { expr, what: "a built-in name".to_string() });
        };
        match &self.design.decl_data(decl).kind {
            DeclKind::Const(_) => self.folded_constant(decl).map_err(|fault| match fault {
                ConstFault::NoValue { .. } => ConstFault::Unsupported { expr, what: "a `const` without a value".to_string() },
                other => other,
            }),
            DeclKind::Param(_) => bindings.get(decl).map(Folded::Known).ok_or(ConstFault::UnboundParam { expr, param: decl }),
            // A unit variant is its tag (§2 rule 5).
            DeclKind::Variant(_) => EnumLayout::variant_tag(self.design, decl).map(|tag| Folded::Known(Constant::Int(tag))).ok_or(ConstFault::Unsupported { expr, what: "a variant outside its enum".to_string() }),
            DeclKind::FnParam(_) | DeclKind::Binding(_) => match (self.locals.get(&decl).copied(), bindings.get(decl)) {
                (Some(value), _) => Ok(value),
                // the loop variable of an unrolled analog loop, or the parameter of an unrolled
                // recursion, bound to its constant for this iteration or call (`unroll.rs`)
                (None, Some(value)) => Ok(Folded::Known(value)),
                (None, None) => self.body_let(expr, decl, bindings),
            },
            // A value input the caller binds to a constant (`structural.rs`, a selector through an input).
            DeclKind::Port(_) if let Some(value) = bindings.get(decl) => Ok(Folded::Known(value)),
            DeclKind::Port(_) | DeclKind::Signal(_) if self.types.carrier(decl).is_some() => Err(ConstFault::Dynamic { expr, what: "a carrier".to_string() }),
            DeclKind::Port(_) | DeclKind::Signal(_) => Err(ConstFault::Dynamic { expr, what: "a node".to_string() }),
            _ => Err(ConstFault::Unsupported { expr, what: "this name in a const expression".to_string() }),
        }
    }

    /// The value of the module-body `let` name `decl` read at `expr`: its value expression evaluated
    /// over the same params (§8.1.19), so a param value may name a `let` that computes it. A `let`
    /// that reads a carrier, a probe, the environment or the result of an instance has no constant
    /// value: the fault names the `let`. A `let` that reads itself is one too.
    fn body_let(&mut self, expr: ExprId, decl: DeclId, bindings: &Bindings) -> Result<Folded, ConstFault> {
        let Some(value) = Self::body_let_value(self.design, decl) else {
            return Err(ConstFault::Dynamic { expr, what: "a name with no value yet".to_string() });
        };
        let name = self.design.decl_data(decl).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        if self.open_lets.contains(&decl) {
            return Err(ConstFault::Dynamic { expr, what: format!("the `let` `{name}`, which reads itself") });
        }
        self.open_lets.push(decl);
        let outer = std::mem::take(&mut self.locals);
        let result = self.folded(value, bindings);
        self.locals = outer;
        self.open_lets.pop();
        result.map_err(|fault| match fault {
            ConstFault::Dynamic { what, .. } => ConstFault::Dynamic { expr, what: format!("the `let` `{name}`, which reads {what}") },
            other => other,
        })
    }

    /// The value a module-body `let` binding stands for: the whole value of an identifier pattern,
    /// the matching component of a tuple pattern over a tuple expression.
    pub(super) fn body_let_value(design: &Design, binding: DeclId) -> Option<ExprId> {
        let data = design.decl_data(binding);
        let DeclKind::Binding(b) = &data.kind else { return None };
        if b.origin != crate::pom::BindingOrigin::BodyLet {
            return None;
        }
        let DeclKind::Let(l) = &design.decl_data(data.parent?).kind else { return None };
        match &design.pat_node(l.pat).kind {
            PatKind::Ident { binding: bound, sub: None, .. } if *bound == binding => Some(l.value),
            PatKind::Tuple(items) => {
                let at = items.iter().position(|p| matches!(&design.pat_node(*p).kind, PatKind::Ident { binding: bound, sub: None, .. } if *bound == binding))?;
                let mut whole = l.value;
                while let ExprKind::Paren(inner) = &design.expr_node(whole).kind {
                    whole = *inner;
                }
                match &design.expr_node(whole).kind {
                    ExprKind::Tuple(values) if values.len() == items.len() => Some(values[at]),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// A unary operator over `operand`: unknown when the operand is (§3.3.2), else wrapped to the
    /// width of an integer result.
    fn unary(&mut self, expr: ExprId, op: UnOp, operand: ExprId, bindings: &Bindings) -> Result<Folded, ConstFault> {
        // §3.3.2: an operator of an unknown value is unknown
        let Folded::Known(value) = self.folded(operand, bindings)? else {
            return Ok(Folded::Unknown);
        };
        if let (Constant::Int(v), Some(width)) = (value, self.width(expr)) {
            return match op {
                UnOp::Neg => Ok(Folded::Known(width.wrap(v.wrapping_neg()))),
                UnOp::Not => Ok(Folded::Known(width.wrap(!v))),
                UnOp::Deref | UnOp::Ref => Err(ConstFault::Unsupported { expr, what: "a reference or a dereference".to_string() }),
            };
        }
        match op {
            UnOp::Neg => Ok(Folded::Known(Constant::Real(Real::new(-Self::real(operand, value)?)))),
            UnOp::Not => Ok(Folded::Known(Constant::Bool(!Self::boolean(operand, value)?))),
            UnOp::Deref | UnOp::Ref => Err(ConstFault::Unsupported { expr, what: "a reference or a dereference".to_string() }),
        }
    }

    /// A binary operator over `lhs` and `rhs`: `&&` and `||` stop at a known left operand that
    /// decides; an unknown operand makes the result unknown unless the operator decides (§3.3.2).
    fn binary(&mut self, expr: ExprId, op: BinOp, lhs: ExprId, rhs: ExprId, bindings: &Bindings) -> Result<Folded, ConstFault> {
        let left = self.folded(lhs, bindings)?;
        // `&&` and `||` short-circuit on a known left operand, as in the source: the right side is
        // not read.
        if let Folded::Known(known) = left {
            match op {
                BinOp::And if !Self::boolean(lhs, known)? => return Ok(Folded::Known(Constant::Bool(false))),
                BinOp::Or if Self::boolean(lhs, known)? => return Ok(Folded::Known(Constant::Bool(true))),
                _ => {}
            }
        }
        let right = self.folded(rhs, bindings)?;
        match (left, right) {
            (Folded::Known(a), Folded::Known(b)) => self.known_binary(expr, op, (lhs, a), (rhs, b)).map(Folded::Known),
            _ => Ok(self.unknown_binary(expr, op, left, right)),
        }
    }

    /// An operator with an unknown operand (§3.3.2): the result is unknown, and the operation is
    /// not evaluated on it (an unknown divisor is no division by zero), except where a known
    /// operand decides it level by level. A known all-false operand decides `&` and `&&`, a
    /// known all-true operand decides `|` and `||` (§3.3.12), and an unknown value shifted by a
    /// known `N` or more is known 0 (§3.3.11), a signed `>>` excepted: it fills with the unknown
    /// sign.
    fn unknown_binary(&self, expr: ExprId, op: BinOp, left: Folded, right: Folded) -> Folded {
        let width = self.width(expr);
        let decided = |known: Folded| match (op, known) {
            (BinOp::And | BinOp::BitAnd, Folded::Known(Constant::Bool(false))) => Some(Constant::Bool(false)),
            (BinOp::Or | BinOp::BitOr, Folded::Known(Constant::Bool(true))) => Some(Constant::Bool(true)),
            (BinOp::BitAnd, Folded::Known(Constant::Int(0))) => Some(Constant::Int(0)),
            (BinOp::BitOr, Folded::Known(Constant::Int(ones))) if width.is_some_and(|w| w.wrap(-1) == Constant::Int(ones)) => Some(Constant::Int(ones)),
            _ => None,
        };
        if let Some(value) = decided(left).or_else(|| decided(right)) {
            return Folded::Known(value);
        }
        match (op, left, right, width) {
            (BinOp::Shl, Folded::Unknown, Folded::Known(Constant::Int(n)), Some(w)) if n >= i128::from(w.bits) => Folded::Known(Constant::Int(0)),
            (BinOp::Shr, Folded::Unknown, Folded::Known(Constant::Int(n)), Some(w)) if !w.signed && n >= i128::from(w.bits) => Folded::Known(Constant::Int(0)),
            _ => Folded::Unknown,
        }
    }

    /// An operator over two known operands, `(expression, value)` each.
    fn known_binary(&self, expr: ExprId, op: BinOp, (lhs, left): (ExprId, Constant), (rhs, right): (ExprId, Constant)) -> Result<Constant, ConstFault> {
        if let (Constant::Int(a), Constant::Int(b)) = (left, right) {
            return self.integer_binary(expr, op, a, b);
        }
        let real = |value: f64| Ok(Constant::Real(Real::new(value)));
        match op {
            BinOp::And | BinOp::Or => Ok(Constant::Bool(Self::boolean(rhs, right)?)),
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => {
                let (a, b) = (Self::boolean(lhs, left)?, Self::boolean(rhs, right)?);
                Ok(Constant::Bool(match op {
                    BinOp::BitAnd => a & b,
                    BinOp::BitOr => a | b,
                    _ => a ^ b,
                }))
            }
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

    /// An operator over two integers. The operands have the result's type, a shift amount any
    /// integer type; the result wraps to the type the checker gave `expr`.
    fn integer_binary(&self, expr: ExprId, op: BinOp, a: i128, b: i128) -> Result<Constant, ConstFault> {
        let compare = |holds: bool| Ok(Constant::Bool(holds));
        match op {
            BinOp::Lt => return compare(a < b),
            BinOp::Le => return compare(a <= b),
            BinOp::Gt => return compare(a > b),
            BinOp::Ge => return compare(a >= b),
            BinOp::Eq => return compare(a == b),
            BinOp::Ne => return compare(a != b),
            _ => {}
        }
        let Some(width) = self.width(expr) else {
            return Err(ConstFault::Unsupported { expr, what: "an integer operator outside an integer type".to_string() });
        };
        let panic = |what: &str| Err(ConstFault::Panic { expr, what: what.to_string() });
        let value = match op {
            BinOp::Add => a.wrapping_add(b),
            BinOp::Sub => a.wrapping_sub(b),
            BinOp::Mul => a.wrapping_mul(b),
            BinOp::Div if b == 0 => return panic("an integer division by zero"),
            BinOp::Rem if b == 0 => return panic("an integer remainder by zero"),
            BinOp::Div => a.wrapping_div(b),
            BinOp::Rem => a.wrapping_rem(b),
            BinOp::BitAnd => a & b,
            BinOp::BitOr => a | b,
            BinOp::BitXor => a ^ b,
            BinOp::Shl | BinOp::Shr if b < 0 => return panic("a shift by a negative amount"),
            // G5: a shift by the width or more is the mathematical result truncated to the width.
            BinOp::Shl if b >= i128::from(width.bits) => 0,
            BinOp::Shr if b >= i128::from(width.bits) => {
                if a < 0 {
                    -1
                } else {
                    0
                }
            }
            BinOp::Shl => a << b,
            // `a` is held sign-extended, so `>>` is arithmetic for a signed type and logical for
            // an unsigned one (whose value is never negative).
            BinOp::Shr => a >> b,
            _ => return Err(ConstFault::Unsupported { expr, what: "this operator on integers".to_string() }),
        };
        Ok(width.wrap(value))
    }

    /// `operand as T`: among the integer types by truncation and sign extension (§1 rule 1),
    /// from `f64` saturating with NaN as 0 (G4), from `bool` as 0 or 1; to `f64` exactly as the
    /// compiled code converts.
    fn cast(&mut self, expr: ExprId, operand: ExprId, bindings: &Bindings) -> Result<Folded, ConstFault> {
        let Folded::Known(value) = self.folded(operand, bindings)? else {
            return Ok(Folded::Unknown);
        };
        let converted = match (self.types.ty(expr), value) {
            (Some(Ty::F64), Constant::Int(v)) => Constant::Real(Real::new(v as f64)),
            (Some(Ty::F64), Constant::Real(r)) => Constant::Real(r),
            (Some(to), value) => match (Width::of(to), value) {
                (Some(width), Constant::Int(v)) => width.wrap(v),
                (Some(width), Constant::Real(r)) => width.saturate(r.get()),
                (Some(width), Constant::Bool(b)) => width.wrap(i128::from(b)),
                (None, _) => return Err(ConstFault::Unsupported { expr, what: "this cast".to_string() }),
            },
            (None, _) => return Err(ConstFault::Unsupported { expr, what: "this cast".to_string() }),
        };
        Ok(Folded::Known(converted))
    }

    /// An `if` as a value: a known condition takes its arm, an unknown one merges both (§3.3.3).
    fn conditional(&mut self, expr: ExprId, cond: ExprId, then: &BlockNode, else_: Option<ExprId>, bindings: &Bindings) -> Result<Folded, ConstFault> {
        let Folded::Known(condition) = self.folded(cond, bindings)? else {
            return self.merged_arms(expr, then, else_, bindings);
        };
        match (Self::boolean(cond, condition)?, else_) {
            (true, _) => self.block(expr, &then.stmts, bindings),
            (false, Some(else_)) => self.folded(else_, bindings),
            (false, None) => Err(ConstFault::Unsupported { expr, what: "an `if` without an `else` as a value".to_string() }),
        }
    }

    /// An `if` on an unknown condition: both arms are taken and the merge of what they yield is
    /// the value (§3.3.3). An arm that panics is no fault, it makes the merge unknown (§1 rule 13).
    fn merged_arms(&mut self, expr: ExprId, then: &BlockNode, else_: Option<ExprId>, bindings: &Bindings) -> Result<Folded, ConstFault> {
        let Some(else_) = else_ else {
            return Err(ConstFault::Unsupported { expr, what: "an `if` without an `else` as a value".to_string() });
        };
        let (first, second) = (self.block(expr, &then.stmts, bindings), self.folded(else_, bindings));
        match (first, second) {
            (Ok(a), Ok(b)) => Ok(a.merge(b)),
            (Err(ConstFault::Panic { .. }), Ok(_) | Err(ConstFault::Panic { .. })) | (Ok(_), Err(ConstFault::Panic { .. })) => Ok(Folded::Unknown),
            (Err(fault), _) | (_, Err(fault)) => Err(fault),
        }
    }

    /// The value of a block: its last statement, an expression without `;`.
    fn block(&mut self, expr: ExprId, stmts: &[StmtId], bindings: &Bindings) -> Result<Folded, ConstFault> {
        let mut value = None;
        for stmt in stmts {
            value = None;
            match self.design.stmt_node(*stmt).kind.clone() {
                StmtKind::Let { pat, init: Some(init), else_: None, .. } => {
                    let bound = self.folded(init, bindings)?;
                    match &self.design.pat_node(pat).kind {
                        PatKind::Ident { binding, sub: None, .. } => {
                            self.locals.insert(*binding, bound);
                        }
                        _ => return Err(ConstFault::Unsupported { expr, what: "a `let` pattern that is not a name".to_string() }),
                    }
                }
                StmtKind::Expr { expr: inner, semi } => {
                    let inner_value = self.folded(inner, bindings)?;
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
    fn call(&mut self, expr: ExprId, args: &[ExprId], bindings: &Bindings) -> Result<Folded, ConstFault> {
        if self.types.callee(expr) == Some(Callee::Intrinsic(Intrinsic::Logic)) {
            return self.logic(expr, args);
        }
        let Some(Callee::Function(function)) = self.types.callee(expr) else {
            let what = match &self.design.expr_node(expr).kind {
                ExprKind::Call { class: crate::pom::CallClass::Probe, .. } => "a probe",
                ExprKind::Call { class: crate::pom::CallClass::Instance { .. }, .. } => "the result of an instance",
                _ => "a call that is not a function",
            };
            return Err(ConstFault::Dynamic { expr, what: what.to_string() });
        };
        let mut values = Vec::new();
        for arg in args {
            values.push(self.folded(*arg, bindings)?);
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

    /// `logic::<T>("..")` (§22.1.4): the value whose level `k` is the pattern's character `WIDTH - 1
    /// - k`, a literal `0` or `1`. A pattern with an `x` is `Unknown` (one flag per value, G6
    /// provisional: the per-level spec gives it its partial form), and so is a tag no variant of an
    /// enum target has (§22.1.3 rule 2). The checker reported a pattern that is not a literal of
    /// `T::WIDTH` characters of `0`, `1` and `x` (`E0702`).
    fn logic(&self, expr: ExprId, args: &[ExprId]) -> Result<Folded, ConstFault> {
        let pattern = args.first().and_then(|a| match &self.design.expr_node(*a).kind {
            ExprKind::Lit(lit) => lit.str_value(),
            _ => None,
        });
        let Some(pattern) = pattern else {
            return Err(ConstFault::Unsupported { expr, what: "a `logic` pattern that is not a string literal".to_string() });
        };
        if pattern.contains('x') {
            return Ok(Folded::Unknown);
        }
        let bits = pattern.chars().fold(0u64, |bits, level| (bits << 1) | u64::from(level == '1'));
        let known = |value: Constant| Ok(Folded::Known(value));
        match self.types.ty(expr) {
            Some(Ty::Bool) => known(Constant::Bool(bits != 0)),
            Some(Ty::F64) => known(Constant::Real(Real::new(f64::from_bits(bits)))),
            Some(Ty::Enum(decl)) if !EnumLayout::of(self.design, *decl).tags().contains(&i128::from(bits)) => Ok(Folded::Unknown),
            Some(Ty::Enum(_)) => known(Constant::Int(i128::from(bits))),
            Some(ty) => match Width::of(ty) {
                Some(width) => known(width.wrap(i128::from(bits))),
                None => Err(ConstFault::Unsupported { expr, what: "a `logic` of a type with no levels".to_string() }),
            },
            None => Err(ConstFault::Unsupported { expr, what: "a `logic` the checker did not type".to_string() }),
        }
    }

    /// A method call: a read of a flag (§3.3.4), or a row of the Appendix H catalogue applied to
    /// its receiver and arguments, which is unknown when one of them is (§3.3.2).
    fn method(&mut self, expr: ExprId, recv: ExprId, args: &[ExprId], bindings: &Bindings) -> Result<Folded, ConstFault> {
        let receiver = self.folded(recv, bindings);
        let mut values = Vec::with_capacity(args.len());
        for arg in args {
            values.push(self.folded(*arg, bindings));
        }
        if let Some(builtin) = self.types.builtin(expr) {
            return Self::flag_read(expr, builtin, receiver?, values.into_iter().next().transpose()?);
        }
        let Some(method) = self.types.method(expr) else {
            return Err(ConstFault::Unsupported { expr, what: "a method outside the catalogue".to_string() });
        };
        let (receiver, values) = (receiver?, values.into_iter().collect::<Result<Vec<_>, _>>()?);
        let known: Option<Vec<Constant>> = values.iter().map(|v| if let Folded::Known(c) = v { Some(*c) } else { None }).collect();
        match (receiver, known) {
            (Folded::Known(receiver), Some(values)) => self.known_method(expr, recv, args, method, receiver, values).map(Folded::Known),
            _ => Ok(Folded::Unknown),
        }
    }

    /// `known_or`, `is_known` and `high` of a folded value: known whatever the value holds
    /// (§3.3.4). `known_or` gives its default, which may itself be unknown, for an unknown value
    /// (§22.1.1 rule 4). The rest of the flag reads (`known`, `level`) are not folded.
    fn flag_read(expr: ExprId, builtin: Builtin, receiver: Folded, argument: Option<Folded>) -> Result<Folded, ConstFault> {
        match (builtin, receiver, argument) {
            (Builtin::KnownOr, Folded::Known(_), _) => Ok(receiver),
            (Builtin::KnownOr, Folded::Unknown, Some(default)) => Ok(default),
            (Builtin::IsKnown, _, _) => Ok(Folded::Known(Constant::Bool(matches!(receiver, Folded::Known(_))))),
            (Builtin::High, _, _) => Ok(Folded::Known(Constant::Bool(receiver == Folded::Known(Constant::Bool(true))))),
            _ => Err(ConstFault::Unsupported { expr, what: "this method in a const expression".to_string() }),
        }
    }

    /// A catalogue row applied to known values.
    fn known_method(&self, expr: ExprId, recv: ExprId, args: &[ExprId], method: Method, receiver: Constant, values: Vec<Constant>) -> Result<Constant, ConstFault> {
        let crossed = || Err(ConstFault::Panic { expr, what: "a `clamp` whose lower bound is above its upper bound".to_string() });
        if let Constant::Int(x) = receiver {
            let ints: Vec<i128> = values.iter().filter_map(|v| if let Constant::Int(i) = v { Some(*i) } else { None }).collect();
            return match (method, ints.as_slice()) {
                (Method::Min, [y]) => Ok(Constant::Int(x.min(*y))),
                (Method::Max, [y]) => Ok(Constant::Int(x.max(*y))),
                (Method::Clamp, [lo, hi]) if lo > hi => crossed(),
                (Method::Clamp, [lo, hi]) => Ok(Constant::Int(x.max(*lo).min(*hi))),
                (Method::RemEuclid, [0]) => Err(ConstFault::Panic { expr, what: "a `rem_euclid` by zero".to_string() }),
                (Method::RemEuclid, [d]) => Ok(Constant::Int(x.rem_euclid(*d))),
                (Method::IntAbs | Method::Signum | Method::Pow | Method::SaturatingAdd | Method::SaturatingSub | Method::SaturatingMul, _) => {
                    let Some(width) = self.width(expr) else { return Err(ConstFault::Unsupported { expr, what: "this method of an integer".to_string() }) };
                    match (method, ints.as_slice()) {
                        // Arithmetic wraps (§1): `(-128i8).abs()` is `-128`, `2u8.pow(9)` is 0.
                        (Method::IntAbs, []) => Ok(width.wrap(x.abs())),
                        (Method::Signum, []) => Ok(Constant::Int(x.signum())),
                        (Method::Pow, [e]) => {
                            let (mut base, mut acc, mut e) = (x, 1i128, *e);
                            while e > 0 {
                                if e & 1 == 1 {
                                    acc = width.wrapped(acc.wrapping_mul(base));
                                }
                                base = width.wrapped(base.wrapping_mul(base));
                                e >>= 1;
                            }
                            Ok(width.wrap(acc))
                        }
                        (Method::SaturatingAdd, [y]) => Ok(Constant::Int(width.clamp(x + *y))),
                        (Method::SaturatingSub, [y]) => Ok(Constant::Int(width.clamp(x - *y))),
                        (Method::SaturatingMul, [y]) => Ok(Constant::Int(width.clamp(x.saturating_mul(*y)))),
                        _ => Err(ConstFault::Unsupported { expr, what: "this method of an integer".to_string() }),
                    }
                }
                _ => Err(ConstFault::Unsupported { expr, what: "this method of an integer".to_string() }),
            };
        }
        let x = Self::real(recv, receiver)?;
        let mut reals = Vec::with_capacity(values.len());
        for (arg, value) in args.iter().zip(values) {
            reals.push(Self::real(*arg, value)?);
        }
        let result = match (method, reals.as_slice()) {
            (Method::Exp, []) => x.exp(),
            (Method::Ln, []) => x.ln(),
            (Method::Sqrt, []) => x.sqrt(),
            (Method::Abs, []) => x.abs(),
            (Method::Floor, []) => x.floor(),
            (Method::Min, [y]) => x.min(*y),
            (Method::Max, [y]) => x.max(*y),
            // `f64::clamp` panics on crossed or NaN bounds, as the compiled code does.
            (Method::Clamp, [lo, hi]) if lo.is_nan() || hi.is_nan() || lo > hi => return crossed(),
            (Method::Clamp, [lo, hi]) => x.clamp(*lo, *hi),
            // Folded from msl-digital T27 on, with the `Option` pair.
            _ => return Err(ConstFault::Unsupported { expr, what: "this method in a const expression".to_string() }),
        };
        Ok(Constant::Real(Real::new(result)))
    }
}

/// The width and sign of an integer type: what a result wraps to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Width {
    bits: u32,
    signed: bool,
}

impl Width {
    fn of(ty: &Ty) -> Option<Width> {
        match *ty {
            Ty::UInt(n) => Some(Width { bits: u32::from(n), signed: false }),
            Ty::Int(n) => Some(Width { bits: u32::from(n), signed: true }),
            _ => None,
        }
    }

    /// `v` modulo `2^bits` (§1 rule 6), held as the compiled code holds it: below `2^bits`
    /// unsigned, sign-extended signed.
    fn wrap(self, v: i128) -> Constant {
        let modulus = 1i128 << self.bits;
        let low = v.rem_euclid(modulus);
        Constant::Int(if self.signed && low >= modulus / 2 { low - modulus } else { low })
    }

    /// The value of `v` modulo `2^bits` as an `i128`, for a running product.
    fn wrapped(self, v: i128) -> i128 {
        match self.wrap(v) {
            Constant::Int(w) => w,
            _ => v,
        }
    }

    /// `v` clamped to the range of this type (`saturating_*`).
    fn clamp(self, v: i128) -> i128 {
        let (min, max) = if self.signed { (-(1i128 << (self.bits - 1)), (1i128 << (self.bits - 1)) - 1) } else { (0, (1i128 << self.bits) - 1) };
        v.clamp(min, max)
    }

    /// `x as` this type (G4): truncated toward zero, saturated to the type's range, NaN as 0.
    fn saturate(self, x: f64) -> Constant {
        if x.is_nan() {
            return Constant::Int(0);
        }
        let (min, max) = if self.signed { (-(1i128 << (self.bits - 1)), (1i128 << (self.bits - 1)) - 1) } else { (0, (1i128 << self.bits) - 1) };
        // `as i128` truncates toward zero and saturates; the range of 64 bits fits in it.
        Constant::Int((x as i128).clamp(min, max))
    }
}
