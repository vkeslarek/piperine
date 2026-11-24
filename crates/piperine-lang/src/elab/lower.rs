//! Value code: POM expressions lowered to the IR node of the same form.
//!
//! One `Scope` per owner (a module body, later a function) names the params, nodes and locals
//! an expression may read. A `const` read becomes its folded value; a param read stays a
//! `Param` read, never a number (a host `set` must reach the solver without re-elaboration).
//! An integer expression that reads only literals and constants is folded to one literal, by
//! the rules the compiled code follows (`ConstEval`).

use std::collections::BTreeMap;

use piperine_ir::{BinaryOp, Branch, Condition, Constant, Expr, ExprId as LoweredExpr, Local, MathFn, Method as IrMethod, Origin, Site, Stmt, StmtId as LoweredStmt, Terminal, UnaryOp, ValueType};

use super::elements::ElementPart;
use super::compound::EnumLayout;
use super::control::LoopFrame;
use super::returns::FunctionResult;
use super::variants::Half;
use super::consts::Bindings;
use super::unroll::Expansion;
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
    /// The handle parameters of the functions inlined at a call (`handles.rs`), each bound to the caller's argument: the
    /// name of a port or a signal of the frame, or `gnd`.
    pub(super) handles: BTreeMap<DeclId, ExprId>,
    /// Code with no local of its own (a drive): a `match` reads its scrutinee again at each test.
    pub(super) no_locals: bool,
    /// Code a core primitive evaluates at a converged solution (a `noise` density, a `delay` input): a `limit` there is
    /// its argument, since limiting changes no iterate that has stopped moving (§14.7.4).
    pub(super) at_solution: bool,
    /// Code a primitive evaluates on its own, outside every statement of the `analog` block (a `noise` density, a
    /// `delay` input, a `ddt` state): an `ac_stim` read there is also a statement of the block, which is where the
    /// stimulus site is written (`analog.rs`).
    pub(super) value_input: bool,
    /// Code that is differentiated symbolically (a `ddx` value): a `fn` called there is expanded where it stands
    /// (`expansion.rs`), not called.
    pub(super) differentiated: bool,
    /// The branches whose flow a differentiated value reads as a leaf though the module gives them no potential
    /// contribution (`analog.rs`): the derivative may not read their value.
    pub(super) flow_leaves: Vec<u32>,
    /// The parameters and `let`s of the `fn` being expanded in a differentiated value, each the lowered expression it holds
    /// (`expansion.rs`).
    pub(super) values: BTreeMap<DeclId, LoweredExpr>,
    /// The `Option` parameters and `let`s of the `fn` being expanded: the flag and the payload each holds.
    pub(super) option_values: BTreeMap<DeclId, (LoweredExpr, LoweredExpr)>,
    /// The forks the `fn` being expanded has made so far (`symbolic.rs`).
    pub(super) forks: u32,
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
    /// The loops the code being lowered sits in, innermost last (`control.rs`).
    pub(super) loops: Vec<LoopFrame>,
    /// What a `return` of the function being lowered gives (`returns.rs`); `None` outside a function
    /// whose body returns early.
    pub(super) result: Option<FunctionResult>,
    /// The constant each loop variable of the unrolled analog loops the code sits in, and each
    /// parameter of the unrolled recursive calls it sits in, stands for (`unroll.rs`).
    pub(super) unrolled: Bindings,
    /// The generation `for`s the code being lowered sits in and the ordinal of the iteration in each (`generation.rs`).
    pub(super) ordinals: Vec<(DeclId, u32)>,
    /// Whether the code being lowered is a statement of an `analog` block, not a function's body. The analog side reads
    /// constant elements of an array of carriers only, each through its mirror (`elements.rs`), so an array read whole
    /// or at a run-time element is read there as the array of its elements.
    pub(super) in_analog_block: bool,
    /// How many `if` and `match` decisions that did not fold the code being lowered sits under.
    pub(super) decisions: u32,
    /// The recursive calls being expanded, outermost first (`unroll.rs`).
    pub(super) expanding: Vec<Expansion>,
    /// The `let`s of the temporaries an inlined `Option` call reads (`returns.rs`), declared before the
    /// statement being lowered so every read of the pair sees them bound.
    pub(super) pending: Vec<LoweredStmt>,
    /// The inlined `Option` functions being lowered that leave early, innermost last (`returns.rs`).
    pub(super) inline_exits: Vec<super::returns::InlineExit>,
}

impl Scope {
    /// Takes over what a standalone scope made from a copy of this one's (`standalone_scope`) added: the `ac_stim` sites
    /// and the statements that write them, the branches a flow read numbered, and the flow leaves. So a stimulus or a
    /// flow read inside a value a primitive evaluates on its own (a `noise` density, a `delay` input, a `ddt` state, a
    /// `ddx` value) is a site or a branch of the body. The `limit` sites it made stay its own: a `limit` there is read
    /// at a solution (`at_solution`) or differentiated through, never evaluated.
    pub(super) fn adopt_standalone(&mut self, standalone: &mut Scope) {
        self.stims = standalone.stims.clone();
        self.branches = standalone.branches.clone();
        self.flow_leaves.append(&mut standalone.flow_leaves);
        self.hoisted_steps.append(&mut standalone.hoisted_steps);
    }

    pub(super) fn new(file: FileId) -> Self {
        Scope { file, params: BTreeMap::new(), locals: Vec::new(), local_index: BTreeMap::new(), limits: Vec::new(), stims: Vec::new(), branches: Vec::new(), nesting: 0, guards: Vec::new(), hoisted_steps: Vec::new(), block_depth: 0, limit_name: None, options: BTreeMap::new(), frame: 0, texts: BTreeMap::new(), handles: BTreeMap::new(), no_locals: false, at_solution: false, value_input: false, differentiated: false, flow_leaves: Vec::new(), values: BTreeMap::new(), option_values: BTreeMap::new(), forks: 0, analog: false, bounds: 0, let_sources: BTreeMap::new(), addend_lets: std::collections::BTreeSet::new(), keys: Bindings::new(), merged: Vec::new(), conditions: Vec::new(), contributions: Vec::new(), local_inits: BTreeMap::new(), loops: Vec::new(), result: None, unrolled: Bindings::new(), ordinals: Vec::new(), in_analog_block: false, decisions: 0, expanding: Vec::new(), pending: Vec::new(), inline_exits: Vec::new() }
    }
}

/// The IR type of a type of the subset.
pub(super) struct ValueTypes<'a> {
    pub(super) arena: &'a super::types::TypeArena,
}

impl ValueTypes<'_> {
    /// The IR type of an `Option` held as one value (`ValueType::Option`) where it is an element of an array or an item of a
    /// tuple, as it is a field of a struct; `None` for any other type. An `Option` of value code is the pair of its halves.
    fn packed_option(&self, ty: &Ty) -> Option<ValueType> {
        match ty {
            Ty::Option(inner) => self.of(inner).or_else(|| self.packed_option(inner)).map(|payload| ValueType::Option(Box::new(payload))),
            _ => None,
        }
    }

    /// `None` for a type no value has in the IR: a node, a module, an `Option` (a pair of values,
    /// msl-digital T27).
    pub(super) fn of(&self, ty: &Ty) -> Option<ValueType> {
        match ty {
            Ty::F64 => Some(ValueType::F64),
            Ty::F32 => Some(ValueType::F32),
            Ty::F16 => Some(ValueType::F16),
            Ty::F128 => Some(ValueType::F128),
            Ty::Bool => Some(ValueType::Bool),
            Ty::Unit => Some(ValueType::Unit),
            Ty::UInt(n) => Some(ValueType::UInt(*n)),
            Ty::Int(n) => Some(ValueType::Int(*n)),
            Ty::Fn(params, ret) => {
                let params: Option<Vec<ValueType>> = params.iter().map(|p| self.of(p)).collect();
                Some(ValueType::Fn(params?, Box::new(self.of(ret)?)))
            }
            // A struct, an enum, a tuple and an array are the arena's entries and their composition.
            Ty::Struct(_) | Ty::Enum(_) | Ty::Tuple(_) | Ty::Array(..) => self.arena.of(ty, |part| self.of(part).or_else(|| self.packed_option(part))),
            Ty::Node(_) | Ty::AnyNode | Ty::Module(_) | Ty::Option(_) | Ty::Carrier { .. } | Ty::Event | Ty::Str | Ty::Never | Ty::Range { .. } | Ty::Iterator { .. } => None,
            // the elaborator reads types through the substitution (T32): a parameter never reaches the IR
            Ty::Param(_) | Ty::GenericInteger { .. } => None,
        }
    }
}

impl Elaboration<'_, '_> {
    /// The type the checker gave `expr`, with the generic parameters of the instance being lowered
    /// replaced by their arguments: the one read of the type table of an expression (`ConstEval::typed`).
    pub(super) fn typed(&self, expr: ExprId) -> Option<Ty> {
        self.consts.typed(expr).map(|ty| self.with_hand_written_widths(&ty))
    }

    /// The type of the binding `binding` under the substitution.
    pub(super) fn typed_local(&self, binding: DeclId) -> Option<Ty> {
        self.consts.typed_local(binding).map(|ty| self.with_hand_written_widths(&ty))
    }

    /// The type the type node `ty` writes, under the substitution.
    pub(super) fn typed_written(&self, ty: crate::pom::TypeId) -> Option<Ty> {
        self.consts.typed_written(ty).map(|ty| self.with_hand_written_widths(&ty))
    }

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
        ValueTypes { arena: &self.type_arena }.of(ty)
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
        let id = self.ir.stmt(stmt.clone());
        self.divergences.note_statement(id, &stmt);
        id
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

    /// `expr` lowered, or `None` after the diagnostics of what could not be. An expression that fails while nothing
    /// has been reported is refused by name here, so no position (a statement, a definition, an event expression, a
    /// value input) can drop it silently.
    pub(super) fn lower_expr(&mut self, scope: &mut Scope, expr: ExprId) -> Option<LoweredExpr> {
        let lowered = self.lower_expr_unchecked(scope, expr);
        if lowered.is_none() && self.diagnostics.is_empty() {
            self.refuse_silent_failure(scope.file, self.design.expr_node(expr).span, "this expression");
        }
        lowered
    }

    /// `P0001` for something that failed to lower while no diagnostic was reported: the one place the elaborator says
    /// "nothing vanishes silently".
    pub(super) fn refuse_silent_failure(&mut self, file: FileId, span: Span, what: &str) {
        self.unsupported(file, span, what);
    }

    fn lower_expr_unchecked(&mut self, scope: &mut Scope, expr: ExprId) -> Option<LoweredExpr> {
        let node = self.design.expr_node(expr).clone();
        let file = self.design.decl_data(node.owner).file;
        let origin = self.expr_origin(expr);
        // `!` is no value: a diverging expression is `()` until its position gives it a type (`exits.rs`).
        let ty = match self.typed(expr) {
            Some(Ty::Never) => Some(ValueType::Unit),
            other => other.and_then(|t| self.value_type(&t)),
        };
        let Some(ty) = ty else {
            self.unsupported(file, node.span, "an expression the type checker gave no value type");
            return None;
        };
        // An event read as a `bool` is `fired(k)` (§13 rules 2 and 9, `event_reads.rs`).
        if self.types.event_read(expr) {
            return self.lower_event_read(scope, expr);
        }
        if ty.is_integer() && self.constant_form(expr) {
            return match self.consts.value(expr, &Bindings::new()) {
                Ok(value) => Some(self.constant_expr(value, ty, origin)),
                Err(fault) => {
                    self.const_fault(&fault, "a constant expression");
                    None
                }
            };
        }
        match &node.kind {
            ExprKind::Lit(_) => match self.consts.value(expr, &Bindings::new()) {
                Ok(value) => Some(self.constant_expr(value, ty, origin)),
                Err(fault) => {
                    self.const_fault(&fault, "a literal");
                    None
                }
            },
            ExprKind::Paren(inner) => self.lower_expr(scope, *inner),
            ExprKind::Path(_) => self.lower_path(scope, expr, ty, origin),
            ExprKind::Field { expr: recv, name, .. } if matches!(self.typed(*recv), Some(Ty::Struct(_) | Ty::Tuple(_))) => self.lower_field_value(scope, *recv, &name.text, ty, origin),
            // a field of a bundle port or signal is the carrier its leaf stands for (§7 rule 12)
            ExprKind::Field { .. } if let Some(leaf) = self.design.bundle_field(expr) => self.lower_leaf_read(scope, expr, leaf, origin),
            ExprKind::Field { .. } => self.lower_field(scope, expr, ty, origin),
            ExprKind::Struct { path, fields, .. } => self.lower_struct_expr(scope, expr, *path, fields, ty, origin),
            ExprKind::Tuple(items) => self.lower_tuple(scope, items, ty, origin),
            ExprKind::Array(items) => self.lower_array(scope, items, ty, origin),
            ExprKind::ArrayRepeat { elem, len } => self.lower_array_repeat(scope, *elem, *len, origin),
            ExprKind::Index { expr: base, index } => self.lower_index(scope, expr, *base, *index, origin),
            // an operator the checker resolved to the `impl` function of an operator trait is a call of it (Appendix H.10)
            ExprKind::Unary { expr: operand, .. } if let Some(Callee::Method(function)) = self.types.callee(expr) => self.lower_method_call(scope, expr, function, *operand, &[], origin),
            ExprKind::Binary { lhs, rhs, .. } if let Some(Callee::Method(function)) = self.types.callee(expr) => self.lower_method_call(scope, expr, function, *lhs, &[*rhs], origin),
            ExprKind::Unary { op, expr: operand } => {
                let op = match op {
                    UnOp::Neg => UnaryOp::Neg,
                    UnOp::Not => UnaryOp::Not,
                };
                let operand = self.lower_expr(scope, *operand)?;
                Some(self.emit(Expr::Unary { op, expr: operand }, ty, origin))
            }
            ExprKind::Binary { op: op @ (BinOp::Eq | BinOp::Ne), lhs, rhs } if matches!(self.typed(*lhs), Some(Ty::Str)) => self.lower_text_comparison(scope, *op, *lhs, *rhs, origin),
            ExprKind::Binary { op: op @ (BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge), lhs, rhs } if matches!(self.typed(*lhs), Some(Ty::Option(_))) => self.lower_option_comparison(scope, *op, *lhs, *rhs, origin),
            ExprKind::Binary { op, lhs, rhs } => {
                // The right operand of `&&`/`||` runs only when the left does not decide: it is
                // conditional exactly like an `if` arm.
                let short_circuit = matches!(op, BinOp::And | BinOp::Or);
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
                let op = Self::binary_op(*op);
                Some(self.emit(Expr::Binary { op, lhs, rhs }, ty, origin))
            }
            ExprKind::If { cond, then, else_ } if matches!(self.design.expr_node(*cond).kind, ExprKind::Let { .. }) => self.lower_if_let(scope, *cond, then, *else_, ty, None, origin),
            // A condition that reads only key params and constants is folded: only the arm it
            // selects is lowered (`structural.rs`, §11 rule 3).
            ExprKind::If { cond, then, else_ } if let Some(holds) = self.folded_decision(scope, *cond) => {
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
                scope.decisions += 1;
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
                scope.decisions -= 1;
                let (cond, then, otherwise) = (cond?, then?, otherwise?);
                let (then, otherwise) = (self.fit_to(then, &ty), self.fit_to(otherwise, &ty));
                let whole = self.emit(Expr::If { cond, then, otherwise }, ty, origin);
                self.note_branch(whole, cond, then, otherwise);
                Some(whole)
            }
            ExprKind::Block(block) => {
                scope.block_depth += 1;
                let lowered = self.lower_block(scope, file, &block.stmts, node.span);
                scope.block_depth -= 1;
                lowered
            }
            ExprKind::Assign { .. } => self.lower_assignment_expr(scope, expr, origin),
            ExprKind::While { cond, body } => self.lower_while(scope, *cond, body, origin),
            ExprKind::Loop { body } => self.lower_loop(scope, body, ty, matches!(self.typed(expr), Some(Ty::Never)), origin),
            ExprKind::For { pat, iter, body } => self.lower_for(scope, *pat, *iter, body, origin),
            ExprKind::Break(value) => self.lower_break(scope, *value, node.span, origin),
            ExprKind::Continue => Some(self.lower_continue(origin)),
            ExprKind::Return(value) => self.lower_return(scope, *value, node.span, origin),
            ExprKind::Try(inner) => self.lower_try(scope, *inner, node.span, origin),
            ExprKind::Call { args, class, .. } => self.lower_call(scope, expr, &args.iter().map(|a| a.value).collect::<Vec<_>>(), *class, ty, origin),
            ExprKind::MethodCall { recv, args, .. } if let Some(Callee::Method(function)) = self.types.callee(expr) => {
                let arguments: Vec<ExprId> = args.iter().map(|a| a.value).collect();
                self.lower_method_call(scope, expr, function, *recv, &arguments, origin)
            }
            ExprKind::MethodCall { recv, args, .. } if let Some(Callee::Bound { item }) = self.types.callee(expr) => {
                let arguments: Vec<ExprId> = args.iter().map(|a| a.value).collect();
                self.lower_bound_call(scope, expr, item, *recv, &arguments, origin)
            }
            ExprKind::MethodCall { recv, args, .. } if self.types.method(expr).is_some_and(Method::is_sequence_row) => {
                let method = self.types.method(expr)?;
                let args: Vec<ExprId> = args.iter().map(|a| a.value).collect();
                self.lower_sequence_method(scope, expr, method, *recv, &args, ty, origin)
            }
            ExprKind::MethodCall { recv, .. } if self.types.builtin(expr).is_some() => {
                let builtin = self.types.builtin(expr)?;
                self.lower_builtin_read(scope, expr, builtin, *recv, ty, origin)
            }
            ExprKind::MethodCall { recv, args, method: method_name, .. } => {
                let Some(method) = self.types.method(expr) else {
                    // `x.encode()` of a `T: Repr` parameter has no method row: its levels are written out per instance
                    if args.is_empty() && let Some(levels) = self.lower_parameter_encode(scope, expr, *recv, &method_name.text, origin.clone()) {
                        return levels;
                    }
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
                Some(self.emit(Expr::Math { f, args: vec![recv] }, ty, origin))
            }
            ExprKind::Cast { expr: operand, .. } => {
                let operand = self.lower_expr(scope, *operand)?;
                Some(self.emit(Expr::Cast { expr: operand, to: ty.clone() }, ty, origin))
            }
            // A scrutinee over the key params folds: only the arm it selects is lowered (`structural.rs`).
            ExprKind::Match { scrutinee, arms } if let Some(selected) = self.folded_selection(scope, *scrutinee, arms) => self.lower_expr(scope, arms[selected].body),
            ExprKind::Match { scrutinee, arms } => {
                scope.decisions += 1;
                let lowered = self.lower_match(scope, *scrutinee, arms, ty, None, origin);
                scope.decisions -= 1;
                lowered
            }
            ExprKind::Closure { params, body, .. } => self.lower_closure(scope, expr, params, *body, origin),
            other => {
                self.unsupported(file, node.span, crate::types::unsupported::Unsupported::expr(other));
                None
            }
        }
    }

    /// A call: of a function of the reachable set (one `Call` node, the function lowered once
    /// however many bodies call it), or of a core primitive or a probe (see `analog.rs`).
    fn lower_call(&mut self, scope: &mut Scope, expr: ExprId, args: &[ExprId], class: CallClass, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let span = self.design.expr_node(expr).span;
        // a `fn` called in a value that is differentiated is expanded where it stands (`expansion.rs`)
        if scope.differentiated && class != CallClass::Probe && let Some(expanded) = self.expand_function_call(scope, expr) {
            return expanded;
        }
        // a call of a generic function is typed (generics T20 to T23) and instantiated by T33 and T34: until
        // then the elaborator names it and stops
        if let Some(Callee::Function(function)) = self.types.callee(expr)
            && self.design.decl_data(function).generics_span.is_some()
        {
            // a generic `extern` function types like any generic function but has no body to instantiate: its call is
            // refused by name until the feature that runs it (`params-and-io`: `parse_table`, `parse_hex`, `table_model`)
            if matches!(&self.design.decl_data(function).kind, DeclKind::Fn(f) if f.abi.is_some()) {
                let name = self.design.decl_data(function).name.as_ref().map_or_else(String::new, |n| n.text.clone());
                self.unsupported(scope.file, span, &format!("a call of the generic `extern` function `{name}`"));
                return None;
            }
            // the instance is folded and checked (`instances.rs`) and lowered once for its arguments (`variants.rs`)
            let instance = self.concrete_instance(expr)?;
            // a function generic over a nature takes handles: inlined for the nature it is instantiated at
            if self.is_inlined(instance.item) {
                let outer = self.consts.set_substitution(self.instance_substitution(instance.item, &instance.arguments));
                let inlined = self.inline_emitting_value(scope, instance.item, args, origin);
                self.consts.set_substitution(outer);
                return inlined;
            }
            let lowered = self.lower_arguments(scope, expr, instance.item)?;
            let (id, ret) = self.request_instance(instance.item, instance.arguments, Half::Whole)?;
            return Some(self.emit(Expr::Call { function: id, args: lowered }, ret, origin));
        }
        // an associated function of a generic `impl` runs for the arguments the checker read from the call (`Logic::from(b)`)
        if let Some(Callee::Function(function)) = self.types.callee(expr)
            && self.is_generic_impl_item(function)
            && let Some(recorded) = self.types.instantiation(expr).cloned()
        {
            let values = recorded.arguments.iter().map(|value| self.concrete_argument(value, scope.file, span)).collect::<Option<Vec<_>>>()?;
            let lowered = self.lower_arguments(scope, expr, function)?;
            let (id, ret) = self.request_instance(function, values, Half::Whole)?;
            return Some(self.emit(Expr::Call { function: id, args: lowered }, ret, origin));
        }
        match (class, self.types.callee(expr)) {
            (CallClass::Probe, _) => self.lower_probe(scope, expr, ty, origin),
            // A function that emits is no IR function: its call is inlined where it stands (§19 rule 10).
            (_, Some(Callee::Function(function))) if self.is_inlined(function) => self.inline_emitting_value(scope, function, args, origin),
            // A recursion reached from an `analog` block is expanded where it is called (§3 rule 12, `unroll.rs`).
            (_, Some(Callee::Function(function))) if scope.in_analog_block && self.reaches_recursion(function) => self.lower_expanded_call(scope, expr, function, origin),
            (_, Some(Callee::Function(function))) => {
                let lowered = self.lower_arguments(scope, expr, function)?;
                let (id, ret) = self.request(function, Half::Whole)?;
                Some(self.emit(Expr::Call { function: id, args: lowered }, ret, origin))
            }
            (_, Some(Callee::Core(id))) => self.lower_core(scope, expr, id, args, ty, origin),
            // A module with a result is hoisted: its result is read where the placement stands.
            (_, Some(Callee::Module(_))) if self.result_callee(expr).is_some() => self.lower_result(scope, expr),
            (_, Some(Callee::Module(_))) => {
                self.unsupported(scope.file, span, "a placement used as a value");
                None
            }
            // A method of the project is typed (generics T25) and lowered by its own task (T35).
            // `Tr::f(x, ..)` by path: the first argument is the receiver
            (_, Some(Callee::Method(function))) if let Some((receiver, rest)) = args.split_first() => self.lower_method_call(scope, expr, function, *receiver, rest, origin),
            (_, Some(Callee::Bound { item })) if let Some((receiver, rest)) = args.split_first() => self.lower_bound_call(scope, expr, item, *receiver, rest, origin),
            (_, Some(Callee::Method(_) | Callee::Bound { .. })) => {
                self.unsupported(scope.file, span, "a method call");
                None
            }
            // Typed by `types::intrinsic`, lowered by its §22.1 contract (`intrinsics.rs`).
            (_, Some(Callee::Intrinsic(intrinsic))) => self.lower_intrinsic(scope, expr, intrinsic, args, origin),
            (_, None) if self.types.default_of(expr).is_some() => Some(self.emit(Expr::Default(ty.clone()), ty, origin)),
            (_, None) if let ExprKind::Call { callee, .. } = &self.design.expr_node(expr).kind && let Some(owner) = self.constructed_decl(*callee) => self.lower_constructor_call(scope, args, owner, ty, origin),
            (_, None) if let Some(decl) = self.called_local(scope, expr) => self.lower_value_call(scope, expr, decl, args, origin),
            (_, None) => {
                self.unsupported(scope.file, span, "a call the type checker did not resolve");
                None
            }
        }
    }

    /// The arguments of the call `expr` of `function`, one IR argument per parameter in parameter
    /// order, from the checker's binding of the call (§3.2): an `Option<T>` parameter takes two,
    /// its flag and its payload, a parameter left out `(false, zero)`. `None` after a diagnostic.
    pub(super) fn lower_arguments(&mut self, scope: &mut Scope, expr: ExprId, function: DeclId) -> Option<Vec<LoweredExpr>> {
        let Some(binding) = self.types.binding(expr).map(<[_]>::to_vec) else {
            self.unsupported(scope.file, self.design.expr_node(expr).span, "a call whose arguments the type checker did not bind");
            return None;
        };
        let params: Vec<DeclId> = self.design.decl_data(function).children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::FnParam(_))).collect();
        let mut lowered = Vec::new();
        let mut ok = true;
        for (param, slot) in params.iter().zip(binding) {
            match self.typed_local(*param) {
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
    /// order, assignable exactly when the body assigns it (§1.5.12). `None` after a diagnostic
    /// when the checker typed it with a non-value type.
    pub(super) fn bind_local(&mut self, scope: &mut Scope, binding: DeclId) -> Option<u32> {
        let assignable = self.types.is_assigned(binding);
        self.bind_local_as(scope, binding, assignable)
    }

    /// `bind_local` with the assignable flag given: a loop's own variable is bound with `false`
    /// and copied (`assignable_copy`), so the loop's counter is never written.
    pub(super) fn bind_local_as(&mut self, scope: &mut Scope, binding: DeclId, assignable: bool) -> Option<u32> {
        let data = self.design.decl_data(binding);
        let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let (file, span) = (data.file, data.span);
        let Some(ty) = self.typed_local(binding).and_then(|t| self.value_type(&t)) else {
            self.unsupported(file, span, "a name whose type is not a value type");
            return None;
        };
        let index = scope.locals.len() as u32;
        scope.locals.push(Local { name, ty, mutable: assignable });
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
                let block = self.emit(Expr::Block { stmts: stmts.clone(), value }, ty, origin);
                self.note_block(block, &stmts, value);
                Some(block)
            }
        }
    }

    /// The statements of a block in order and its value (the last expression without `;`).
    /// `None` after the diagnostics of every statement that could not be lowered.
    pub(super) fn lower_statements(&mut self, scope: &mut Scope, stmts: &[StmtId]) -> Option<(Vec<LoweredStmt>, Option<LoweredExpr>)> {
        let mut out = Vec::new();
        let mut value = None;
        let mut ok = true;
        let outer = std::mem::take(&mut scope.pending);
        for (i, stmt) in stmts.iter().enumerate() {
            let start = out.len();
            let node = self.design.stmt_node(*stmt).clone();
            let file = self.design.decl_data(node.owner).file;
            let carried = std::mem::replace(&mut ok, true);
            match &node.kind {
                StmtKind::Let { pat, init: Some(init), else_: None, .. } => ok &= self.lower_let(scope, &mut out, *pat, *init, file, node.span),
                // `let x;`: a local first assigned later (§1.7.5, `control.rs`).
                StmtKind::Let { pat, init: Some(init), else_: Some(otherwise), .. } => ok &= self.lower_let_else(scope, &mut out, *pat, *init, otherwise, file),
                StmtKind::Let { pat, init: None, else_: None, .. } => ok &= self.lower_deferred_let(scope, &mut out, *pat, file, node.span),
                // `place = value;` and `place op= value;` (§1.5.12, `control.rs`).
                StmtKind::Expr { expr, .. } if matches!(self.design.expr_node(*expr).kind, ExprKind::Assign { .. }) => ok &= self.lower_assignment(scope, &mut out, *expr),
                // A loop of an `analog` block that contributes or places is its unrolling (`unroll.rs`).
                StmtKind::Expr { expr, .. } if let Some(lowered) = self.lower_analog_loop(scope, &mut out, *expr) => ok &= lowered,
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
            // a statement that failed to lower while nothing was reported is refused by name: nothing vanishes silently (a failure that rests on an earlier diagnostic, such as an unreadable file, keeps that one)
            if !ok && self.diagnostics.is_empty() {
                self.refuse_silent_failure(file, node.span, "this statement");
            }
            ok &= carried;
            // a statement that is the value of its block leaves the flag to the code that takes the value
            if value.is_none() && scope.inline_exits.last_mut().is_some_and(|frame| std::mem::take(&mut frame.loop_flagged)) {
                let origin = self.locator.origin(file, node.span);
                out.push(self.leave_if_inlined_return_ran(scope, origin));
            }
            let declared = std::mem::take(&mut scope.pending);
            out.splice(start..start, declared);
        }
        scope.pending = outer;
        ok.then_some((out, value))
    }

    /// `let pat = init;`: a name, an `Option` (a flag local and a payload local) or a tuple
    /// (one local per component). `false` after a diagnostic.
    fn lower_let(&mut self, scope: &mut Scope, out: &mut Vec<LoweredStmt>, pat: crate::pom::PatId, init: ExprId, file: FileId, span: Span) -> bool {
        match self.design.pat_node(pat).kind.clone() {
            PatKind::Ident { binding, sub: None, .. } if matches!(self.typed_local(binding), Some(Ty::Str)) => self.lower_text_let(scope, binding, init),
            PatKind::Ident { binding, sub: None, .. } if matches!(self.typed_local(binding), Some(Ty::Option(_))) => self.lower_option_let(scope, out, binding, init),
            // An iterator or a range holds no value: the name stands for the chain it was bound to (`iterators.rs`).
            PatKind::Ident { binding, sub: None, .. } if self.is_sequence_binding(binding) => {
                scope.let_sources.insert(binding, init);
                true
            }
            PatKind::Ident { binding, sub: None, .. } => {
                let local = self.bind_local(scope, binding);
                scope.let_sources.insert(binding, init);
                let name = self.design.decl_data(binding).name.as_ref().map(|n| n.text.clone());
                scope.limit_name = name.map(|n| (self.peel_parens(init), n));
                let init = self.lower_expr(scope, init);
                scope.limit_name = None;
                match (local, init) {
                    (Some(local), Some(init)) => {
                        // A local is what its initializer is.
                        let mutable = scope.locals[local as usize].mutable;
                        let ty = self.type_of(init);
                        scope.locals[local as usize].ty = ty;
                        if !mutable {
                            scope.local_inits.insert(local, init);
                        }
                        out.push(self.stmt(Stmt::Let { local, init }));
                        true
                    }
                    _ => false,
                }
            }
            // `let (a, b) = (x, y);` is one local per component; any other value is bound once and
            // taken apart by its pattern (`aggregate.rs`).
            PatKind::Tuple(pats) if self.is_component_let(&pats, init) => self.lower_tuple_let(scope, out, &pats, init, file, span),
            PatKind::Tuple(_) | PatKind::TupleStruct { .. } | PatKind::Struct { .. } | PatKind::Array(_) => self.lower_destructuring_let(scope, out, pat, init),
            _ => {
                self.unsupported(file, span, "a `let` pattern that is not a name");
                false
            }
        }
    }

    pub(super) fn binary_op(op: BinOp) -> BinaryOp {
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
            ExprKind::Path(path) => self.types.width(expr).is_some() || matches!(self.design.path_node(*path).segments.last().map(|s| s.res), Some(Resolution::Decl(d)) if matches!(self.design.decl_data(d).kind, DeclKind::Const(_) | DeclKind::Variant(_))),
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
            // A unit variant is folded to its position among the enum's variants.
            Constant::Int(v) if let ValueType::Enum(id) = ty => self.emit(Expr::VariantOf { ty: id, variant: v as u32, fields: Vec::new() }, ty, origin),
            Constant::Int(v) => self.emit(Expr::Int(v), ty, origin),
            Constant::FloatBits(bits) => self.emit(Expr::FloatBits(bits), ty, origin),
            Constant::Wide(id) => self.emit(Expr::Wide(id), ty, origin),
            // The const evaluator builds no composite constant before the aggregate lowering; a
            // composite folded value reaching here would be a lowering bug, never user input.
            Constant::Aggregate(id) => panic!("the composite constant agg{} reached constant_expr before the aggregate lowering exists", id.0),
        }
    }

    /// A name: a `const` (folded), a param of the body (a read), a local.
    fn lower_path(&mut self, scope: &mut Scope, expr: ExprId, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let node = self.design.expr_node(expr).clone();
        let ExprKind::Path(path) = &node.kind else { return None };
        let res = self.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res);
        if let Some(levels) = self.consts.parameter_levels(expr) {
            return Some(self.constant_expr(Constant::Int(i128::from(levels)), ty, origin));
        }
        let Resolution::Decl(decl) = res else {
            self.unsupported(scope.file, node.span, "a built-in name used as a value");
            return None;
        };
        match &self.design.decl_data(decl).kind {
            // A composite `const` is written out where it is read (`consts/imported.rs`).
            DeclKind::Const(_) if self.consts.is_composite_constant(decl) => self.lower_composite_constant(scope, decl, expr, ty, origin),
            DeclKind::Const(_) => match self.consts.constant(decl) {
                Ok(value) => Some(self.constant_expr(value, ty, origin)),
                Err(fault) => {
                    self.const_fault(&fault, "a `const`");
                    None
                }
            },
            // A unit variant is a value of its enum (§1.3.5).
            DeclKind::Variant(_) => match (EnumLayout::variant_tag(self.design, decl), &ty) {
                (Some(position), ValueType::Enum(id)) => Some(self.emit(Expr::VariantOf { ty: *id, variant: position as u32, fields: Vec::new() }, ty, origin)),
                _ => {
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
            // a carrier declared inside generation, read inside its loops, is the iteration's element (§8.2.23)
            DeclKind::Signal(_) if !self.generated_carrier_levels(scope, expr).is_empty() => match self.element_lookup(scope, expr) {
                super::elements::Lookup::Found(element) => Some(self.carrier_element_read(scope, element, origin)),
                _ => None,
            },
            DeclKind::Port(_) | DeclKind::Signal(_) => match (self.frame_carrier(scope.frame, decl), self.input_of(scope, decl)) {
                (Some(index), _) if scope.in_analog_block && self.structure.carriers[index as usize].count.is_some() => Some(self.elements_of_array(scope, index, ElementPart::Value, origin)),
                (Some(index), _) => Some(self.carrier_read(index, origin)),
                // A value input of a hoisted module: the caller's expression, in the caller's frame.
                (None, Some(Some(given))) => self.in_caller(scope, |this, scope| this.lower_expr(scope, given)),
                (None, Some(None)) => {
                    self.unsupported(scope.file, node.span, "a value input left out, read as a value");
                    None
                }
                // an array of carriers of length 0 read whole is the empty array (§1.2.3)
                (None, None) if self.frames.get(scope.frame).is_some_and(|f| f.empty_arrays.contains(&decl)) => Some(self.emit(Expr::Array(Vec::new()), ty, origin)),
                (None, None) => {
                    self.unsupported(scope.file, node.span, "a node or a port read as a value");
                    None
                }
            },
            // The loop variable of an unrolled analog loop and the parameter of an expanded recursion
            // are the constant of this iteration or call (`unroll.rs`).
            // a handle read: the carrier the caller passed (`handles.rs`)
            DeclKind::FnParam(_) | DeclKind::Binding(_) if let Some(value) = scope.values.get(&decl).copied() => Some(value),
            DeclKind::FnParam(_) if let Some(bound) = scope.handles.get(&decl).copied() => self.lower_expr(scope, bound),
            DeclKind::FnParam(_) | DeclKind::Binding(_) if let Some(value) = scope.unrolled.get(decl) => Some(self.constant_expr(value, ty, origin)),
            DeclKind::FnParam(_) | DeclKind::Binding(_) => match scope.local_index.get(&decl) {
                Some(index) => {
                    let ty = scope.locals[*index as usize].ty.clone();
                    Some(self.emit(Expr::Local(*index), ty, origin))
                }
                // a `let` written in generation: the iteration's value inside its loops, the array of them outside (§8.2.23)
                None if scope.frame == 0 && self.body_let_value(decl).is_some() && self.generated.of(self.design.decl_data(decl).parent?).is_some() => self.lower_generated_let(scope, expr, decl, ty, origin),
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
            // a const parameter read in the body of an instance is the constant of its argument (T33, GN-02)
            DeclKind::GenericParam(_) => match self.consts.substitution().get(decl) {
                Some(crate::types::GenericValue::Const(crate::types::ConstTerm::Value(value))) => Some(self.constant_expr(piperine_ir::Constant::Int(*value), ty, origin)),
                Some(crate::types::GenericValue::Const(crate::types::ConstTerm::Bool(flag))) => Some(self.constant_expr(piperine_ir::Constant::Bool(*flag), ty, origin)),
                _ => {
                    self.unsupported(scope.file, node.span, "a generic parameter read as a value");
                    None
                }
            },
            DeclKind::Fn(_) => self.lower_function_value(scope, decl, ty, node.span, origin),
            DeclKind::Struct(_) => self.lower_unit_struct(ty, origin),
            _ => {
                self.unsupported(scope.file, node.span, "this name as a value");
                None
            }
        }
    }
}
