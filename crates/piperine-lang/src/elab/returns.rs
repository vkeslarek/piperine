//! `return` and `?` (part I §1.8.13, §1.8.14) and the functions whose result is an `Option`.
//!
//! An `Option` is never an IR value (MD-67 item 2): its value is the pair `(is, val)`. A function
//! that returns one is two IR functions, `f.is` and `f.val` (`Half`), each running the whole body
//! and returning its half; a call reads the pair `(f.is(args), f.val(args))`, the arguments read
//! once for each half. Inside such a function `return Some(e)` returns `e`'s half, and `e?` is
//! `if e.is { e.val } else { return None }`, `None` being `(false, the zero of the payload)`.
//!
//! A function with a `return` or a `?` has the result type of its signature, flagged where the
//! function may see unknowns (`FunctionResult`): every return and the tail are held to it.

use piperine_ir::{Expr, ExprId as LoweredExpr, Local, Origin, Place, Stmt, StmtId, ValueType};

use super::control::LoopFrame;

use super::compound::Pair;
use super::elaborator::Elaboration;
use super::known::Flag;
use super::lower::Scope;
use super::variants::Half;
use crate::msl::Span;
use crate::pom::{BlockNode, DeclId, DeclKind, ExprId, ExprKind, FileId, StmtKind};
use crate::types::{Callee, Ty};

/// What a `return` of the function being lowered must give.
#[derive(Clone)]
pub(super) struct FunctionResult {
    /// The IR type of the value returned: the half's type for an `Option` result.
    pub(super) ty: ValueType,
    /// Which half of an `Option` result this variant returns.
    pub(super) half: Half,
    /// The payload of an `Option` result.
    pub(super) payload: Option<ValueType>,
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

impl Elaboration<'_, '_> {
    /// The body of `function` as the variant `half` lowers it, at the result type its signature
    /// gives (`FunctionResult`) when the body returns early.
    pub(super) fn lower_function_body(&mut self, scope: &mut Scope, function: DeclId, half: Half, file: FileId, body: &BlockNode) -> Option<LoweredExpr> {
        let declared = match &self.design.decl_data(function).kind {
            DeclKind::Fn(f) => f.ret.and_then(|t| self.types.written(t).cloned()),
            _ => None,
        };
        let payload = match &declared {
            Some(Ty::Option(payload)) => self.value_type(payload),
            _ => None,
        };
        if matches!(declared, Some(Ty::Option(_))) && (payload.is_none() || half == Half::Whole) {
            self.unsupported(file, body.span, "a function returning an `Option` whose payload has no value in the IR");
            return None;
        }
        let base = match (&payload, half) {
            (Some(_), Half::Presence) => ValueType::Bool,
            (Some(payload), _) => payload.clone(),
            (None, _) => declared.as_ref().and_then(|t| self.value_type(t)).unwrap_or(ValueType::Unit),
        };
        if self.returns_early(body) {
            scope.result = Some(FunctionResult { ty: Self::mutable_type(scope, base), half, payload: payload.clone() });
        }
        let lowered = match &payload {
            Some(_) => self.lower_half_block(scope, file, body, half)?,
            None => self.lower_block(scope, file, &body.stmts, body.span)?,
        };
        match scope.result.clone() {
            Some(result) => {
                let lowered = self.fit_to(lowered, &result.ty);
                Some(self.coerced_to(scope, lowered, &result.ty))
            }
            None => Some(lowered),
        }
    }

    /// Whether `body` holds a `return` or a `?`.
    pub(super) fn returns_early(&self, body: &BlockNode) -> bool {
        let mut inside = Vec::new();
        for expr in crate::pom::block_exprs(self.design, body) {
            crate::pom::preorder(self.design, expr, &mut inside);
        }
        inside.iter().any(|e| matches!(self.design.expr_node(*e).kind, ExprKind::Return(_) | ExprKind::Try(_)))
    }

    /// The block `body` of a function that returns an `Option`, as the half `half` of its tail's
    /// pair: the statements, then the half.
    fn lower_half_block(&mut self, scope: &mut Scope, file: FileId, body: &BlockNode, half: Half) -> Option<LoweredExpr> {
        let origin = self.locator.origin(file, body.span);
        let tail = match body.stmts.last().map(|s| self.design.stmt_node(*s).kind.clone()) {
            Some(StmtKind::Expr { expr, semi: false }) => Some(expr),
            _ => None,
        };
        let statements = if tail.is_some() { &body.stmts[..body.stmts.len() - 1] } else { &body.stmts[..] };
        let (stmts, _) = self.lower_statements(scope, statements)?;
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
        if !scope.inline_exits.is_empty() {
            let Some(value) = value else {
                self.unsupported(scope.file, span, "a `return` with no value in an inlined function that returns an `Option`");
                return None;
            };
            let pair = self.lower_option(scope, value)?;
            return self.leave_inlined(scope, pair.is, pair.val, span, origin);
        }
        let Some(result) = scope.result.clone() else {
            self.unsupported(scope.file, span, "a `return` outside a function that returns early");
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
        let lowered = match lowered {
            Some(value) => Some(self.held_to(scope, value, &result.ty, span)?),
            None => None,
        };
        Some(self.emit_exit(Expr::Return(lowered), origin))
    }

    /// `value` held to the type `want`: made flagged when `want` is and it is not; refused when it
    /// may be unknown and `want` cannot hold one.
    fn held_to(&mut self, scope: &mut Scope, value: LoweredExpr, want: &ValueType, span: Span) -> Option<LoweredExpr> {
        if self.flag(value) == Flag::Flagged && Flag::of(want) == Flag::Plain {
            self.unsupported(scope.file, span, "a value that may be unknown returned from a function whose result cannot hold one");
            return None;
        }
        Some(self.coerced_to(scope, value, want))
    }

    /// `inner?` (§1.8.14), where `inner` is an `Option`: its payload, or the function returns the
    /// empty `Option`.
    pub(super) fn lower_try(&mut self, scope: &mut Scope, inner: ExprId, span: Span, origin: Origin) -> Option<LoweredExpr> {
        if !scope.inline_exits.is_empty() {
            return self.lower_inlined_try(scope, inner, span, origin);
        }
        let Some((result, payload)) = scope.result.clone().and_then(|r| r.payload.clone().map(|p| (r, p))) else {
            self.unsupported(scope.file, span, "`?` outside a function that returns an `Option`");
            return None;
        };
        let pair = self.lower_option(scope, inner)?;
        let empty = match result.half {
            Half::Presence => self.emit(Expr::Bool(false), ValueType::Bool, origin.clone()),
            _ => match self.zero(&payload, origin.clone()) {
                Some(zero) => zero,
                None => {
                    self.unsupported(scope.file, span, "`?` where the payload has no zero in the IR");
                    return None;
                }
            },
        };
        let empty = self.held_to(scope, empty, &result.ty, span)?;
        let exit = self.emit_exit(Expr::Return(Some(empty)), origin.clone());
        let base = match self.type_of(pair.val) {
            ValueType::Flagged(inner) => *inner,
            other => other,
        };
        let exit = self.fit_to(exit, &base);
        let ty = self.flags(&[pair.is, pair.val]).wrap(base);
        Some(self.emit(Expr::If { cond: pair.is, then: pair.val, otherwise: exit }, ty, origin))
    }

    /// The pair `(f.is(args), f.val(args))` of a call of a function that returns an `Option`: the
    /// arguments are lowered once for each half.
    ///
    /// A function that reaches `emit!` or takes a `str` is inlined where it stands (§19 rule 10), and
    /// its body must run once: see `lower_inlined_option_call`.
    pub(super) fn lower_option_call(&mut self, scope: &mut Scope, expr: ExprId, function: DeclId) -> Option<Pair> {
        if self.is_inlined(function) {
            return self.lower_inlined_option_call(scope, expr, function);
        }
        let origin = self.expr_origin(expr);
        let first = self.lower_arguments(scope, expr, function)?;
        let pattern: Vec<Flag> = first.iter().map(|a| self.flag(*a)).collect();
        let (presence, is_ty) = self.request(function, pattern.clone(), Half::Presence)?;
        let (payload, val_ty) = self.request(function, pattern, Half::Payload)?;
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
        let span = self.design.expr_node(expr).span;
        let ExprKind::Call { args, .. } = self.design.expr_node(expr).kind.clone() else { return None };
        let args: Vec<ExprId> = args.iter().map(|a| a.value).collect();
        let (mut stmts, body) = self.bind_inlined_arguments(scope, function, &args)?;
        let tail = match body.stmts.last().map(|s| self.design.stmt_node(*s).kind.clone()) {
            Some(StmtKind::Expr { expr, semi: false }) => expr,
            _ => {
                self.unsupported(scope.file, span, "a function returning an `Option` with no tail value");
                return None;
            }
        };
        let early = self.returns_early(&body);
        let slots = if early { Some(self.declare_exit_slots(function, scope, span, &origin)?) } else { None };
        if let Some(frame) = &slots {
            scope.inline_exits.push(frame.clone());
            scope.loops.push(LoopFrame { value: ValueType::Unit });
        }
        let lowered = self.lower_statements(scope, &body.stmts[..body.stmts.len() - 1]).and_then(|(mut inner, last)| {
            // a block-like statement just before the tail (`if c { .. }`) comes back as the value of the slice
            if let Some(last) = last {
                inner.push(self.stmt(Stmt::Expr(last)));
                if scope.inline_exits.last_mut().is_some_and(|frame| std::mem::take(&mut frame.loop_flagged)) {
                    let origin = self.expr_origin(tail);
                    inner.push(self.leave_if_inlined_return_ran(scope, origin));
                }
            }
            Some((inner, self.lower_option(scope, tail)?))
        });
        let frame = slots.as_ref().and_then(|_| {
            scope.loops.pop();
            scope.inline_exits.pop()
        });
        let (inner, pair) = lowered?;
        let (is_local, val_local) = match &frame {
            Some(frame) => {
                let mut body = inner;
                let (is, val) = (self.held_to(scope, pair.is, &scope.locals[frame.is as usize].ty.clone(), span)?, self.held_to(scope, pair.val, &scope.locals[frame.val as usize].ty.clone(), span)?);
                body.push(self.assign_local(frame.is, is));
                body.push(self.assign_local(frame.val, val));
                let leave = self.emit_exit(Expr::Break(None), origin.clone());
                body.push(self.stmt(Stmt::Expr(leave)));
                let looped = self.emit(Expr::Loop { body }, ValueType::Unit, origin.clone());
                stmts.push(self.assign_flag(scope, frame.flag, false, &origin));
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
        let is_ty = scope.locals[is_local as usize].ty.clone();
        let val_ty = scope.locals[val_local as usize].ty.clone();
        let read_is = self.emit(Expr::Local(is_local), is_ty.clone(), origin.clone());
        let is = self.emit(Expr::Block { stmts, value: Some(read_is) }, is_ty, origin.clone());
        let val = self.emit(Expr::Local(val_local), val_ty, origin);
        Some(Pair { is, val, literal: None })
    }

    /// The temporaries of an inlined `Option` function that leaves early: its flag and payload, of the
    /// types its signature gives (flagged where the code may see unknowns), and the loop flag.
    fn declare_exit_slots(&mut self, function: DeclId, scope: &mut Scope, span: Span, origin: &Origin) -> Option<InlineExit> {
        let declared = match &self.design.decl_data(function).kind {
            DeclKind::Fn(f) => f.ret.and_then(|t| self.types.written(t).cloned()),
            _ => None,
        };
        let payload = match &declared {
            Some(Ty::Option(payload)) => self.value_type(payload),
            _ => None,
        };
        let Some(payload) = payload else {
            self.unsupported(scope.file, span, "a function returning an `Option` whose payload has no value in the IR");
            return None;
        };
        let is_ty = Self::mutable_type(scope, ValueType::Bool);
        let val_ty = Self::mutable_type(scope, payload);
        let is = self.declare_typed(scope, "option.is", is_ty, origin)?;
        let val = self.declare_typed(scope, "option.val", val_ty, origin)?;
        let flag = self.declare_typed(scope, "option.left", Self::mutable_type(scope, ValueType::Bool), origin)?;
        Some(InlineExit { is, val, flag, depth: scope.loops.len() + 1, flag_uses: 0, loop_flagged: false })
    }

    /// `return pair` or the `None` of a `?` inside an inlined `Option` function: the pair is assigned to
    /// its temporaries and the body's `loop` is left (from inside a loop of the body, through the flag
    /// its statements test). Diverges. `None` after a diagnostic.
    fn leave_inlined(&mut self, scope: &mut Scope, is: LoweredExpr, val: LoweredExpr, span: Span, origin: Origin) -> Option<LoweredExpr> {
        let frame = scope.inline_exits.last()?.clone();
        let is = self.held_to(scope, is, &scope.locals[frame.is as usize].ty.clone(), span)?;
        let val = self.held_to(scope, val, &scope.locals[frame.val as usize].ty.clone(), span)?;
        let mut stmts = vec![self.assign_local(frame.is, is), self.assign_local(frame.val, val)];
        if scope.loops.len() > frame.depth {
            stmts.push(self.assign_flag(scope, frame.flag, true, &origin));
            if let Some(top) = scope.inline_exits.last_mut() {
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
        let frame = scope.inline_exits.last()?.clone();
        let payload = match scope.locals[frame.val as usize].ty.clone() {
            ValueType::Flagged(inner) => *inner,
            other => other,
        };
        let none = self.emit(Expr::Bool(false), ValueType::Bool, origin.clone());
        let Some(zero) = self.zero(&payload, origin.clone()) else {
            self.unsupported(scope.file, span, "`?` where the payload has no zero in the IR");
            return None;
        };
        let exit = self.leave_inlined(scope, none, zero, span, origin.clone())?;
        let base = match self.type_of(pair.val) {
            ValueType::Flagged(inner) => *inner,
            other => other,
        };
        let exit = self.fit_to(exit, &base);
        let ty = self.flags(&[pair.is, pair.val]).wrap(base);
        Some(self.emit(Expr::If { cond: pair.is, then: pair.val, otherwise: exit }, ty, origin))
    }

    /// After a loop of an inlined body in which a `return` ran: `if left { break }`, which leaves the next
    /// loop out, up to the one the body runs in.
    pub(super) fn leave_if_inlined_return_ran(&mut self, scope: &Scope, origin: Origin) -> StmtId {
        let flag = scope.inline_exits.last().map_or(0, |frame| frame.flag);
        let flag_ty = scope.locals[flag as usize].ty.clone();
        let read = self.emit(Expr::Local(flag), flag_ty.clone(), origin.clone());
        // the flag is always known; a flagged local is read through `known_or`
        let raised = match flag_ty {
            ValueType::Flagged(_) => {
                let default = self.emit(Expr::Bool(false), ValueType::Bool, origin.clone());
                self.emit(Expr::KnownOr { value: read, default }, ValueType::Bool, origin.clone())
            }
            _ => read,
        };
        let leave = self.emit_exit(Expr::Break(None), origin.clone());
        let nothing = self.emit(Expr::Unit, ValueType::Unit, origin.clone());
        let test = self.emit(Expr::If { cond: raised, then: leave, otherwise: nothing }, ValueType::Unit, origin);
        self.stmt(Stmt::Expr(test))
    }

    /// The statement `flag = raised;`, the value held to the flag's type.
    fn assign_flag(&mut self, scope: &mut Scope, flag: u32, raised: bool, origin: &Origin) -> StmtId {
        let value = self.emit(Expr::Bool(raised), ValueType::Bool, origin.clone());
        let ty = scope.locals[flag as usize].ty.clone();
        let value = self.coerced_to(scope, value, &ty);
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
        let base = match &ty {
            ValueType::Flagged(inner) => (**inner).clone(),
            other => other.clone(),
        };
        let zero = self.zero(&base, origin.clone())?;
        let init = self.coerced_to(scope, zero, &ty);
        let local = scope.locals.len() as u32;
        scope.locals.push(Local { name: name.to_string(), ty, mutable: true });
        let declaration = self.stmt(Stmt::Let { local, init });
        scope.pending.push(declaration);
        Some(local)
    }

    /// The function `expr` calls when its result is an `Option`.
    pub(super) fn calls_option_function(&self, expr: ExprId) -> Option<DeclId> {
        let Some(Callee::Function(function)) = self.types.callee(expr) else { return None };
        let DeclKind::Fn(f) = &self.design.decl_data(function).kind else { return None };
        f.ret.and_then(|t| self.types.written(t)).is_some_and(|t| matches!(t, Ty::Option(_))).then_some(function)
    }
}
