//! `return` and `?` (part I §1.8.13, §1.8.14) and the functions whose result is an `Option`.
//!
//! An `Option` is never an IR value (MD-67 item 2): its value is the pair `(is, val)`. A function
//! that returns one is two IR functions, `f.is` and `f.val` (`Half`), each running the whole body
//! and returning its half; a call reads the pair `(f.is(args), f.val(args))`, the arguments read
//! once for each half. Inside such a function `return Some(e)` returns `e`'s half, and `e?` is
//! `if e.is { e.val } else { return None }`, `None` being `(false, the zero of the payload)`.
//!
//! A function with a `return` or a `?` has the result type of its signature
//! (`FunctionResult`): every return and the tail are held to it.

use super::ValueLowering;
use piperine_ir::{Expr, ExprId as LoweredExpr, Local, Origin, Place, Stmt, StmtId, ValueType};

use super::control::LoopFrame;

use super::compound::Pair;
use crate::elab::elaborator::Scope;
use crate::elab::functions::variants::Half;
use crate::elab::functions::instances::ConcreteValue;
use crate::refusal::forms::{GenericsForm, ValueForm};
use crate::msl::Span;
use crate::pom::{BlockNode, DeclId, DeclKind, ExprId, ExprKind, FileId, StmtKind};
use crate::types::{Callee, Ty};

/// What a `return` of the function being lowered must give.
#[derive(Clone)]
pub(in crate::elab) struct FunctionResult {
    /// The IR type of the value returned: the half's type for an `Option` result.
    pub(in crate::elab) ty: ValueType,
    /// Which half of an `Option` result this variant returns.
    pub(in crate::elab) half: Half,
    /// The payload of an `Option` result.
    pub(in crate::elab) payload: Option<ValueType>,
}

/// An inlined function that returns an `Option` and leaves early (§19 rule 10, §1.8.13): where its
/// `return` and `?` write the pair they give.
#[derive(Clone)]
pub(super) struct InlineExit {
    /// The temporaries holding the flag and the payload, and the flag that says a `return` ran inside a
    /// loop of the body.
    is: u32,
    val: u32,
    flag: u32,
    /// How many loops `scope.loops` holds at the body of the inlined function: a `return` above that
    /// depth is inside a loop of its own and sets `flag` before it breaks.
    depth: usize,
    /// How many `return`s set `flag` so far; a loop of the body that raised it is followed by the test.
    pub(super) flag_uses: usize,
    /// Whether a loop that raised `flag` ended in the statement being lowered.
    pub(super) loop_flagged: bool,
}

impl ValueLowering<'_, '_, '_> {
    /// The body of `function` as the variant `half` lowers it, at the result type its signature
    /// gives (`FunctionResult`) when the body returns early.
    pub(in crate::elab) fn lower_function_body(&mut self, scope: &mut Scope, function: DeclId, half: Half, file: FileId, body: &BlockNode) -> Option<LoweredExpr> {
        let declared = match &self.state.design.decl_data(function).kind {
            DeclKind::Fn(f) => f.ret.and_then(|t| self.typed_written(t)),
            _ => None,
        };
        let payload = match &declared {
            Some(Ty::Option(payload)) => self.value_type(payload),
            _ => None,
        };
        if matches!(declared, Some(Ty::Option(_))) && (payload.is_none() || half == Half::Whole) {
            self.refuse(file, body.span, ValueForm::FunctionReturningOptionWhosePayloadHasNo);
            return None;
        }
        let base = match (&payload, half) {
            (Some(_), Half::Presence) => ValueType::Bool,
            (Some(payload), _) => payload.clone(),
            (None, _) => declared.as_ref().and_then(|t| self.value_type(t)).unwrap_or(ValueType::Unit),
        };
        if self.returns_early(body) {
            scope.values.result = Some(FunctionResult { ty: base, half, payload: payload.clone() });
        }
        let lowered = match &payload {
            Some(_) => self.lower_half_block(scope, file, body, half)?,
            None => self.lower_block(scope, file, &body.stmts, body.span)?,
        };
        match scope.values.result.clone() {
            Some(result) => Some(self.fit_to(lowered, &result.ty)),
            None => Some(lowered),
        }
    }

    /// Whether `body` holds a `return` or a `?`.
    pub(super) fn returns_early(&self, body: &BlockNode) -> bool {
        let mut inside = Vec::new();
        for expr in crate::pom::BodyWalk::new(self.state.design).block_expressions(body) {
            crate::pom::BodyWalk::new(self.state.design).descendants_depth_first(expr, &mut inside);
        }
        inside.iter().any(|e| matches!(self.state.design.expr_node(*e).kind, ExprKind::Return(_) | ExprKind::Try(_)))
    }

    /// The block `body` of a function that returns an `Option`, as the half `half` of its tail's
    /// pair: the statements, then the half.
    fn lower_half_block(&mut self, scope: &mut Scope, file: FileId, body: &BlockNode, half: Half) -> Option<LoweredExpr> {
        let origin = self.state.locator.origin(file, body.span);
        let tail = match body.stmts.last().map(|s| self.state.design.stmt_node(*s).kind.clone()) {
            Some(StmtKind::Expr { expr, semi: false }) => Some(expr),
            _ => None,
        };
        let statements = if tail.is_some() { &body.stmts[..body.stmts.len() - 1] } else { &body.stmts[..] };
        let (mut stmts, last) = self.lower_statements(scope, statements)?;
        // a block-like statement just before the tail (`for k in .. { }`, `if c { .. }`) comes back as the value of the slice: it still runs
        if let Some(last) = last {
            stmts.push(self.stmt(Stmt::Expr(last)));
        }
        let value = match tail {
            Some(tail) => {
                let pair = self.lower_option(scope, tail)?;
                Some(Self::half_of(&pair, half))
            }
            None => None,
        };
        let ty = value.map_or(ValueType::Unit, |v| self.type_of(v));
        let block = self.emit(Expr::Block { stmts: stmts.clone(), value }, ty, origin);
        self.note_block(block, &stmts, value);
        Some(block)
    }

    /// The expression of `pair` that `half` returns.
    fn half_of(pair: &Pair, half: Half) -> LoweredExpr {
        match half {
            Half::Presence => pair.is,
            _ => pair.val,
        }
    }

    /// `return` or `return value` in a function: leaves it with the value held to the function's
    /// result type.
    pub(super) fn lower_return(&mut self, scope: &mut Scope, value: Option<ExprId>, span: Span, origin: Origin) -> Option<LoweredExpr> {
        if !scope.values.inline_exits.is_empty() {
            let Some(value) = value else {
                self.refuse(scope.values.file, span, ValueForm::ReturnWithNoValueInInlinedFunction);
                return None;
            };
            let pair = self.lower_option(scope, value)?;
            return self.leave_inlined(scope, pair.is, pair.val, origin);
        }
        let Some(result) = scope.values.result.clone() else {
            self.refuse(scope.values.file, span, ValueForm::ReturnOutsideFunctionThatReturnsEarly);
            return None;
        };
        let lowered = match (value, &result.payload) {
            (Some(value), Some(_)) => {
                let pair = self.lower_option(scope, value)?;
                Some(Self::half_of(&pair, result.half))
            }
            (Some(value), None) => Some(self.lower_expr(scope, value)?),
            (None, _) => None,
        };
        Some(self.emit_exit(Expr::Return(lowered), origin))
    }

    /// `inner?` (§1.8.14), where `inner` is an `Option`: its payload, or the function returns the
    /// empty `Option`.
    pub(super) fn lower_try(&mut self, scope: &mut Scope, inner: ExprId, span: Span, origin: Origin) -> Option<LoweredExpr> {
        if !scope.values.inline_exits.is_empty() {
            return self.lower_inlined_try(scope, inner, span, origin);
        }
        let Some((result, payload)) = scope.values.result.clone().and_then(|r| r.payload.clone().map(|p| (r, p))) else {
            self.refuse(scope.values.file, span, ValueForm::OutsideFunctionThatReturnsOption);
            return None;
        };
        let pair = self.lower_option(scope, inner)?;
        let empty = match result.half {
            Half::Presence => self.emit(Expr::Bool(false), ValueType::Bool, origin.clone()),
            _ => match self.zero(&payload, origin.clone()) {
                Some(zero) => zero,
                None => {
                    self.refuse(scope.values.file, span, ValueForm::WherePayloadHasNoZeroInIr);
                    return None;
                }
            },
        };
        let exit = self.emit_exit(Expr::Return(Some(empty)), origin.clone());
        let base = self.type_of(pair.val);
        let exit = self.fit_to(exit, &base);
        Some(self.emit(Expr::If { cond: pair.is, then: pair.val, otherwise: exit }, base, origin))
    }

    /// The pair `(f.is(args), f.val(args))` of a call of a function that returns an `Option`: the
    /// arguments are lowered once for each half.
    ///
    /// A function that reaches `emit!` or takes a `str` is inlined where it stands (§19 rule 10), and
    /// its body must run once: see `lower_inlined_option_call`.
    pub(super) fn lower_option_call(&mut self, scope: &mut Scope, expr: ExprId, function: DeclId) -> Option<Pair> {
        // a generic function runs for the arguments the checker recorded at the call, one instance of each half (§3.8.10)
        let instance = if self.state.design.decl_data(function).generics_span.is_some() { Some(self.functions().concrete_instance(expr)?) } else { None };
        let arguments = instance.as_ref().map(|instance| instance.arguments.clone()).unwrap_or_default();
        if self.is_inlined(function) {
            let Some(instance) = instance else { return self.lower_inlined_option_call(scope, expr, function) };
            let outer = self.elaboration.state.consts.set_substitution(self.instance_substitution(instance.item, &instance.arguments));
            let inlined = self.lower_inlined_option_call(scope, expr, function);
            self.state.consts.set_substitution(outer);
            return inlined;
        }
        let origin = self.expr_origin(expr);
        let first = self.lower_arguments(scope, expr, function)?;
        let (presence, is_ty) = self.functions().request_instance(function, arguments.clone(), Half::Presence)?;
        let (payload, val_ty) = self.functions().request_instance(function, arguments, Half::Payload)?;
        let second = self.lower_arguments(scope, expr, function)?;
        let is = self.emit(Expr::Call { function: presence, args: first }, is_ty, origin.clone());
        let val = self.emit(Expr::Call { function: payload, args: second }, val_ty, origin);
        Some(Pair { is, val, literal: None })
    }

    /// A call of an inlined function that returns an `Option` (§19 rule 10): the body runs once. Its
    /// statements, then the tail's pair, assign two temporaries; `is` is the block that runs them and
    /// reads the flag, `val` reads the payload, which every reader of the pair does after `is`. The
    /// temporaries are declared before the statement being lowered (`Scope::pending`), where every read
    /// sees them bound, so the `emit!` of the body runs exactly once per call. A body that leaves early
    /// runs inside a `loop` that ends with a `break`: its `return` and `?` assign the temporaries and
    /// break out (`leave_inlined`). `None` after a diagnostic.
    fn lower_inlined_option_call(&mut self, scope: &mut Scope, expr: ExprId, function: DeclId) -> Option<Pair> {
        let origin = self.expr_origin(expr);
        let span = self.state.design.expr_node(expr).span;
        let ExprKind::Call { args, .. } = self.state.design.expr_node(expr).kind.clone() else { return None };
        let args: Vec<ExprId> = args.iter().map(|a| a.value).collect();
        let (mut stmts, body) = self.bind_inlined_arguments(scope, function, &args)?;
        let tail = match body.stmts.last().map(|s| self.state.design.stmt_node(*s).kind.clone()) {
            Some(StmtKind::Expr { expr, semi: false }) => expr,
            _ => {
                self.refuse(scope.values.file, span, ValueForm::FunctionReturningOptionWithNoTailValue);
                return None;
            }
        };
        let early = self.returns_early(&body);
        let slots = if early { Some(self.declare_exit_slots(function, scope, span, &origin)?) } else { None };
        if let Some(frame) = &slots {
            scope.values.inline_exits.push(frame.clone());
            scope.values.loops.push(LoopFrame);
        }
        let lowered = self.lower_statements(scope, &body.stmts[..body.stmts.len() - 1]).and_then(|(mut inner, last)| {
            // a block-like statement just before the tail (`if c { .. }`) comes back as the value of the slice
            if let Some(last) = last {
                inner.push(self.stmt(Stmt::Expr(last)));
                if scope.values.inline_exits.last_mut().is_some_and(|frame| std::mem::take(&mut frame.loop_flagged)) {
                    let origin = self.expr_origin(tail);
                    inner.push(self.leave_if_inlined_return_ran(scope, origin));
                }
            }
            Some((inner, self.lower_option(scope, tail)?))
        });
        let frame = slots.as_ref().and_then(|_| {
            scope.values.loops.pop();
            scope.values.inline_exits.pop()
        });
        let (inner, pair) = lowered?;
        let (is_local, val_local) = match &frame {
            Some(frame) => {
                let mut body = inner;
                body.push(self.assign_local(frame.is, pair.is));
                body.push(self.assign_local(frame.val, pair.val));
                let leave = self.emit_exit(Expr::Break(None), origin.clone());
                body.push(self.stmt(Stmt::Expr(leave)));
                let looped = self.emit(Expr::Loop { body }, ValueType::Unit, origin.clone());
                stmts.push(self.assign_flag(frame.flag, false, &origin));
                stmts.push(self.stmt(Stmt::Expr(looped)));
                (frame.is, frame.val)
            }
            None => {
                stmts.extend(inner);
                let is_local = self.declare_temporary(scope, "option.is", pair.is, &origin)?;
                let val_local = self.declare_temporary(scope, "option.val", pair.val, &origin)?;
                for (local, value) in [(is_local, pair.is), (val_local, pair.val)] {
                    stmts.push(self.assign_local(local, value));
                }
                (is_local, val_local)
            }
        };
        let is_ty = scope.values.locals[is_local as usize].ty.clone();
        let val_ty = scope.values.locals[val_local as usize].ty.clone();
        let read_is = self.emit(Expr::Local(is_local), is_ty.clone(), origin.clone());
        let is = self.emit(Expr::Block { stmts, value: Some(read_is) }, is_ty, origin.clone());
        let val = self.emit(Expr::Local(val_local), val_ty, origin);
        Some(Pair { is, val, literal: None })
    }

    /// The temporaries of an inlined `Option` function that leaves early: its flag and payload, of the
    /// types its signature gives, and the loop flag.
    fn declare_exit_slots(&mut self, function: DeclId, scope: &mut Scope, span: Span, origin: &Origin) -> Option<InlineExit> {
        let declared = match &self.state.design.decl_data(function).kind {
            DeclKind::Fn(f) => f.ret.and_then(|t| self.typed_written(t)),
            _ => None,
        };
        let payload = match &declared {
            Some(Ty::Option(payload)) => self.value_type(payload),
            _ => None,
        };
        let Some(payload) = payload else {
            self.refuse(scope.values.file, span, ValueForm::FunctionReturningOptionWhosePayloadHasNo);
            return None;
        };
        let is = self.declare_typed(scope, "option.is", ValueType::Bool, origin)?;
        let val = self.declare_typed(scope, "option.val", payload, origin)?;
        let flag = self.declare_typed(scope, "option.left", ValueType::Bool, origin)?;
        Some(InlineExit { is, val, flag, depth: scope.values.loops.len() + 1, flag_uses: 0, loop_flagged: false })
    }

    /// `return pair` or the `None` of a `?` inside an inlined `Option` function: the pair is assigned to
    /// its temporaries and the body's `loop` is left (from inside a loop of the body, through the flag
    /// its statements test). Diverges. `None` after a diagnostic.
    fn leave_inlined(&mut self, scope: &mut Scope, is: LoweredExpr, val: LoweredExpr, origin: Origin) -> Option<LoweredExpr> {
        let frame = scope.values.inline_exits.last()?.clone();
        let mut stmts = vec![self.assign_local(frame.is, is), self.assign_local(frame.val, val)];
        if scope.values.loops.len() > frame.depth {
            stmts.push(self.assign_flag(frame.flag, true, &origin));
            if let Some(top) = scope.values.inline_exits.last_mut() {
                top.flag_uses += 1;
            }
        }
        let leave = self.emit_exit(Expr::Break(None), origin.clone());
        let block = self.emit(Expr::Block { stmts: stmts.clone(), value: Some(leave) }, ValueType::Unit, origin);
        self.note_block(block, &stmts, Some(leave));
        Some(block)
    }

    /// `inner?` inside an inlined `Option` function: its payload, or the function leaves with `None`.
    fn lower_inlined_try(&mut self, scope: &mut Scope, inner: ExprId, span: Span, origin: Origin) -> Option<LoweredExpr> {
        let pair = self.lower_option(scope, inner)?;
        let frame = scope.values.inline_exits.last()?.clone();
        let payload = scope.values.locals[frame.val as usize].ty.clone();
        let none = self.emit(Expr::Bool(false), ValueType::Bool, origin.clone());
        let Some(zero) = self.zero(&payload, origin.clone()) else {
            self.refuse(scope.values.file, span, ValueForm::WherePayloadHasNoZeroInIr);
            return None;
        };
        let exit = self.leave_inlined(scope, none, zero, origin.clone())?;
        let base = self.type_of(pair.val);
        let exit = self.fit_to(exit, &base);
        Some(self.emit(Expr::If { cond: pair.is, then: pair.val, otherwise: exit }, base, origin))
    }

    /// After a loop of an inlined body in which a `return` ran: `if left { break }`, which leaves the next
    /// loop out, up to the one the body runs in.
    pub(super) fn leave_if_inlined_return_ran(&mut self, scope: &Scope, origin: Origin) -> StmtId {
        let flag = scope.values.inline_exits.last().map_or(0, |frame| frame.flag);
        let flag_ty = scope.values.locals[flag as usize].ty.clone();
        let read = self.emit(Expr::Local(flag), flag_ty.clone(), origin.clone());
        let leave = self.emit_exit(Expr::Break(None), origin.clone());
        let nothing = self.emit(Expr::Unit, ValueType::Unit, origin.clone());
        let test = self.emit(Expr::If { cond: read, then: leave, otherwise: nothing }, ValueType::Unit, origin);
        self.stmt(Stmt::Expr(test))
    }

    /// The statement `flag = raised;`.
    fn assign_flag(&mut self, flag: u32, raised: bool, origin: &Origin) -> StmtId {
        let value = self.emit(Expr::Bool(raised), ValueType::Bool, origin.clone());
        self.assign_local(flag, value)
    }

    /// The statement `local = value;`.
    fn assign_local(&mut self, local: u32, value: LoweredExpr) -> StmtId {
        self.stmt(Stmt::Assign { place: Place { local, path: Vec::new() }, op: None, value })
    }

    /// A new mutable local `name` of the type of `value`, declared with the zero of it in
    /// `Scope::pending`. `None` after a diagnostic.
    fn declare_temporary(&mut self, scope: &mut Scope, name: &str, value: LoweredExpr, origin: &Origin) -> Option<u32> {
        let ty = self.type_of(value);
        self.declare_typed(scope, name, ty, origin)
    }

    /// A new mutable local `name` of type `ty`, declared with the zero of it in `Scope::pending`.
    /// `None` after a diagnostic.
    fn declare_typed(&mut self, scope: &mut Scope, name: &str, ty: ValueType, origin: &Origin) -> Option<u32> {
        let init = self.zero(&ty, origin.clone())?;
        let local = scope.values.locals.len() as u32;
        scope.values.locals.push(Local { name: name.to_string(), ty, mutable: true });
        let declaration = self.stmt(Stmt::Let { local, init });
        scope.values.pending.push(declaration);
        Some(local)
    }

    /// The function `expr` calls when its result is an `Option`.
    pub(in crate::elab) fn calls_option_function(&self, expr: ExprId) -> Option<DeclId> {
        let Some(Callee::Function(function)) = self.state.types.callee(expr) else { return None };
        let DeclKind::Fn(f) = &self.state.design.decl_data(function).kind else { return None };
        f.ret.and_then(|t| self.typed_written(t)).is_some_and(|t| matches!(t, Ty::Option(_))).then_some(function)
    }

    /// The method `expr` calls when its result is an `Option`.
    pub(super) fn calls_option_method(&self, expr: ExprId) -> Option<DeclId> {
        let Some(Callee::Method(function)) = self.state.types.callee(expr) else { return None };
        let DeclKind::Fn(f) = &self.state.design.decl_data(function).kind else { return None };
        f.ret.and_then(|t| self.typed_written(t)).is_some_and(|t| matches!(t, Ty::Option(_))).then_some(function)
    }

    /// A call `receiver.f(arguments)` of a method that returns an `Option`: the pair of its two halves, each a call of
    /// the method's instance for the arguments its `impl` takes from the receiver (`Logic<4>` gives `N = 4`). The
    /// receiver and the arguments are lowered once for each half. `None` after a diagnostic.
    pub(super) fn lower_option_instance_method(&mut self, scope: &mut Scope, expr: ExprId, function: DeclId, receiver: ExprId, arguments: &[ExprId]) -> Option<Pair> {
        let span = self.state.design.expr_node(expr).span;
        if self.is_inlined(function) {
            self.refuse(scope.values.file, span, ValueForm::MethodOnNatureThatReturnsOption);
            return None;
        }
        let origin = self.expr_origin(expr);
        let receiver_type = self.typed(receiver)?;
        let instance = if self.is_generic_impl_item(function) {
            let Some(values) = self.state.types.impl_arguments(function, &receiver_type) else {
                self.refuse(scope.values.file, span, GenericsForm::MethodOfGenericImplWhoseParametersReceiver);
                return None;
            };
            values.iter().map(|value| self.functions().concrete_argument(value, scope.values.file, span)).collect::<Option<Vec<_>>>()?
        } else if self.is_trait_function(function) {
            vec![ConcreteValue::Type(receiver_type)]
        } else {
            Vec::new()
        };
        let lower_all = |this: &mut Self, scope: &mut Scope| -> Option<Vec<LoweredExpr>> {
            let mut lowered = vec![this.lower_expr(scope, receiver)?];
            for argument in arguments {
                lowered.push(this.lower_expr(scope, *argument)?);
            }
            Some(lowered)
        };
        let first = lower_all(self, scope)?;
        let (presence, is_ty) = self.functions().request_instance(function, instance.clone(), Half::Presence)?;
        let (payload, val_ty) = self.functions().request_instance(function, instance, Half::Payload)?;
        let second = lower_all(self, scope)?;
        let is = self.emit(Expr::Call { function: presence, args: first }, is_ty, origin.clone());
        let val = self.emit(Expr::Call { function: payload, args: second }, val_ty, origin);
        Some(Pair { is, val, literal: None })
    }
}
