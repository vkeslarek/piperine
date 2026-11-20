//! Mutation and control flow (part I §1.5.12, §1.7.5, §1.8): `let mut`, assignment, the deferred
//! `let x;`, loops and the exits that leave them.
//!
//! A `let mut` is a `mutable` IR local and an assignment is `Stmt::Assign`. In code that may see
//! an unknown (a reaction, a function variant with an argument that may be) a `let mut` scalar is
//! flagged whatever it starts as: an assignment under an unknown decision merges the written
//! value with the old one (§3.3.3), which may be unknown. Code with no unknown keeps plain locals.

use piperine_ir::{Expr, ExprId as LoweredExpr, Local, Origin, Place, Stmt, StmtId as LoweredStmt, ValueType};

use super::elaborator::Elaboration;
use super::known::Flag;
use super::lower::Scope;
use crate::pom::{DeclKind, ExprId, ExprKind, FileId, PatId, PatKind, Resolution};

impl Elaboration<'_, '_> {
    /// Whether the source wrote `let mut` (or `mut` in a pattern) for `binding`.
    pub(super) fn declared_mutable(&self, binding: crate::pom::DeclId) -> bool {
        let DeclKind::Binding(data) = &self.design.decl_data(binding).kind else { return false };
        matches!(&self.design.pat_node(data.pat).kind, PatKind::Ident { mutable: true, binding: named, .. } if *named == binding)
    }

    /// The type a `let mut` of base type `base` has in `scope`: flagged where the code may see an
    /// unknown (a flag rides on a scalar only).
    pub(super) fn mutable_type(scope: &Scope, base: ValueType) -> ValueType {
        let base = match base {
            ValueType::Flagged(inner) => *inner,
            other => other,
        };
        if scope.may_be_unknown { Flag::Flagged.wrap(base) } else { base }
    }

    /// `value` as a value of `target`'s flag: unchanged when both agree, `merge(v, v)` over a
    /// temporary when `target` may be unknown and `value` may not. `None` after a diagnostic is not
    /// needed here: a value that may be unknown never reaches a plain target (`assignable`).
    pub(super) fn coerced_to(&mut self, scope: &mut Scope, out: &mut Vec<LoweredStmt>, value: LoweredExpr, target: &ValueType) -> Option<LoweredExpr> {
        let (want, have) = (Flag::of(target), self.flag(value));
        if want == have || want == Flag::Plain {
            return Some(value);
        }
        let origin = self.expr_origins[value.index()].clone();
        let ty = self.type_of(value);
        let temporary = scope.locals.len() as u32;
        scope.locals.push(Local { name: format!("coerce${temporary}"), ty: ty.clone(), mutable: false });
        out.push(self.stmt(Stmt::Let { local: temporary, init: value }));
        let (a, b) = (self.emit(Expr::Local(temporary), ty.clone(), origin.clone()), self.emit(Expr::Local(temporary), ty.clone(), origin.clone()));
        Some(self.emit(Expr::Merge { a, b }, Flag::Flagged.wrap(ty), origin))
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
        let Some(local) = self.assigned_local(scope, lhs) else { return false };
        let target = scope.locals[local as usize].ty.clone();
        let Some(value) = self.lower_expr(scope, rhs) else { return false };
        if self.flag(value) == Flag::Flagged && Flag::of(&target) == Flag::Plain {
            self.unsupported(scope.file, node.span, "an assignment of a value that may be unknown to a local that cannot hold one");
            return false;
        }
        let Some(value) = self.coerced_to(scope, out, value, &target) else { return false };
        let op = op.map(Self::binary_op);
        out.push(self.stmt(Stmt::Assign { place: Place { local, path: Vec::new() }, op, value }));
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

    /// The IR local an assignment's target names: a `let mut` or deferred local. A field or an
    /// index target joins with the aggregate types.
    fn assigned_local(&mut self, scope: &Scope, target: ExprId) -> Option<u32> {
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
