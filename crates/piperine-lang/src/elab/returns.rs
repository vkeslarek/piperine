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

use piperine_ir::{Expr, ExprId as LoweredExpr, Origin, ValueType};

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
    fn returns_early(&self, body: &BlockNode) -> bool {
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
    pub(super) fn lower_option_call(&mut self, scope: &mut Scope, expr: ExprId, function: DeclId) -> Option<Pair> {
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

    /// The function `expr` calls when its result is an `Option`.
    pub(super) fn calls_option_function(&self, expr: ExprId) -> Option<DeclId> {
        let Some(Callee::Function(function)) = self.types.callee(expr) else { return None };
        let DeclKind::Fn(f) = &self.design.decl_data(function).kind else { return None };
        f.ret.and_then(|t| self.types.written(t)).is_some_and(|t| matches!(t, Ty::Option(_))).then_some(function)
    }
}
