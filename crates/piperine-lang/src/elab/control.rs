//! Mutation and control flow (part I §1.5.12, §1.7.5, §1.8): `let mut`, assignment, the deferred
//! `let x;`, loops and the exits that leave them.
//!
//! A local the body assigns is a `mutable` IR local, whatever the source wrote on its `let`
//! (every local is a place, §1.5.12), and an assignment is `Stmt::Assign`. In code that may see
//! an unknown (a reaction, a function variant with an argument that may be) an assigned scalar is
//! flagged whatever it starts as: an assignment under an unknown decision merges the written
//! value with the old one (§3.3.3), which may be unknown. Code with no unknown keeps plain locals.

use piperine_ir::{BinaryOp, Expr, ExprId as LoweredExpr, Local, Origin, Place, Stmt, StmtId as LoweredStmt, ValueType};

use super::elaborator::Elaboration;
use super::known::Flag;
use super::lower::Scope;
use crate::pom::{BlockNode, DeclId, ExprId, ExprKind, FileId, PatId, PatKind, Resolution};

/// One loop the code being lowered sits in.
pub(super) struct LoopFrame {
    /// The type of the loop's value: what its `break` values have, `()` for a `while` and a `for`.
    pub(super) value: ValueType,
}

impl Elaboration<'_, '_> {
    /// The type a `let mut` of base type `base` has in `scope`: flagged where the code may see an
    /// unknown (a flag rides on a scalar only).
    pub(super) fn mutable_type(scope: &Scope, base: ValueType) -> ValueType {
        let base = match base {
            ValueType::Flagged(inner) => *inner,
            other => other,
        };
        if scope.may_be_unknown { Flag::Flagged.wrap(base) } else { base }
    }

    /// `value` as a value of `target`'s flag: unchanged when both agree or `target` is plain,
    /// `{ let t = value; merge(t, t) }` when `target` may be unknown and `value` may not (a known
    /// value merged with itself is that value, §3.3.3, typed as one that may be unknown). A value
    /// that may be unknown never reaches a plain target: callers refuse it first.
    pub(super) fn coerced_to(&mut self, scope: &mut Scope, value: LoweredExpr, target: &ValueType) -> LoweredExpr {
        let (want, have) = (Flag::of(target), self.flag(value));
        if want == have || want == Flag::Plain {
            return value;
        }
        let origin = self.expr_origins[value.index()].clone();
        let ty = self.type_of(value);
        let temporary = scope.locals.len() as u32;
        scope.locals.push(Local { name: format!("coerce${temporary}"), ty: ty.clone(), mutable: false });
        let bind = self.stmt(Stmt::Let { local: temporary, init: value });
        let (a, b) = (self.emit(Expr::Local(temporary), ty.clone(), origin.clone()), self.emit(Expr::Local(temporary), ty.clone(), origin.clone()));
        let merged = self.emit(Expr::Merge { a, b }, Flag::Flagged.wrap(ty.clone()), origin.clone());
        self.emit(Expr::Block { stmts: vec![bind], value: Some(merged) }, Flag::Flagged.wrap(ty), origin)
    }

    /// `let x;` (§1.7.5): a local with no value yet. The checker proves it is assigned before it is
    /// read, so the IR local starts as the zero of its type (an unknown where it may hold one) and
    /// is assignable. `false` after a diagnostic.
    pub(super) fn lower_deferred_let(&mut self, scope: &mut Scope, out: &mut Vec<LoweredStmt>, pat: PatId, file: FileId, span: crate::msl::Span) -> bool {
        let PatKind::Ident { binding, sub: None, .. } = self.design.pat_node(pat).kind.clone() else {
            self.unsupported(file, span, "a deferred `let` whose pattern is not a name");
            return false;
        };
        let Some(local) = self.bind_local(scope, binding) else { return false };
        let ty = Self::mutable_type(scope, scope.locals[local as usize].ty.clone());
        let base = match &ty {
            ValueType::Flagged(inner) => (**inner).clone(),
            other => other.clone(),
        };
        let origin = self.locator.origin(file, span);
        let init = if Flag::of(&ty) == Flag::Flagged { Some(self.unknown_expr(base, origin)) } else { self.zero(&base, origin) };
        let Some(init) = init else {
            self.unsupported(file, span, "a deferred `let` of a type with no zero in the IR");
            return false;
        };
        let slot = &mut scope.locals[local as usize];
        slot.ty = ty;
        slot.mutable = true;
        out.push(self.stmt(Stmt::Let { local, init }));
        true
    }

    /// `place = value;` or `place op= value;` as a statement of `out`. `false` after a diagnostic.
    pub(super) fn lower_assignment(&mut self, scope: &mut Scope, out: &mut Vec<LoweredStmt>, expr: ExprId) -> bool {
        let node = self.design.expr_node(expr).clone();
        let ExprKind::Assign { op, lhs, rhs } = node.kind else { return false };
        let Some(place) = self.place_of(scope, lhs) else { return false };
        let Some(target) = self.place_type(scope, &place) else {
            self.unsupported(scope.file, node.span, "an assignment to a place whose type has no value in the IR");
            return false;
        };
        let Some(value) = self.lower_expr(scope, rhs) else { return false };
        if self.flag(value) == Flag::Flagged && Flag::of(&target) == Flag::Plain {
            self.unsupported(scope.file, node.span, "an assignment of a value that may be unknown to a local that cannot hold one");
            return false;
        }
        let value = self.coerced_to(scope, value, &target);
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
        let node = self.design.expr_node(self.peel_parens(target)).clone();
        let ExprKind::Path(path) = &node.kind else {
            self.unsupported(scope.file, node.span, "an assignment to a field or an element");
            return None;
        };
        let Some(Resolution::Decl(decl)) = self.design.path_node(*path).segments.last().map(|s| s.res) else {
            self.unsupported(scope.file, node.span, "an assignment to a name that is not a local");
            return None;
        };
        match scope.local_index.get(&decl) {
            Some(index) if scope.locals[*index as usize].mutable => Some(*index),
            _ => {
                self.unsupported(scope.file, node.span, "an assignment to a local that is not mutable here");
                None
            }
        }
    }
}

// ---- loops and the exits that leave them (§1.8.7 to §1.8.12) ------------------------------

impl Elaboration<'_, '_> {
    /// `while cond { body }`, whose condition is read before every iteration: type `()`.
    pub(super) fn lower_while(&mut self, scope: &mut Scope, cond: ExprId, body: &BlockNode, origin: Origin) -> Option<LoweredExpr> {
        if let ExprKind::Let { pat, expr } = self.design.expr_node(cond).kind.clone() {
            return self.lower_while_let(scope, pat, expr, body, origin);
        }
        let cond = self.lower_expr(scope, cond)?;
        let body = self.lower_loop_body(scope, body, ValueType::Unit)?;
        Some(self.emit(Expr::While { cond, body }, ValueType::Unit, origin))
    }

    /// The statements of a loop body, lowered with the frame of a loop whose value has type
    /// `value` pushed (`break` reads it).
    pub(super) fn lower_loop_body(&mut self, scope: &mut Scope, body: &BlockNode, value: ValueType) -> Option<Vec<LoweredStmt>> {
        scope.loops.push(LoopFrame { value });
        let lowered = self.lower_statements(scope, &body.stmts);
        scope.loops.pop();
        let (mut stmts, tail) = lowered?;
        if let Some(tail) = tail {
            stmts.push(self.stmt(Stmt::Expr(tail)));
        }
        Some(stmts)
    }

    /// `loop { body }`: its value is what its `break`s carry, of the type the checker gave
    /// (`base`), and may be unknown where the code may see unknowns.
    pub(super) fn lower_loop(&mut self, scope: &mut Scope, body: &BlockNode, base: ValueType, broke_never: bool, origin: Origin) -> Option<LoweredExpr> {
        let ty = if base == ValueType::Unit { base } else { Self::mutable_type(scope, base) };
        let body = self.lower_loop_body(scope, body, ty.clone())?;
        let looped = self.emit(Expr::Loop { body: body.clone() }, ty, origin);
        // a `loop` with no `break` never gives a value: it fits any type its position needs
        if broke_never {
            self.note_exit(looped, Expr::Loop { body });
        }
        Some(looped)
    }

    /// `for k in low..high { body }` (or `..=`) over an integer range. It stays a `ForRange`
    /// unless its bounds may be unknown or its body leaves early in code that may see unknowns:
    /// then it is the `while` that counts a mutable local, which the generated code can merge.
    pub(super) fn lower_for(&mut self, scope: &mut Scope, pat: PatId, iter: ExprId, body: &BlockNode, origin: Origin) -> Option<LoweredExpr> {
        let node = self.design.expr_node(self.peel_parens(iter)).clone();
        // an array, an iterator chain or a named range runs as the loop of `iterators.rs`
        let ExprKind::Range { lo: Some(lo), hi: Some(hi), inclusive } = node.kind else {
            return self.lower_sequence_for(scope, pat, iter, body, origin);
        };
        let (low, high) = (self.lower_expr(scope, lo)?, self.lower_expr(scope, hi)?);
        let (variable, binding) = self.for_variable(scope, pat)?;
        let counted = self.flags(&[low, high]) == Flag::Flagged || (scope.may_be_unknown && self.leaves_early(body));
        if counted && inclusive {
            self.unsupported(scope.file, body.span, "a `for` over `a..=b` whose bounds may be unknown or whose body leaves early");
            return None;
        }
        if counted {
            return self.lower_counting_loop(scope, (variable, binding), (low, high), body, origin);
        }
        scope.locals[variable as usize].ty = self.type_of(low);
        let copy = self.assignable_copy(scope, binding, variable);
        let mut body = self.lower_loop_body(scope, body, ValueType::Unit)?;
        body.splice(0..0, copy);
        Some(self.emit(Expr::ForRange { local: variable, low, high, inclusive, reverse: false, body }, ValueType::Unit, origin))
    }

    /// The local the pattern of a `for` binds, never assignable (the loop writes it), and the name
    /// it binds: a name, or a fresh local for `_`.
    fn for_variable(&mut self, scope: &mut Scope, pat: PatId) -> Option<(u32, Option<DeclId>)> {
        let node = self.design.pat_node(pat).clone();
        match node.kind {
            PatKind::Ident { binding, sub: None, .. } => Some((self.bind_local_as(scope, binding, false)?, Some(binding))),
            PatKind::Wild => {
                scope.locals.push(Local { name: "_".to_string(), ty: ValueType::Unit, mutable: false });
                Some((scope.locals.len() as u32 - 1, None))
            }
            _ => {
                self.unsupported(scope.file, node.span, "a `for` pattern that is not a name");
                None
            }
        }
    }

    /// Whether `body` holds a `break`, a `continue` or a `return` (one of an inner loop counts).
    fn leaves_early(&self, body: &BlockNode) -> bool {
        let mut inside = Vec::new();
        for expr in crate::pom::block_exprs(self.design, body) {
            crate::pom::preorder(self.design, expr, &mut inside);
        }
        inside.iter().any(|e| matches!(self.design.expr_node(*e).kind, ExprKind::Break(_) | ExprKind::Continue | ExprKind::Return(_)))
    }

    /// `for k in low..high { body }` as `{ let end = high; let mut next = low; while next < end { let k = next; next += 1; body } }`
    /// (`..=` is refused: its last step would overflow a bound at the type's maximum). The next
    /// value is taken before the body, so a `continue` still counts.
    fn lower_counting_loop(&mut self, scope: &mut Scope, (variable, binding): (u32, Option<DeclId>), (low, high): (LoweredExpr, LoweredExpr), body: &BlockNode, origin: Origin) -> Option<LoweredExpr> {
        let base = match self.type_of(low) {
            ValueType::Flagged(inner) => *inner,
            other => other,
        };
        let counter_ty = Self::mutable_type(scope, base.clone());
        let end_ty = self.type_of(high);
        let (end, next) = (scope.locals.len() as u32, scope.locals.len() as u32 + 1);
        scope.locals.push(Local { name: format!("for$end{end}"), ty: end_ty.clone(), mutable: false });
        scope.locals.push(Local { name: format!("for$next{next}"), ty: counter_ty.clone(), mutable: true });
        let low = self.coerced_to(scope, low, &counter_ty);
        let bind_end = self.stmt(Stmt::Let { local: end, init: high });
        let bind_next = self.stmt(Stmt::Let { local: next, init: low });
        let read = |this: &mut Self, local: u32, ty: &ValueType| this.emit(Expr::Local(local), ty.clone(), origin.clone());
        let (counter, bound) = (read(self, next, &counter_ty), read(self, end, &end_ty));
        let test_ty = self.flags(&[counter, bound]).wrap(ValueType::Bool);
        let cond = self.emit(Expr::Binary { op: BinaryOp::Lt, lhs: counter, rhs: bound }, test_ty, origin.clone());
        let current = read(self, next, &counter_ty);
        scope.locals[variable as usize].ty = counter_ty.clone();
        let take = self.stmt(Stmt::Let { local: variable, init: current });
        let one = self.emit(Expr::Int(1), base, origin.clone());
        let step = self.stmt(Stmt::Assign { place: Place { local: next, path: Vec::new() }, op: Some(BinaryOp::Add), value: one });
        let copy = self.assignable_copy(scope, binding, variable);
        let mut rest = self.lower_loop_body(scope, body, ValueType::Unit)?;
        rest.splice(0..0, [take, step].into_iter().chain(copy));
        let looped = self.emit(Expr::While { cond, body: rest }, ValueType::Unit, origin.clone());
        Some(self.emit(Expr::Block { stmts: vec![bind_end, bind_next], value: Some(looped) }, ValueType::Unit, origin))
    }

    /// `break` or `break value`: leaves the innermost loop. A value is coerced to the loop's type
    /// (flagged where the loop's is); it never has one a plain loop cannot hold.
    pub(super) fn lower_break(&mut self, scope: &mut Scope, value: Option<ExprId>, span: crate::msl::Span, origin: Origin) -> Option<LoweredExpr> {
        let Some(wanted) = scope.loops.last().map(|frame| frame.value.clone()) else {
            self.unsupported(scope.file, span, "a `break` outside a loop");
            return None;
        };
        let value = match value {
            Some(value) => {
                let lowered = self.lower_expr(scope, value)?;
                if self.flag(lowered) == Flag::Flagged && Flag::of(&wanted) == Flag::Plain {
                    self.unsupported(scope.file, self.design.expr_node(value).span, "a `break` value that may be unknown out of a loop that cannot hold one");
                    return None;
                }
                Some(self.coerced_to(scope, lowered, &wanted))
            }
            None => None,
        };
        Some(self.emit_exit(Expr::Break(value), origin))
    }

    /// `continue`: starts the innermost loop's next iteration.
    pub(super) fn lower_continue(&mut self, origin: Origin) -> LoweredExpr {
        self.emit_exit(Expr::Continue, origin)
    }
}
