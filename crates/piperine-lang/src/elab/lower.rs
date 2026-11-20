//! Value code: POM expressions lowered to the IR node of the same form.
//!
//! One `Scope` per owner (a module body, later a function) names the params, nodes and locals
//! an expression may read. A `const` read becomes its folded value; a param read stays a
//! `Param` read, never a number (a host `set` must reach the solver without re-elaboration).
//! An integer expression that reads only literals and constants is folded to one literal, by
//! the rules the compiled code follows (`ConstEval`).

use std::collections::BTreeMap;

use piperine_ir::{BinaryOp, Branch, Condition, Constant, Expr, ExprId as LoweredExpr, Local, MathFn, Method as IrMethod, Origin, Site, Stmt, StmtId as LoweredStmt, Terminal, UnaryOp, ValueType};

use super::compound::EnumLayout;
use super::consts::{Bindings, Folded};
use super::known::Flag;
use super::elaborator::Elaboration;
use super::selectors::{ArmCondition, ContributionRecord};
use crate::msl::Span;
use crate::pom::{BinOp, CallClass, DeclId, DeclKind, ExprId, ExprKind, FileId, LitKind, PatKind, Resolution, StmtId, StmtKind, UnOp};
use crate::types::{Callee, Method, Ty};

/// What names the code being lowered: the owner's params and locals (its nodes are those of its
/// frame, `Frame::nodes`).
pub(super) struct Scope {
    pub(super) file: FileId,
    /// Param declaration to its index in the body's params.
    pub(super) params: BTreeMap<DeclId, u32>,
    pub(super) locals: Vec<Local>,
    pub(super) local_index: BTreeMap<DeclId, u32>,
    pub(super) limits: Vec<Site>,
    pub(super) stims: Vec<Site>,
    /// The branches the contributions name, in order of first contribution.
    pub(super) branches: Vec<Branch>,
    /// How many closure bodies and `Option` method bodies the code being lowered sits in: a
    /// `limit`, `ac_stim` or `bound_step` there is refused (the conditions of `if` and `match` arms
    /// are `guards`).
    pub(super) nesting: u32,
    /// The source form of the conditions of the arms the code sits in, outermost first: what a
    /// guarded site lowers standalone (`selectors.rs`).
    pub(super) guards: Vec<ArmCondition>,
    /// The `bound_step` statements met under a condition or in a nested block, moved to the top of
    /// the analog block (`analog.rs`).
    pub(super) hoisted_steps: Vec<LoweredStmt>,
    /// Plain blocks entered below the analog block's own statement list: a contribution there
    /// is not a top-level statement (Part IV §10 rule 8 makes it legal; M1 refuses it).
    pub(super) block_depth: u32,
    /// The name the `let` being lowered gives the `limit` call that is its whole initializer.
    pub(super) limit_name: Option<(ExprId, String)>,
    /// The `Option` locals: a `let` binding of type `Option<T>` to its flag local and its
    /// payload local.
    pub(super) options: BTreeMap<DeclId, (u32, u32)>,
    /// The frame whose names the code reads (`Elaboration::frames`): 0, the body's own module.
    pub(super) frame: usize,
    /// The `&str` params of the functions inlined at a call (`records.rs`), bound to their text.
    pub(super) texts: BTreeMap<DeclId, String>,
    /// Code with no local of its own (a drive): a `match` reads its scrutinee again at each test.
    pub(super) no_locals: bool,
    /// Whether the code being lowered reads `freq()` from the solve (§14.4.4): the analog block
    /// and the functions it calls (a function's `freq_` is the caller's; a reaction reads 0).
    pub(super) analog: bool,
    /// The `bound_step` statements met so far: the next one's site.
    pub(super) bounds: u32,
    /// The conditions of the arms the code being lowered sits in, outermost first: the selector
    /// of a contribution (§11), read off the same structure `Body::contributions` reads.
    pub(super) conditions: Vec<Condition>,
    /// The source expression of each plain `let` met so far: a standalone scope (`derivatives.rs`)
    /// reads a block-local `let` as this expression.
    pub(super) let_sources: BTreeMap<DeclId, ExprId>,
    /// The body `let`s that bind a bare `ddt` call and are read only as whole addends.
    pub(super) addend_lets: std::collections::BTreeSet<DeclId>,
    /// The values of the key params of the body being lowered (`structural.rs`): a condition that
    /// reads only these and constants is folded.
    pub(super) keys: Bindings,
    /// The branches the body merges (`structural.rs`): contributions to them are not lowered.
    pub(super) merged: Vec<(Terminal, Terminal)>,
    /// Every contribution lowered so far, for `selectors.rs` to decide the branch kinds.
    pub(super) contributions: Vec<ContributionRecord>,
    /// The initializer of each local a plain `let` bound: a condition that reads the local reads
    /// what it holds (`selectors.rs`).
    pub(super) local_inits: BTreeMap<u32, LoweredExpr>,
}

impl Scope {
    pub(super) fn new(file: FileId) -> Self {
        Scope { file, params: BTreeMap::new(), locals: Vec::new(), local_index: BTreeMap::new(), limits: Vec::new(), stims: Vec::new(), branches: Vec::new(), nesting: 0, guards: Vec::new(), hoisted_steps: Vec::new(), block_depth: 0, limit_name: None, options: BTreeMap::new(), frame: 0, texts: BTreeMap::new(), no_locals: false, analog: false, bounds: 0, let_sources: BTreeMap::new(), addend_lets: std::collections::BTreeSet::new(), keys: Bindings::new(), merged: Vec::new(), conditions: Vec::new(), contributions: Vec::new(), local_inits: BTreeMap::new() }
    }
}

/// The IR type of a type of the subset.
pub(super) struct ValueTypes<'d> {
    pub(super) design: &'d crate::pom::Design,
}

impl ValueTypes<'_> {
    /// `None` for a type no value has in the IR: a node, a module, an `Option` (a pair of values,
    /// msl-digital T27), an enum of one variant (its tag has no level).
    pub(super) fn of(&self, ty: &Ty) -> Option<ValueType> {
        match ty {
            Ty::F64 => Some(ValueType::F64),
            Ty::Bool => Some(ValueType::Bool),
            Ty::Unit => Some(ValueType::Unit),
            Ty::UInt(n) => Some(ValueType::UInt(u32::from(*n))),
            Ty::Int(n) => Some(ValueType::Int(u32::from(*n))),
            // §2 rule 5: an enum of unit variants is its tag.
            Ty::Enum(decl) => EnumLayout::of(self.design, *decl).ty(),
            Ty::Fn(params, ret) => {
                let params: Option<Vec<ValueType>> = params.iter().map(|p| self.of(p)).collect();
                Some(ValueType::Fn(params?, Box::new(self.of(ret)?)))
            }
            Ty::Node(_) | Ty::AnyNode | Ty::Module(_) | Ty::Option(_) | Ty::Tuple(_) | Ty::Carrier { .. } | Ty::Event | Ty::Str => None,
        }
    }
}

impl Elaboration<'_, '_> {
    /// The IR method a numeric row of Appendix H lowers to; `None` for the rows that lower
    /// elsewhere (`exp`, `ln`, `sqrt`, `abs` of `f64` are `Math`; `checked_*` and the `Option`
    /// and `bool` rows are pairs).
    pub(super) fn ir_method(method: Method) -> Option<IrMethod> {
        Some(match method {
            Method::Min => IrMethod::Min,
            Method::Max => IrMethod::Max,
            Method::Clamp => IrMethod::Clamp,
            Method::Floor => IrMethod::Floor,
            Method::Sin => IrMethod::Sin,
            Method::Cos => IrMethod::Cos,
            Method::Tan => IrMethod::Tan,
            Method::Asin => IrMethod::Asin,
            Method::Acos => IrMethod::Acos,
            Method::Atan => IrMethod::Atan,
            Method::Atan2 => IrMethod::Atan2,
            Method::Sinh => IrMethod::Sinh,
            Method::Cosh => IrMethod::Cosh,
            Method::Tanh => IrMethod::Tanh,
            Method::Powf => IrMethod::Powf,
            Method::Powi => IrMethod::Powi,
            Method::Log10 => IrMethod::Log10,
            Method::Log2 => IrMethod::Log2,
            Method::Ceil => IrMethod::Ceil,
            Method::Round => IrMethod::Round,
            Method::Trunc => IrMethod::Trunc,
            Method::Fract => IrMethod::Fract,
            Method::Signum => IrMethod::Signum,
            Method::RemEuclid => IrMethod::RemEuclid,
            Method::Hypot => IrMethod::Hypot,
            Method::IsNan => IrMethod::IsNan,
            Method::IsFinite => IrMethod::IsFinite,
            Method::IsInfinite => IrMethod::IsInfinite,
            Method::IntAbs => IrMethod::IntAbs,
            Method::Pow => IrMethod::Pow,
            Method::SaturatingAdd => IrMethod::SaturatingAdd,
            Method::SaturatingSub => IrMethod::SaturatingSub,
            Method::SaturatingMul => IrMethod::SaturatingMul,
            _ => return None,
        })
    }

    /// The IR type of a type of the subset; `None` when no value has it in the IR.
    pub(super) fn value_type(&self, ty: &Ty) -> Option<ValueType> {
        ValueTypes { design: self.design }.of(ty)
    }

    /// Add an expression to the IR, remembering its type (the builder does not hand it back) and
    /// whether it is continuous (`events.rs`).
    pub(super) fn emit(&mut self, kind: Expr, ty: ValueType, origin: Origin) -> LoweredExpr {
        self.expr_types.push(ty.clone());
        let continuous = self.continuous_kind(&kind);
        self.expr_continuous.push(continuous);
        self.expr_origins.push(origin.clone());
        self.ir.expr(kind, ty, origin)
    }

    /// Add a statement to the IR, remembering whether it reads a continuous value.
    pub(super) fn stmt(&mut self, stmt: Stmt) -> LoweredStmt {
        let continuous = stmt.operands().iter().any(|e| self.continuous(*e));
        self.stmt_continuous.push(continuous);
        self.ir.stmt(stmt)
    }

    /// The type of an expression already added.
    pub(super) fn type_of(&self, expr: LoweredExpr) -> ValueType {
        self.expr_types[expr.index()].clone()
    }

    /// The origin of an expression: where its construct starts.
    pub(super) fn expr_origin(&self, expr: ExprId) -> Origin {
        let node = self.design.expr_node(expr);
        self.locator.origin(self.design.decl_data(node.owner).file, node.span)
    }

    /// `expr` lowered, or `None` after the diagnostics of what could not be.
    pub(super) fn lower_expr(&mut self, scope: &mut Scope, expr: ExprId) -> Option<LoweredExpr> {
        let node = self.design.expr_node(expr).clone();
        let file = self.design.decl_data(node.owner).file;
        let origin = self.expr_origin(expr);
        let ty = self.types.ty(expr).and_then(|t| self.value_type(t));
        let Some(ty) = ty else {
            let what = match self.types.ty(expr) {
                Some(Ty::Enum(_)) => "a value of an enum of one variant, whose tag has no level",
                _ => "an expression the type checker gave no value type",
            };
            self.unsupported(file, node.span, what);
            return None;
        };
        if ty.is_integer() && self.constant_form(expr) {
            return match self.consts.folded(expr, &Bindings::new()) {
                Ok(Folded::Known(value)) => Some(self.constant_expr(value, ty, origin)),
                Ok(Folded::Unknown) => Some(self.unknown_expr(ty, origin)),
                Err(fault) => {
                    self.const_fault(&fault, "a constant expression");
                    None
                }
            };
        }
        match &node.kind {
            ExprKind::Lit(lit) if matches!(lit.kind, LitKind::Unknown) => Some(self.unknown_expr(ty, origin)),
            ExprKind::Lit(_) => match self.consts.value(expr, &Bindings::new()) {
                Ok(value) => Some(self.constant_expr(value, ty, origin)),
                Err(fault) => {
                    self.const_fault(&fault, "a literal");
                    None
                }
            },
            ExprKind::Paren(inner) => self.lower_expr(scope, *inner),
            ExprKind::Path(_) => self.lower_path(scope, expr, ty, origin),
            ExprKind::Field { .. } => self.lower_field(scope, expr, ty, origin),
            ExprKind::Unary { op, expr: operand } => {
                let op = match op {
                    UnOp::Neg => UnaryOp::Neg,
                    UnOp::Not => UnaryOp::Not,
                    UnOp::Deref | UnOp::Ref => {
                        self.unsupported(file, node.span, "a reference or a dereference");
                        return None;
                    }
                };
                let operand = self.lower_expr(scope, *operand)?;
                let ty = self.flag(operand).wrap(ty);
                Some(self.emit(Expr::Unary { op, expr: operand }, ty, origin))
            }
            ExprKind::Binary { op, lhs, rhs } => {
                // The right operand of `&&`/`||` runs only when the left does not decide: it is
                // conditional exactly like an `if` arm.
                let short_circuit = matches!(op, BinOp::And | BinOp::Or);
                let right = *rhs;
                let left = *lhs;
                let lhs = self.lower_expr(scope, *lhs);
                // `a && b` runs `b` when `a` holds, `a || b` when it does not.
                if short_circuit {
                    scope.guards.push(ArmCondition::If { condition: left, holds: matches!(op, BinOp::And) });
                }
                let rhs = self.lower_expr(scope, *rhs);
                if short_circuit {
                    scope.guards.pop();
                }
                let (lhs, rhs) = (lhs?, rhs?);
                let flag = self.flags(&[lhs, rhs]);
                let op = match (Self::binary_op(*op), flag) {
                    // §3.3.12: over a value that may be unknown, `&&` and `||` give the table of
                    // `&` and `|`; the IR's short-circuit forms take known operands only. A right
                    // operand that can panic or emit must not run whatever the left gives, so it
                    // is an `if` and the generated code merges it (§3.3.3, §1 rule 13).
                    (op @ (BinaryOp::And | BinaryOp::Or), Flag::Flagged) => {
                        if self.partial(right).is_some() {
                            return Some(self.lower_short_circuit_as_if(op == BinaryOp::And, lhs, rhs, ty, origin));
                        }
                        if op == BinaryOp::And { BinaryOp::BitAnd } else { BinaryOp::BitOr }
                    }
                    (op, _) => op,
                };
                Some(self.emit(Expr::Binary { op, lhs, rhs }, flag.wrap(ty), origin))
            }
            ExprKind::If { cond, then, else_ } if matches!(self.design.expr_node(*cond).kind, ExprKind::Let { .. }) => self.lower_if_let(scope, *cond, then, *else_, ty, origin),
            // A condition that reads only key params and constants is folded: only the arm it
            // selects is lowered (`structural.rs`, §11 rule 3).
            ExprKind::If { cond, then, else_ } if let Some(holds) = self.folded_condition(&scope.keys.clone(), &self.placement_chain(scope.frame), *cond) => {
                if holds {
                    return self.lower_block(scope, file, &then.stmts, node.span);
                }
                match else_ {
                    Some(else_) => self.lower_expr(scope, *else_),
                    None => Some(self.emit(Expr::Unit, ValueType::Unit, origin)),
                }
            }
            ExprKind::If { cond, then, else_ } => {
                let source = *cond;
                let cond = self.lower_expr(scope, *cond);
                let arm = Self::enter_arm(scope, cond, true);
                scope.guards.push(ArmCondition::If { condition: source, holds: true });
                let then = self.lower_block(scope, file, &then.stmts, node.span);
                scope.guards.pop();
                Self::leave_arm(scope, arm);
                let arm = Self::enter_arm(scope, cond, false);
                scope.guards.push(ArmCondition::If { condition: source, holds: false });
                let otherwise = match else_ {
                    Some(else_) => self.lower_expr(scope, *else_),
                    None => Some(self.emit(Expr::Unit, ValueType::Unit, origin.clone())),
                };
                scope.guards.pop();
                Self::leave_arm(scope, arm);
                let (cond, then, otherwise) = (cond?, then?, otherwise?);
                // A condition that may be unknown is kept: the generated code decides it.
                let ty = self.flags(&[cond, then, otherwise]).wrap(ty);
                Some(self.emit(Expr::If { cond, then, otherwise }, ty, origin))
            }
            ExprKind::Block(block) => {
                scope.block_depth += 1;
                let lowered = self.lower_block(scope, file, &block.stmts, node.span);
                scope.block_depth -= 1;
                lowered
            }
            ExprKind::Call { args, class, .. } => self.lower_call(scope, expr, &args.iter().map(|a| a.value).collect::<Vec<_>>(), *class, ty, origin),
            ExprKind::MethodCall { recv, args, .. } if self.types.builtin(expr).is_some() => {
                let builtin = self.types.builtin(expr)?;
                let args: Vec<ExprId> = args.iter().map(|a| a.value).collect();
                self.lower_flag_read(scope, expr, builtin, *recv, &args, ty, origin)
            }
            ExprKind::MethodCall { recv, args, .. } => {
                let Some(method) = self.types.method(expr) else {
                    self.unsupported(file, node.span, "a method outside the catalogue");
                    return None;
                };
                let f = match method {
                    Method::Exp => MathFn::Exp,
                    Method::Ln => MathFn::Ln,
                    Method::Sqrt => MathFn::Sqrt,
                    Method::Abs => MathFn::Abs,
                    row if Self::ir_method(row).is_some() => {
                        let method = Self::ir_method(row)?;
                        let receiver = self.lower_expr(scope, *recv);
                        let mut lowered = Vec::new();
                        for arg in args {
                            lowered.push(self.lower_expr(scope, arg.value));
                        }
                        let (receiver, args) = (receiver?, lowered.into_iter().collect::<Option<Vec<_>>>()?);
                        let ty = self.flags(&args).or(self.flag(receiver)).wrap(ty);
                        return Some(self.emit(Expr::Method { method, receiver, args }, ty, origin));
                    }
                    // Lowered as a flag and a payload from msl-digital T27 on.
                    Method::IsSome | Method::IsNone | Method::UnwrapOr | Method::Unwrap | Method::Expect | Method::UnwrapOrElse => {
                        let args: Vec<ExprId> = args.iter().map(|a| a.value).collect();
                        return self.lower_option_method(scope, method, *recv, &args, ty, origin);
                    }
                    // The rest are `Option`s (`checked_*`, `map`, `and_then`, `or`, `then`, `then_some`): they lower
                    // on their pair, never as one value.
                    _ => {
                        self.unsupported(file, node.span, &format!("a `{method:?}` row read as a value rather than as an `Option`"));
                        return None;
                    }
                };
                let recv = self.lower_expr(scope, *recv)?;
                // §3.3.2: one unknown operand makes the result unknown.
                let ty = self.flag(recv).wrap(ty);
                Some(self.emit(Expr::Math { f, args: vec![recv] }, ty, origin))
            }
            ExprKind::Cast { expr: operand, .. } => {
                let operand = self.lower_expr(scope, *operand)?;
                let flagged = self.flag(operand).wrap(ty.clone());
                Some(self.emit(Expr::Cast { expr: operand, to: ty }, flagged, origin))
            }
            // A scrutinee over the key params folds: only the arm it selects is lowered (`structural.rs`).
            ExprKind::Match { scrutinee, arms } if let Some(selected) = self.folded_arm(&scope.keys.clone(), &self.placement_chain(scope.frame), *scrutinee, arms) => self.lower_expr(scope, arms[selected].body),
            ExprKind::Match { scrutinee, arms } => self.lower_match(scope, *scrutinee, arms, ty, origin),
            ExprKind::Closure { params, body, .. } => {
                let mut locals = Vec::new();
                for param in params {
                    match self.design.pat_node(param.pat).kind.clone() {
                        PatKind::Ident { binding, sub: None, .. } => locals.push(self.bind_local(scope, binding)?),
                        _ => {
                            self.unsupported(file, node.span, "a closure parameter that is not a name");
                            return None;
                        }
                    }
                }
                scope.nesting += 1;
                let body = self.lower_expr(scope, *body);
                scope.nesting -= 1;
                Some(self.emit(Expr::Closure { params: locals, body: body? }, ty, origin))
            }
            _ => {
                self.unsupported(file, node.span, "this expression form");
                None
            }
        }
    }

    /// A call: of a function of the reachable set (one `Call` node, the function lowered once
    /// however many bodies call it), or of a core primitive or a probe (see `analog.rs`).
    fn lower_call(&mut self, scope: &mut Scope, expr: ExprId, args: &[ExprId], class: CallClass, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let span = self.design.expr_node(expr).span;
        match (class, self.types.callee(expr)) {
            (CallClass::Probe, _) => self.lower_probe(scope, expr, ty, origin),
            // A function that emits is no IR function: its call is inlined where it stands (§19 rule 10).
            (_, Some(Callee::Function(function))) if self.types.emits(function).is_some() => self.inline_emitting_value(scope, function, args, origin),
            (_, Some(Callee::Function(function))) => {
                let lowered = self.lower_arguments(scope, expr, function)?;
                let pattern: Vec<Flag> = lowered.iter().map(|a| self.flag(*a)).collect();
                let (id, ret) = self.request(function, pattern, scope.file, span)?;
                Some(self.emit(Expr::Call { function: id, args: lowered }, ret, origin))
            }
            (_, Some(Callee::Core(id))) => self.lower_core(scope, expr, id, args, ty, origin),
            // A module with a result is hoisted: its result is read where the placement stands.
            (_, Some(Callee::Module(_))) if self.result_callee(expr).is_some() => self.lower_result(scope, expr),
            (_, Some(Callee::Module(_))) => {
                self.unsupported(scope.file, span, "a placement used as a value");
                None
            }
            // Typed by `types::intrinsic`, lowered by its §22.1 contract (`intrinsics.rs`).
            (_, Some(Callee::Intrinsic(intrinsic))) => self.lower_intrinsic(scope, expr, intrinsic, args, ty, origin),
            (_, None) => {
                self.unsupported(scope.file, span, "a call the type checker did not resolve");
                None
            }
        }
    }

    /// The arguments of the call `expr` of `function`, one IR argument per parameter in parameter
    /// order, from the checker's binding of the call (§3.2): an `Option<T>` parameter takes two,
    /// its flag and its payload, a parameter left out `(false, zero)`. `None` after a diagnostic.
    fn lower_arguments(&mut self, scope: &mut Scope, expr: ExprId, function: DeclId) -> Option<Vec<LoweredExpr>> {
        let Some(binding) = self.types.binding(expr).map(<[_]>::to_vec) else {
            self.unsupported(scope.file, self.design.expr_node(expr).span, "a call whose arguments the type checker did not bind");
            return None;
        };
        let params: Vec<DeclId> = self.design.decl_data(function).children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::FnParam(_))).collect();
        let mut lowered = Vec::new();
        let mut ok = true;
        for (param, slot) in params.iter().zip(binding) {
            match self.types.local(*param).cloned() {
                Some(Ty::Option(payload)) => match self.lower_option_slot(scope, slot, &payload, expr) {
                    Some(pair) => lowered.extend(pair),
                    None => ok = false,
                },
                _ => match slot.and_then(|arg| self.lower_expr(scope, arg)) {
                    Some(arg) => lowered.push(arg),
                    None => ok = false,
                },
            }
        }
        ok.then_some(lowered)
    }

    /// `expr` without the parentheses around it.
    pub(super) fn peel_parens(&self, mut expr: ExprId) -> ExprId {
        while let ExprKind::Paren(inner) = &self.design.expr_node(expr).kind {
            expr = *inner;
        }
        expr
    }

    /// Allocate the local a `Binding` declaration names: its name and its type, in declaration
    /// order. `None` after a diagnostic when the checker typed it with a non-value type.
    pub(super) fn bind_local(&mut self, scope: &mut Scope, binding: DeclId) -> Option<u32> {
        let data = self.design.decl_data(binding);
        let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let (file, span) = (data.file, data.span);
        let Some(ty) = self.types.local(binding).and_then(|t| self.value_type(t)) else {
            self.unsupported(file, span, "a name whose type is not a value type");
            return None;
        };
        let index = scope.locals.len() as u32;
        scope.locals.push(Local { name, ty, mutable: false });
        scope.local_index.insert(binding, index);
        Some(index)
    }

    /// The statements of a block as one expression: a block with statements is a `Block`, one
    /// that is only its value is that value (`{ x }` is `x`).
    pub(super) fn lower_block(&mut self, scope: &mut Scope, file: FileId, stmts: &[StmtId], span: Span) -> Option<LoweredExpr> {
        let origin = self.locator.origin(file, span);
        let (stmts, value) = self.lower_statements(scope, stmts)?;
        match (stmts.is_empty(), value) {
            (true, Some(value)) => Some(value),
            (_, value) => {
                let ty = value.map_or(ValueType::Unit, |v| self.type_of(v));
                Some(self.emit(Expr::Block { stmts, value }, ty, origin))
            }
        }
    }

    /// The statements of a block in order and its value (the last expression without `;`).
    /// `None` after the diagnostics of every statement that could not be lowered.
    pub(super) fn lower_statements(&mut self, scope: &mut Scope, stmts: &[StmtId]) -> Option<(Vec<LoweredStmt>, Option<LoweredExpr>)> {
        let mut out = Vec::new();
        let mut value = None;
        let mut ok = true;
        for (i, stmt) in stmts.iter().enumerate() {
            let node = self.design.stmt_node(*stmt).clone();
            let file = self.design.decl_data(node.owner).file;
            match &node.kind {
                StmtKind::Let { pat, init: Some(init), else_: None, .. } => ok &= self.lower_let(scope, &mut out, *pat, *init, file, node.span),
                // `emit!` and a call of a function that emits are statements (`records.rs`).
                StmtKind::Expr { expr, .. } if let Some(lowered) = self.lower_record_statement(scope, &mut out, *expr) => ok &= lowered,
                // `bound_step(dt);` is a statement of the analog block (`analog.rs`).
                StmtKind::Expr { expr, .. } if let Some(lowered) = self.lower_bound_step(scope, &mut out, *expr) => ok &= lowered,
                StmtKind::Expr { expr, semi } => match self.lower_expr(scope, *expr) {
                    Some(lowered) if i + 1 == stmts.len() && !semi => value = Some(lowered),
                    Some(lowered) => out.push(self.stmt(Stmt::Expr(lowered))),
                    None => ok = false,
                },
                StmtKind::Empty => {}
                StmtKind::Contribution { lhs, rhs } => ok &= self.lower_contribution(scope, &mut out, *lhs, *rhs, node.span),
                StmtKind::Commit { place, value, after } => ok &= self.lower_commit(scope, &mut out, *place, *value, *after),
                _ => {
                    self.unsupported(file, node.span, "this statement");
                    ok = false;
                }
            }
        }
        ok.then_some((out, value))
    }

    /// `let pat = init;`: a name, an `Option` (a flag local and a payload local) or a tuple
    /// (one local per component). `false` after a diagnostic.
    fn lower_let(&mut self, scope: &mut Scope, out: &mut Vec<LoweredStmt>, pat: crate::pom::PatId, init: ExprId, file: FileId, span: Span) -> bool {
        match self.design.pat_node(pat).kind.clone() {
            PatKind::Ident { binding, sub: None, .. } if matches!(self.types.local(binding), Some(Ty::Option(_))) => self.lower_option_let(scope, out, binding, init),
            PatKind::Ident { binding, sub: None, .. } => {
                let local = self.bind_local(scope, binding);
                scope.let_sources.insert(binding, init);
                let name = self.design.decl_data(binding).name.as_ref().map(|n| n.text.clone());
                scope.limit_name = name.map(|n| (self.peel_parens(init), n));
                let init = self.lower_expr(scope, init);
                scope.limit_name = None;
                match (local, init) {
                    (Some(local), Some(init)) => {
                        // A local is what its initializer is: flagged when that may be unknown.
                        scope.locals[local as usize].ty = self.type_of(init);
                        scope.local_inits.insert(local, init);
                        out.push(self.stmt(Stmt::Let { local, init }));
                        true
                    }
                    _ => false,
                }
            }
            PatKind::Tuple(pats) => self.lower_tuple_let(scope, out, &pats, init, file, span),
            _ => {
                self.unsupported(file, span, "a `let` pattern that is not a name");
                false
            }
        }
    }

    fn binary_op(op: BinOp) -> BinaryOp {
        match op {
            BinOp::Add => BinaryOp::Add,
            BinOp::Sub => BinaryOp::Sub,
            BinOp::Mul => BinaryOp::Mul,
            BinOp::Div => BinaryOp::Div,
            BinOp::Lt => BinaryOp::Lt,
            BinOp::Le => BinaryOp::Le,
            BinOp::Gt => BinaryOp::Gt,
            BinOp::Ge => BinaryOp::Ge,
            BinOp::Eq => BinaryOp::Eq,
            BinOp::Ne => BinaryOp::Ne,
            BinOp::And => BinaryOp::And,
            BinOp::Or => BinaryOp::Or,
            BinOp::Rem => BinaryOp::Rem,
            BinOp::BitXor => BinaryOp::BitXor,
            BinOp::BitAnd => BinaryOp::BitAnd,
            BinOp::BitOr => BinaryOp::BitOr,
            BinOp::Shl => BinaryOp::Shl,
            BinOp::Shr => BinaryOp::Shr,
        }
    }

    /// Whether `expr` reads only literals and constants through operators, casts and the
    /// catalogue's methods: an expression `ConstEval` folds (DIG-08 AC 2). A call, a block or an
    /// `if` is kept as written.
    fn constant_form(&self, expr: ExprId) -> bool {
        match &self.design.expr_node(expr).kind {
            ExprKind::Lit(lit) => matches!(lit.kind, LitKind::Int(_) | LitKind::Float { .. } | LitKind::Bool(_)),
            ExprKind::Paren(inner) => self.constant_form(*inner),
            ExprKind::Path(path) => matches!(self.design.path_node(*path).segments.last().map(|s| s.res), Some(Resolution::Decl(d)) if matches!(self.design.decl_data(d).kind, DeclKind::Const(_) | DeclKind::Variant(_))),
            ExprKind::Unary { op: UnOp::Neg | UnOp::Not, expr: operand } => self.constant_form(*operand),
            ExprKind::Binary { lhs, rhs, .. } => self.constant_form(*lhs) && self.constant_form(*rhs),
            ExprKind::Cast { expr: operand, .. } => self.constant_form(*operand),
            ExprKind::MethodCall { recv, args, .. } => self.types.method(expr).is_some() && self.constant_form(*recv) && args.iter().all(|a| self.constant_form(a.value)),
            _ => false,
        }
    }

    /// A folded value as the IR expression of its type; an integer takes `ty`, its expression's
    /// value type.
    pub(super) fn constant_expr(&mut self, value: Constant, ty: ValueType, origin: Origin) -> LoweredExpr {
        match value {
            Constant::Real(r) => self.emit(Expr::Real(r), ValueType::F64, origin),
            Constant::Bool(b) => self.emit(Expr::Bool(b), ValueType::Bool, origin),
            Constant::Int(v) => self.emit(Expr::Int(v), ty, origin),
        }
    }

    /// The literal `Unknown` of type `ty`: a value with every level unknown, so flagged (§3.3.1).
    pub(super) fn unknown_expr(&mut self, ty: ValueType, origin: Origin) -> LoweredExpr {
        self.emit(Expr::Unknown, Flag::Flagged.wrap(ty), origin)
    }

    /// A name: a `const` (folded), a param of the body (a read), a local.
    fn lower_path(&mut self, scope: &mut Scope, expr: ExprId, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let node = self.design.expr_node(expr).clone();
        let ExprKind::Path(path) = &node.kind else { return None };
        let res = self.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res);
        let Resolution::Decl(decl) = res else {
            self.unsupported(scope.file, node.span, "a built-in name used as a value");
            return None;
        };
        match &self.design.decl_data(decl).kind {
            DeclKind::Const(_) => match self.consts.folded_constant(decl) {
                Ok(Folded::Known(value)) => Some(self.constant_expr(value, ty, origin)),
                Ok(Folded::Unknown) => Some(self.unknown_expr(ty, origin)),
                Err(fault) => {
                    self.const_fault(&fault, "a `const`");
                    None
                }
            },
            // A unit variant is its tag (§2 rule 5).
            DeclKind::Variant(_) => match EnumLayout::variant_tag(self.design, decl) {
                Some(tag) => Some(self.emit(Expr::Int(tag), ty, origin)),
                None => {
                    self.unsupported(scope.file, node.span, "a variant outside its enum");
                    None
                }
            },
            DeclKind::Param(_) => match (scope.params.get(&decl), self.frames.get(scope.frame).and_then(|f| f.params.get(&decl)).copied()) {
                (Some(index), _) if scope.frame == 0 => Some(self.emit(Expr::Param(*index), ty, origin)),
                // A param of a hoisted module is the constant its placement bound (`hoist.rs`).
                (_, Some(value)) => Some(self.constant_expr(value, ty, origin)),
                _ => {
                    self.unsupported(scope.file, node.span, "a param of another module");
                    None
                }
            },
            DeclKind::Port(_) | DeclKind::Signal(_) => match (self.frame_carrier(scope.frame, decl), self.input_of(scope, decl)) {
                // A read of a carrier may be unknown (§3.3): its type is flagged.
                (Some(index), _) => Some(self.carrier_read(index, origin)),
                // A value input of a hoisted module: the caller's expression, in the caller's frame.
                (None, Some(Some(given))) => self.in_caller(scope, |this, scope| this.lower_expr(scope, given)),
                (None, Some(None)) => {
                    self.unsupported(scope.file, node.span, "a value input left out, read as a value");
                    None
                }
                (None, None) => {
                    self.unsupported(scope.file, node.span, "a node or a port read as a value");
                    None
                }
            },
            DeclKind::FnParam(_) | DeclKind::Binding(_) => match scope.local_index.get(&decl) {
                Some(index) => {
                    let ty = scope.locals[*index as usize].ty.clone();
                    Some(self.emit(Expr::Local(*index), ty, origin))
                }
                // A module-body `let`: inlined at each read (Appendix C.1.1, `hoist.rs`).
                None if self.body_let_value(decl).is_some() => {
                    let value = self.body_let_value(decl)?;
                    self.lower_body_let(scope, value)
                }
                // A block-local `let` in a standalone scope is its source expression (`derivatives.rs`).
                None if scope.no_locals && scope.let_sources.contains_key(&decl) => {
                    let source = scope.let_sources[&decl];
                    self.lower_expr(scope, source)
                }
                None => {
                    self.unsupported(scope.file, node.span, "a name with no value here");
                    None
                }
            },
            _ => {
                self.unsupported(scope.file, node.span, "this name as a value");
                None
            }
        }
    }
}
