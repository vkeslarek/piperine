//! Symbolic differentiation of an `f64` expression (§14.2.3): [`Differentiation::of`] builds
//! `∂e/∂v` as ordinary IR expressions next to `e`, so no backend learns a new form.
//!
//! The variable `v` is a potential probe, a branch flow, or an argument of a function. The rule
//! of each form is design.md "ddx is symbolic" mechanism 1:
//!
//! * a leaf the run supplies (a param, the environment, a session parameter, a carrier, a site's
//!   result, a hoisted result of another instance) is a constant, so its derivative is 0;
//! * `limit` is transparent (§18 rule 3): the derivative of its `x`;
//! * the sum, product and quotient rules, the chain rule through `exp`, `ln`, `sqrt` and the
//!   methods, and piecewise rules (`abs`, `min`, `max`, `clamp`, `if`);
//! * a call is `Σ ∂f/∂argᵢ · ∂argᵢ`, where `∂f/∂argᵢ` is a derived function built once per
//!   `(function, i)` and memoized by [`IrBuilder::derived_function`];
//! * a block differentiates forward: each `let x = e` gains a `let ∂x = ∂e`;
//! * a field of a struct or a tuple written out (`Gain { k: 7.0 }.k`) is the derivative of the part it names;
//! * a form with no rule (a loop, an assignment, a closure, a call through a value) is
//!   [`NotDifferentiable`], naming the form.
//!
//! A zero is folded as it is built (`0 + x`, `0 · x`), so the derivative of an expression that
//! does not read the variable is the literal `0.0`.

use std::collections::BTreeMap;
use std::fmt;

use crate::build::{FunctionSpec, IrBuilder};
use crate::ids::{BodyId, ExprId, FunctionId};
use crate::model::{BinaryOp, Expr, Local, MathFn, Method, Origin, Stmt, Terminal, UnaryOp, ValueType};
use crate::real::Real;

/// What an expression is differentiated against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variable {
    /// The potential probe `v(plus, minus)`: the probe of the same ends is 1, the reversed
    /// probe is −1, any other probe 0.
    Potential { plus: Terminal, minus: Terminal },
    /// The flow of branch `branch` of the body (`i(..)`); the flow of another branch is 0.
    Flow { branch: u32 },
    /// The function argument held in local `argument` (the first locals of a function are its
    /// arguments).
    Argument(u32),
}

/// A form of an expression that has no derivative rule: `P0001` names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotDifferentiable {
    /// The form, spelled as the source does (`a loop`, `an assignment`, `a recursive function`).
    pub form: &'static str,
    /// Where the form stands.
    pub origin: Origin,
}

impl fmt::Display for NotDifferentiable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} cannot be differentiated (at {}:{}:{})", self.form, self.origin.file, self.origin.line, self.origin.col)
    }
}

impl std::error::Error for NotDifferentiable {}

/// The locals of the function being differentiated: where each differentiated local's
/// derivative lives, and the locals the derived function gains.
struct Frame {
    /// The derivative local of each local that holds an `f64` the walk differentiated.
    derivative_of: BTreeMap<u32, u32>,
    /// The locals of the derived function: those of the original, then one per derivative.
    locals: Vec<Local>,
}

/// One differentiation of the expressions of a body or of a function against a [`Variable`].
pub struct Differentiation<'b> {
    builder: &'b mut IrBuilder,
    variable: Variable,
    /// `Some` while a function body is differentiated: its locals are readable then.
    frame: Option<Frame>,
}

impl<'b> Differentiation<'b> {
    /// A differentiation of the expressions of `body` against `variable`. The body names the
    /// scope the expressions were lowered in: its probes and flows are the only ones the
    /// derivative reads; its locals are not readable (a derivative of a body is lowered
    /// standalone, §14 rule 9).
    pub fn new(builder: &'b mut IrBuilder, _body: BodyId, variable: Variable) -> Self {
        Differentiation { builder, variable, frame: None }
    }

    /// `∂expr/∂variable`, an `f64` expression of the same builder.
    pub fn of(&mut self, expr: ExprId) -> Result<ExprId, NotDifferentiable> {
        let (kind, ty, origin) = match self.builder.expr_data(expr) {
            Some(data) => (data.kind.clone(), data.ty.clone(), data.origin.clone()),
            None => return Err(NotDifferentiable { form: "an expression that is not in the IR", origin: Origin { file: String::new(), line: 0, col: 0 } }),
        };
        // an integer, a boolean or a unit has no derivative against a real variable
        if ty != ValueType::F64 {
            return Ok(self.zero(&origin));
        }
        match kind {
            Expr::Real(_) | Expr::Int(_) | Expr::Bool(_) | Expr::Unit | Expr::Param(_) | Expr::Env(_) | Expr::Carrier(_) | Expr::Previous(_) | Expr::ParamPrevious(_) | Expr::Derivative(_) | Expr::Delay(_) | Expr::Noise(_) | Expr::Stim { .. } | Expr::SessionParameter { .. } => Ok(self.zero(&origin)),
            Expr::Probe { plus, minus } => Ok(self.probe(plus, minus, &origin)),
            Expr::Flow { branch } => Ok(match self.variable {
                Variable::Flow { branch: wanted } if wanted == branch => self.one(&origin),
                _ => self.zero(&origin),
            }),
            Expr::Limit { x, .. } => self.of(x),
            Expr::Local(local) => self.local(local, &origin),
            Expr::Unary { op: UnaryOp::Neg, expr } => {
                let inner = self.of(expr)?;
                Ok(self.negate(inner, &origin))
            }
            Expr::Unary { op: UnaryOp::Not, .. } => Ok(self.zero(&origin)),
            Expr::Binary { op, lhs, rhs } => self.binary(op, lhs, rhs, expr, &origin),
            Expr::If { cond, then, otherwise } => {
                let (then, otherwise) = (self.of(then)?, self.of(otherwise)?);
                if self.is_zero(then) && self.is_zero(otherwise) {
                    return Ok(self.zero(&origin));
                }
                Ok(self.add_expr(Expr::If { cond, then, otherwise }, ValueType::F64, &origin))
            }
            Expr::Math { f, args } => self.math(f, args[0], expr, &origin),
            Expr::Method { method, receiver, args } => self.method(method, receiver, &args, expr, &origin),
            Expr::Cast { expr: inner, .. } => {
                // only a cast of an `f64` reaches here (the result is `f64`): an integer or a
                // boolean source is a step function of the variable
                let source = self.builder.expr_data(inner).map(|d| d.ty.clone());
                if source == Some(ValueType::F64) { self.of(inner) } else { Ok(self.zero(&origin)) }
            }
            Expr::Call { function, args } => self.call(function, &args, &origin),
            Expr::Block { stmts, value } => self.block(&stmts, value, &origin),
            // a field of a struct or a tuple written out is the part it names: the derivative of that part
            Expr::Field { expr: base, index } => match self.builder.expr_data(base).map(|d| d.kind.clone()) {
                Some(Expr::StructOf { fields, .. } | Expr::Tuple(fields)) if (index as usize) < fields.len() => self.of(fields[index as usize]),
                _ => Err(NotDifferentiable { form: "a field of a composite value that is not written out", origin }),
            },
            other => Err(NotDifferentiable { form: Self::form_name(&other), origin }),
        }
    }

    /// The probe rule: 1 for the variable's own probe, −1 for its reverse, 0 otherwise.
    fn probe(&mut self, plus: Terminal, minus: Terminal, origin: &Origin) -> ExprId {
        match self.variable {
            Variable::Potential { plus: p, minus: m } if (plus, minus) == (p, m) => self.one(origin),
            Variable::Potential { plus: p, minus: m } if (plus, minus) == (m, p) => self.constant(-1.0, origin),
            _ => self.zero(origin),
        }
    }

    /// The derivative of a read of local `local`: 1 for the argument, the derivative local of a
    /// `let`, else 0. A body has no readable locals.
    fn local(&mut self, local: u32, origin: &Origin) -> Result<ExprId, NotDifferentiable> {
        let Some(frame) = &self.frame else {
            return Err(NotDifferentiable { form: "a local outside a function", origin: origin.clone() });
        };
        if self.variable == Variable::Argument(local) {
            return Ok(self.one(origin));
        }
        match frame.derivative_of.get(&local).copied() {
            Some(derivative) => Ok(self.add_expr(Expr::Local(derivative), ValueType::F64, origin)),
            None => Ok(self.zero(origin)),
        }
    }

    fn binary(&mut self, op: BinaryOp, lhs: ExprId, rhs: ExprId, whole: ExprId, origin: &Origin) -> Result<ExprId, NotDifferentiable> {
        match op {
            BinaryOp::Add | BinaryOp::Sub => {
                let (a, b) = (self.of(lhs)?, self.of(rhs)?);
                Ok(if op == BinaryOp::Add { self.sum(a, b, origin) } else { self.difference(a, b, origin) })
            }
            BinaryOp::Mul => {
                // (a·b)' = a'·b + a·b'
                let (a, b) = (self.of(lhs)?, self.of(rhs)?);
                let left = self.product(a, rhs, origin);
                let right = self.product(lhs, b, origin);
                Ok(self.sum(left, right, origin))
            }
            BinaryOp::Div => {
                // (a/b)' = (a' - (a/b)·b') / b
                let (a, b) = (self.of(lhs)?, self.of(rhs)?);
                if self.is_zero(b) {
                    return Ok(self.quotient(a, rhs, origin));
                }
                let scaled = self.product(whole, b, origin);
                let numerator = self.difference(a, scaled, origin);
                Ok(self.quotient(numerator, rhs, origin))
            }
            BinaryOp::Rem => Err(NotDifferentiable { form: "a remainder", origin: origin.clone() }),
            // a comparison, a logic or a bit operator has no `f64` result
            _ => Ok(self.zero(origin)),
        }
    }

    fn math(&mut self, f: MathFn, x: ExprId, whole: ExprId, origin: &Origin) -> Result<ExprId, NotDifferentiable> {
        let dx = self.of(x)?;
        if self.is_zero(dx) {
            return Ok(dx);
        }
        Ok(match f {
            // exp(x)' = exp(x)·x'
            MathFn::Exp => self.product(whole, dx, origin),
            // ln(x)' = x'/x
            MathFn::Ln => self.quotient(dx, x, origin),
            // sqrt(x)' = x' / (2·sqrt(x))
            MathFn::Sqrt => {
                let two = self.constant(2.0, origin);
                let twice = self.product(two, whole, origin);
                self.quotient(dx, twice, origin)
            }
            // |x|' = x' for x >= 0, -x' below
            MathFn::Abs => {
                let zero = self.zero(origin);
                let cond = self.add_expr(Expr::Binary { op: BinaryOp::Ge, lhs: x, rhs: zero }, ValueType::Bool, origin);
                let negative = self.negate(dx, origin);
                self.add_expr(Expr::If { cond, then: dx, otherwise: negative }, ValueType::F64, origin)
            }
        })
    }

    fn method(&mut self, method: Method, receiver: ExprId, args: &[ExprId], whole: ExprId, origin: &Origin) -> Result<ExprId, NotDifferentiable> {
        let derivatives: Vec<ExprId> = std::iter::once(receiver).chain(args.iter().copied()).map(|e| self.of(e)).collect::<Result<_, _>>()?;
        if derivatives.iter().all(|d| self.is_zero(*d)) && !matches!(method, Method::Powf | Method::Powi) {
            return Ok(self.zero(origin));
        }
        let dx = derivatives[0];
        Ok(match method {
            // a step function of x
            Method::Floor | Method::Ceil | Method::Round | Method::Trunc | Method::Signum => self.zero(origin),
            Method::Fract => dx,
            Method::Sin => {
                let cos = self.method_of(Method::Cos, receiver, &[], origin);
                self.product(cos, dx, origin)
            }
            Method::Cos => {
                let sin = self.method_of(Method::Sin, receiver, &[], origin);
                let negative = self.negate(sin, origin);
                self.product(negative, dx, origin)
            }
            Method::Tan => {
                // tan(x)' = (1 + tan(x)²)·x'
                let square = self.product(whole, whole, origin);
                let one = self.one(origin);
                let slope = self.sum(one, square, origin);
                self.product(slope, dx, origin)
            }
            Method::Asin | Method::Acos => {
                // asin(x)' = x'/sqrt(1 - x²), acos(x)' = -asin(x)'
                let square = self.product(receiver, receiver, origin);
                let one = self.one(origin);
                let gap = self.difference(one, square, origin);
                let root = self.math_of(MathFn::Sqrt, gap, origin);
                let slope = self.quotient(dx, root, origin);
                if method == Method::Acos { self.negate(slope, origin) } else { slope }
            }
            Method::Atan => {
                let square = self.product(receiver, receiver, origin);
                let one = self.one(origin);
                let norm = self.sum(one, square, origin);
                self.quotient(dx, norm, origin)
            }
            Method::Atan2 => {
                // atan2(y, x)' = (x·y' - y·x') / (x² + y²)
                let (dy, dxx) = (derivatives[0], derivatives[1]);
                let x = args[0];
                let left = self.product(x, dy, origin);
                let right = self.product(receiver, dxx, origin);
                let numerator = self.difference(left, right, origin);
                let x_square = self.product(x, x, origin);
                let y_square = self.product(receiver, receiver, origin);
                let norm = self.sum(x_square, y_square, origin);
                self.quotient(numerator, norm, origin)
            }
            Method::Sinh => {
                let cosh = self.method_of(Method::Cosh, receiver, &[], origin);
                self.product(cosh, dx, origin)
            }
            Method::Cosh => {
                let sinh = self.method_of(Method::Sinh, receiver, &[], origin);
                self.product(sinh, dx, origin)
            }
            Method::Tanh => {
                // tanh(x)' = (1 - tanh(x)²)·x'
                let square = self.product(whole, whole, origin);
                let one = self.one(origin);
                let slope = self.difference(one, square, origin);
                self.product(slope, dx, origin)
            }
            Method::Log10 | Method::Log2 => {
                let base = if method == Method::Log10 { std::f64::consts::LN_10 } else { std::f64::consts::LN_2 };
                let scale = self.constant(base, origin);
                let denominator = self.product(receiver, scale, origin);
                self.quotient(dx, denominator, origin)
            }
            Method::Hypot => {
                // hypot(a, b)' = (a·a' + b·b') / hypot(a, b)
                let (da, db) = (derivatives[0], derivatives[1]);
                let left = self.product(receiver, da, origin);
                let right = self.product(args[0], db, origin);
                let numerator = self.sum(left, right, origin);
                self.quotient(numerator, whole, origin)
            }
            Method::Powf => self.power(receiver, args[0], derivatives[0], derivatives[1], whole, origin),
            Method::Powi => self.integer_power(receiver, args[0], dx, origin),
            Method::Min | Method::Max => {
                let op = if method == Method::Min { BinaryOp::Le } else { BinaryOp::Ge };
                let cond = self.add_expr(Expr::Binary { op, lhs: receiver, rhs: args[0] }, ValueType::Bool, origin);
                self.add_expr(Expr::If { cond, then: derivatives[0], otherwise: derivatives[1] }, ValueType::F64, origin)
            }
            Method::Clamp => {
                // clamp(x, lo, hi)': lo' below lo, hi' above hi, else x'
                let below = self.add_expr(Expr::Binary { op: BinaryOp::Lt, lhs: receiver, rhs: args[0] }, ValueType::Bool, origin);
                let above = self.add_expr(Expr::Binary { op: BinaryOp::Gt, lhs: receiver, rhs: args[1] }, ValueType::Bool, origin);
                let upper = self.add_expr(Expr::If { cond: above, then: derivatives[2], otherwise: derivatives[0] }, ValueType::F64, origin);
                self.add_expr(Expr::If { cond: below, then: derivatives[1], otherwise: upper }, ValueType::F64, origin)
            }
            Method::RemEuclid => return Err(NotDifferentiable { form: "a euclidean remainder", origin: origin.clone() }),
            // the remaining methods hold integers or answer a `bool`: no `f64` result
            _ => self.zero(origin),
        })
    }

    /// `powf`: `a^b·(b'·ln a + b·a'/a)`, with the two simple cases kept short.
    fn power(&mut self, base: ExprId, exponent: ExprId, d_base: ExprId, d_exponent: ExprId, whole: ExprId, origin: &Origin) -> ExprId {
        if self.is_zero(d_exponent) {
            if self.is_zero(d_base) {
                return self.zero(origin);
            }
            // (a^n)' = n·a^(n-1)·a'
            let one = self.one(origin);
            let lowered = self.difference(exponent, one, origin);
            let reduced = self.method_of(Method::Powf, base, &[lowered], origin);
            let scaled = self.product(exponent, reduced, origin);
            return self.product(scaled, d_base, origin);
        }
        let log = self.math_of(MathFn::Ln, base, origin);
        let from_exponent = self.product(d_exponent, log, origin);
        let from_base = {
            let ratio = self.quotient(d_base, base, origin);
            self.product(exponent, ratio, origin)
        };
        let total = self.sum(from_exponent, from_base, origin);
        self.product(whole, total, origin)
    }

    /// `powi(a, n)' = n·powi(a, n - 1)·a'`, `n` an integer expression.
    fn integer_power(&mut self, base: ExprId, exponent: ExprId, d_base: ExprId, origin: &Origin) -> ExprId {
        if self.is_zero(d_base) {
            return d_base;
        }
        let exponent_type = self.builder.expr_data(exponent).map_or(ValueType::Int(32), |d| d.ty.clone());
        let one = self.add_expr(Expr::Int(1), exponent_type.clone(), origin);
        let lowered = self.add_expr(Expr::Binary { op: BinaryOp::Sub, lhs: exponent, rhs: one }, exponent_type, origin);
        let reduced = self.method_of(Method::Powi, base, &[lowered], origin);
        let factor = self.add_expr(Expr::Cast { expr: exponent, to: ValueType::F64 }, ValueType::F64, origin);
        let scaled = self.product(factor, reduced, origin);
        self.product(scaled, d_base, origin)
    }

    /// `Σ ∂f/∂argᵢ(args)·∂argᵢ`, one derived function per `(function, i)`.
    fn call(&mut self, function: FunctionId, args: &[ExprId], origin: &Origin) -> Result<ExprId, NotDifferentiable> {
        let mut total = self.zero(origin);
        for (index, arg) in args.iter().enumerate() {
            let inner = self.of(*arg)?;
            if self.is_zero(inner) {
                continue;
            }
            let derived = self.derived_function(function, index as u32, origin)?;
            let partial = self.add_expr(Expr::Call { function: derived, args: args.to_vec() }, ValueType::F64, origin);
            let term = self.product(partial, inner, origin);
            total = self.sum(total, term, origin);
        }
        Ok(total)
    }

    /// The function `∂function/∂argument`, built once: the body of `function` differentiated
    /// against its local `argument`, with the same arguments.
    fn derived_function(&mut self, function: FunctionId, argument: u32, origin: &Origin) -> Result<FunctionId, NotDifferentiable> {
        if let Some(built) = self.builder.derived_function(function, argument) {
            return Ok(built);
        }
        if !self.builder.begin_derived(function, argument) {
            return Err(NotDifferentiable { form: "a recursive function", origin: origin.clone() });
        }
        let Some(original) = self.builder.function_data(function).cloned() else {
            self.builder.abandon_derived(function, argument);
            return Err(NotDifferentiable { form: "a function that is not in the IR", origin: origin.clone() });
        };
        if original.ret != ValueType::F64 {
            self.builder.abandon_derived(function, argument);
            return Err(NotDifferentiable { form: "a function that returns no `f64`", origin: original.origin });
        }
        let frame = Frame { derivative_of: BTreeMap::new(), locals: original.locals.clone() };
        let mut inner = Differentiation { builder: &mut *self.builder, variable: Variable::Argument(argument), frame: Some(frame) };
        let result = inner.of(original.body);
        let locals = inner.frame.take().map(|f| f.locals).unwrap_or_default();
        let body = match result {
            Ok(body) => body,
            Err(error) => {
                self.builder.abandon_derived(function, argument);
                return Err(error);
            }
        };
        let derived = self.builder.function(FunctionSpec { name: format!("{}$d{argument}", original.name), origin: original.origin, params: original.params, ret: ValueType::F64, locals, body });
        self.builder.end_derived(function, argument, derived);
        Ok(derived)
    }

    /// A block differentiates forward: each `let x = e` of an `f64` is followed by
    /// `let ∂x = ∂e`; the derivative of the block is that of its value.
    fn block(&mut self, stmts: &[crate::ids::StmtId], value: Option<ExprId>, origin: &Origin) -> Result<ExprId, NotDifferentiable> {
        if stmts.is_empty() {
            return match value {
                Some(value) => self.of(value),
                None => Ok(self.zero(origin)),
            };
        }
        if self.frame.is_none() {
            return Err(NotDifferentiable { form: "a block with `let` outside a function", origin: origin.clone() });
        }
        let mut lowered = Vec::with_capacity(stmts.len() * 2);
        for stmt in stmts {
            let Some(data) = self.builder.stmt_data(*stmt).cloned() else { continue };
            match data {
                Stmt::Let { local, init } => {
                    lowered.push(*stmt);
                    let typed_real = self.frame.as_ref().and_then(|f| f.locals.get(local as usize)).is_some_and(|l| l.ty == ValueType::F64);
                    if typed_real {
                        let derivative = self.of(init)?;
                        let slot = self.fresh_local(local);
                        lowered.push(self.builder.stmt(Stmt::Let { local: slot, init: derivative }));
                    }
                }
                // a statement that only reads (a call of a pure function) changes no derivative
                Stmt::Expr(_) => lowered.push(*stmt),
                Stmt::Assign { .. } => return Err(NotDifferentiable { form: "an assignment", origin: origin.clone() }),
                _ => return Err(NotDifferentiable { form: "a statement that is no `let`", origin: origin.clone() }),
            }
        }
        let derivative_value = match value {
            Some(value) => self.of(value)?,
            None => self.zero(origin),
        };
        Ok(self.add_expr(Expr::Block { stmts: lowered, value: Some(derivative_value) }, ValueType::F64, origin))
    }

    /// A new local of the derived function holding the derivative of `local`.
    fn fresh_local(&mut self, local: u32) -> u32 {
        let Some(frame) = self.frame.as_mut() else { return local };
        let name = frame.locals.get(local as usize).map_or("d".to_string(), |l| format!("d{}", l.name));
        frame.locals.push(Local { name, ty: ValueType::F64, mutable: false });
        let slot = frame.locals.len() as u32 - 1;
        frame.derivative_of.insert(local, slot);
        slot
    }

    /// The spelling of a form no rule covers.
    fn form_name(kind: &Expr) -> &'static str {
        match kind {
            Expr::Loop { .. } | Expr::While { .. } | Expr::ForRange { .. } => "a loop",
            Expr::Break(_) | Expr::Continue | Expr::Return(_) => "a break, continue or return",
            Expr::Closure { .. } | Expr::FunctionRef(_) | Expr::CallValue { .. } => "a closure or a call through a function value",
            Expr::Table { .. } | Expr::TableSlope { .. } => "a `table`",
            Expr::Tuple(_) | Expr::Array(_) | Expr::ArrayRepeat { .. } | Expr::StructOf { .. } | Expr::VariantOf { .. } | Expr::Field { .. } | Expr::Index { .. } | Expr::Length(_) | Expr::IsVariant { .. } | Expr::VariantField { .. } | Expr::OptionPack { .. } | Expr::OptionPart { .. } | Expr::Default(_) => "a composite value",
            Expr::Unwrap { .. } | Expr::Same { .. } | Expr::LevelCast { .. } => "an `unwrap`, `same` or `cast::<..>`",
            _ => "this form",
        }
    }

    // ---- building, with zero folded ----------------------------------------------------

    fn add_expr(&mut self, kind: Expr, ty: ValueType, origin: &Origin) -> ExprId {
        self.builder.expr(kind, ty, origin.clone())
    }

    fn constant(&mut self, value: f64, origin: &Origin) -> ExprId {
        self.add_expr(Expr::Real(Real::new(value)), ValueType::F64, origin)
    }

    fn zero(&mut self, origin: &Origin) -> ExprId {
        self.constant(0.0, origin)
    }

    fn one(&mut self, origin: &Origin) -> ExprId {
        self.constant(1.0, origin)
    }

    /// Whether `id` is the literal `0.0`.
    fn is_zero(&self, id: ExprId) -> bool {
        matches!(self.builder.expr_kind(id), Some(Expr::Real(r)) if r.get() == 0.0)
    }

    /// Whether `id` is the literal `1.0`.
    fn is_one(&self, id: ExprId) -> bool {
        matches!(self.builder.expr_kind(id), Some(Expr::Real(r)) if r.get() == 1.0)
    }

    fn sum(&mut self, a: ExprId, b: ExprId, origin: &Origin) -> ExprId {
        if self.is_zero(a) {
            return b;
        }
        if self.is_zero(b) {
            return a;
        }
        self.add_expr(Expr::Binary { op: BinaryOp::Add, lhs: a, rhs: b }, ValueType::F64, origin)
    }

    fn difference(&mut self, a: ExprId, b: ExprId, origin: &Origin) -> ExprId {
        if self.is_zero(b) {
            return a;
        }
        if self.is_zero(a) {
            return self.negate(b, origin);
        }
        self.add_expr(Expr::Binary { op: BinaryOp::Sub, lhs: a, rhs: b }, ValueType::F64, origin)
    }

    fn product(&mut self, a: ExprId, b: ExprId, origin: &Origin) -> ExprId {
        if self.is_zero(a) || self.is_zero(b) {
            return self.zero(origin);
        }
        if self.is_one(a) {
            return b;
        }
        if self.is_one(b) {
            return a;
        }
        self.add_expr(Expr::Binary { op: BinaryOp::Mul, lhs: a, rhs: b }, ValueType::F64, origin)
    }

    fn quotient(&mut self, numerator: ExprId, denominator: ExprId, origin: &Origin) -> ExprId {
        if self.is_zero(numerator) {
            return numerator;
        }
        self.add_expr(Expr::Binary { op: BinaryOp::Div, lhs: numerator, rhs: denominator }, ValueType::F64, origin)
    }

    fn negate(&mut self, a: ExprId, origin: &Origin) -> ExprId {
        if self.is_zero(a) {
            return a;
        }
        if let Some(Expr::Real(r)) = self.builder.expr_kind(a) {
            let negated = -r.get();
            return self.constant(negated, origin);
        }
        self.add_expr(Expr::Unary { op: UnaryOp::Neg, expr: a }, ValueType::F64, origin)
    }

    fn math_of(&mut self, f: MathFn, x: ExprId, origin: &Origin) -> ExprId {
        self.add_expr(Expr::Math { f, args: vec![x] }, ValueType::F64, origin)
    }

    fn method_of(&mut self, method: Method, receiver: ExprId, args: &[ExprId], origin: &Origin) -> ExprId {
        self.add_expr(Expr::Method { method, receiver, args: args.to_vec() }, ValueType::F64, origin)
    }
}
