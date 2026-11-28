//! Mutation and control flow (part I §1.5.12, §1.7.5, §1.8): `let mut`, assignment, the deferred
//! `let x;`, loops and the exits that leave them.
//!
//! A local the body assigns is a `mutable` IR local, whatever the source wrote on its `let`
//! (every local is a place, §1.5.12), and an assignment is `Stmt::Assign`.

use super::ValueLowering;
use piperine_ir::{Expr, ExprId as LoweredExpr, Local, Origin, Stmt, StmtId as LoweredStmt, ValueType};

use crate::elab::elaborator::Scope;
use crate::refusal::forms::ValueForm;
use crate::pom::{BlockNode, DeclId, ExprId, ExprKind, FileId, PatId, PatKind, Resolution};
use crate::types::Callee;

/// One loop the code being lowered sits in; `Scope::loops` counts them, innermost last.
pub(in crate::elab) struct LoopFrame;

impl ValueLowering<'_, '_, '_> {
    /// `let x;` (§1.7.5): a local with no value yet. The checker proves it is assigned before it is
    /// read, so the IR local starts as the zero of its type and is assignable. `false` after a diagnostic.
    pub(super) fn lower_deferred_let(&mut self, scope: &mut Scope, out: &mut Vec<LoweredStmt>, pat: PatId, file: FileId, span: crate::msl::Span) -> bool {
        let PatKind::Ident { binding, sub: None, .. } = self.state.design.pat_node(pat).kind.clone() else {
            self.refuse(file, span, ValueForm::DeferredLetWhosePatternIsNotName);
            return false;
        };
        let Some(local) = self.bind_local(scope, binding) else { return false };
        let ty = scope.values.locals[local as usize].ty.clone();
        let origin = self.state.locator.origin(file, span);
        let Some(init) = self.zero(&ty, origin) else {
            self.refuse(file, span, ValueForm::DeferredLetOfTypeWithNoZero);
            return false;
        };
        let slot = &mut scope.values.locals[local as usize];
        slot.ty = ty;
        slot.mutable = true;
        out.push(self.stmt(Stmt::Let { local, init }));
        true
    }

    /// `place = value;` or `place op= value;` as a statement of `out`. `false` after a diagnostic.
    pub(super) fn lower_assignment(&mut self, scope: &mut Scope, out: &mut Vec<LoweredStmt>, expr: ExprId) -> bool {
        let node = self.state.design.expr_node(expr).clone();
        let ExprKind::Assign { op, lhs, rhs } = node.kind else { return false };
        let Some(place) = self.place_of(scope, lhs) else { return false };
        if self.place_type(scope, &place).is_none() {
            self.refuse(scope.values.file, node.span, ValueForm::AssignmentToPlaceWhoseTypeHasNo);
            return false;
        }
        // `a &= b` on a library type is `a = a & b`: the call of the operator's `impl` function (Appendix H.10 rule 4)
        if op.is_some()
            && let Some(Callee::Method(function)) = self.state.types.callee(expr)
        {
            let origin = self.expr_origin(expr);
            let Some(value) = self.lower_method_call(scope, expr, function, lhs, &[rhs], origin) else { return false };
            out.push(self.stmt(Stmt::Assign { place, op: None, value }));
            return true;
        }
        let Some(value) = self.lower_expr(scope, rhs) else { return false };
        let op = op.map(Self::binary_op);
        out.push(self.stmt(Stmt::Assign { place, op, value }));
        true
    }

    /// An assignment where a value is read (`if c { x = 1.0 }` without braces, a match arm): a
    /// block of the one statement, of type `()`.
    pub(super) fn lower_assignment_expr(&mut self, scope: &mut Scope, expr: ExprId, origin: Origin) -> Option<LoweredExpr> {
        let mut stmts = Vec::new();
        if !self.lower_assignment(scope, &mut stmts, expr) {
            return None;
        }
        Some(self.emit(Expr::Block { stmts, value: None }, ValueType::Unit, origin))
    }

    /// The IR local an assignment's target names: a `let mut` or deferred local.
    pub(super) fn assigned_local(&mut self, scope: &Scope, target: ExprId) -> Option<u32> {
        let node = self.state.design.expr_node(self.peel_parens(target)).clone();
        let ExprKind::Path(path) = &node.kind else {
            self.refuse(scope.values.file, node.span, ValueForm::AssignmentToFieldOrElement);
            return None;
        };
        let Some(Resolution::Decl(decl)) = self.state.design.path_node(*path).segments.last().map(|s| s.res) else {
            self.refuse(scope.values.file, node.span, ValueForm::AssignmentToNameThatIsNotLocal);
            return None;
        };
        match scope.values.local_index.get(&decl) {
            Some(index) if scope.values.locals[*index as usize].mutable => Some(*index),
            _ => {
                self.refuse(scope.values.file, node.span, ValueForm::AssignmentToLocalThatIsNotMutable);
                None
            }
        }
    }
}

// ---- loops and the exits that leave them (§1.8.7 to §1.8.12) ------------------------------

impl ValueLowering<'_, '_, '_> {
    /// `while cond { body }`, whose condition is read before every iteration: type `()`.
    pub(super) fn lower_while(&mut self, scope: &mut Scope, cond: ExprId, body: &BlockNode, origin: Origin) -> Option<LoweredExpr> {
        if let ExprKind::Let { pat, expr } = self.state.design.expr_node(cond).kind.clone() {
            return self.lower_while_let(scope, pat, expr, body, origin);
        }
        let cond = self.lower_expr(scope, cond)?;
        let body = self.lower_loop_body(scope, body)?;
        Some(self.emit(Expr::While { cond, body }, ValueType::Unit, origin))
    }

    /// The statements of a loop body, lowered with the frame of a loop whose value has type
    /// `value` pushed (`break` reads it).
    pub(super) fn lower_loop_body(&mut self, scope: &mut Scope, body: &BlockNode) -> Option<Vec<LoweredStmt>> {
        scope.values.loops.push(LoopFrame);
        let raised = scope.values.inline_exits.last().map(|frame| frame.flag_uses);
        let lowered = self.lower_statements(scope, &body.stmts);
        scope.values.loops.pop();
        if let (Some(before), Some(frame)) = (raised, scope.values.inline_exits.last_mut()) {
            frame.loop_flagged |= frame.flag_uses > before;
        }
        let (mut stmts, tail) = lowered?;
        if let Some(tail) = tail {
            stmts.push(self.stmt(Stmt::Expr(tail)));
        }
        Some(stmts)
    }

    /// `loop { body }`: its value is what its `break`s carry, of the type the checker gave (`ty`).
    pub(super) fn lower_loop(&mut self, scope: &mut Scope, body: &BlockNode, ty: ValueType, broke_never: bool, origin: Origin) -> Option<LoweredExpr> {
        let body = self.lower_loop_body(scope, body)?;
        let looped = self.emit(Expr::Loop { body: body.clone() }, ty, origin);
        // a `loop` with no `break` never gives a value: it fits any type its position needs
        if broke_never {
            self.note_exit(looped, Expr::Loop { body });
        }
        Some(looped)
    }

    /// `for k in low..high { body }` (or `..=`) over an integer range: a `ForRange`.
    pub(super) fn lower_for(&mut self, scope: &mut Scope, pat: PatId, iter: ExprId, body: &BlockNode, origin: Origin) -> Option<LoweredExpr> {
        let node = self.state.design.expr_node(self.peel_parens(iter)).clone();
        // an array, an iterator chain or a named range runs as the loop of `iterators.rs`
        let ExprKind::Range { lo: Some(lo), hi: Some(hi), inclusive } = node.kind else {
            return self.lower_sequence_for(scope, pat, iter, body, origin);
        };
        let (low, high) = (self.lower_expr(scope, lo)?, self.lower_expr(scope, hi)?);
        let (variable, binding) = self.for_variable(scope, pat)?;
        scope.values.locals[variable as usize].ty = self.type_of(low);
        let copy = self.assignable_copy(scope, binding, variable);
        let mut body = self.lower_loop_body(scope, body)?;
        body.splice(0..0, copy);
        Some(self.emit(Expr::ForRange { local: variable, low, high, inclusive, reverse: false, body }, ValueType::Unit, origin))
    }

    /// The local the pattern of a `for` binds, never assignable (the loop writes it), and the name
    /// it binds: a name, or a fresh local for `_`.
    fn for_variable(&mut self, scope: &mut Scope, pat: PatId) -> Option<(u32, Option<DeclId>)> {
        let node = self.state.design.pat_node(pat).clone();
        match node.kind {
            PatKind::Ident { binding, sub: None, .. } => Some((self.bind_local_as(scope, binding, false)?, Some(binding))),
            PatKind::Wild => {
                scope.values.locals.push(Local { name: "_".to_string(), ty: ValueType::Unit, mutable: false });
                Some((scope.values.locals.len() as u32 - 1, None))
            }
            _ => {
                self.refuse(scope.values.file, node.span, ValueForm::ForPatternThatIsNotName);
                None
            }
        }
    }

    /// `break` or `break value`: leaves the innermost loop.
    pub(super) fn lower_break(&mut self, scope: &mut Scope, value: Option<ExprId>, span: crate::msl::Span, origin: Origin) -> Option<LoweredExpr> {
        if scope.values.loops.is_empty() {
            self.refuse(scope.values.file, span, ValueForm::BreakOutsideLoop);
            return None;
        }
        let value = match value {
            Some(value) => Some(self.lower_expr(scope, value)?),
            None => None,
        };
        Some(self.emit_exit(Expr::Break(value), origin))
    }

    /// `continue`: starts the innermost loop's next iteration.
    pub(super) fn lower_continue(&mut self, origin: Origin) -> LoweredExpr {
        self.emit_exit(Expr::Continue, origin)
    }
}
