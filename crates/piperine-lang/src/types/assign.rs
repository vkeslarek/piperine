//! Places and assignment (§1.5.12, §1.7.5, §1.7.7): `let mut`, `place = value`, `place op= value`,
//! and the locals declared without a value (`let x;`). An assignment to anything that is not a
//! place, a read of a local before it is assigned, and a second assignment of a local without
//! `mut` are `E0183`, naming the target.

use crate::msl::Span;
use crate::pom::{BinOp, DeclId, DeclKind, ExprId, ExprKind, FileId, Resolution};

use super::check::Checker;
use super::Ty;

/// The local a place is rooted in, and whether the place goes below it (a field or an element).
struct Root {
    local: DeclId,
    projected: bool,
}

impl Checker<'_> {
    /// `place = value` or `place op= value`: the type `()` (§1.5.12). The value is typed before
    /// the place is marked assigned, so `x = x + 1` reads an unassigned `x` as the error it is.
    pub(super) fn assignment(&mut self, expr: ExprId, op: Option<BinOp>, lhs: ExprId, rhs: ExprId) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let target = self.assignable(file, lhs);
        let pending = target.as_ref().is_some_and(|root| !root.projected && self.deferred.contains_key(&root.local) && !self.locals.contains_key(&root.local));
        let place = match (op, pending) {
            (None, true) => None,
            (None, false) => self.place_type(lhs, target.as_ref()),
            (Some(_), _) => self.infer(lhs),
        };
        match (op, place) {
            (None, Some(place)) => self.check(rhs, &place),
            (None, None) => {
                // `let x;` takes the type of its first assignment.
                if let (Some(found), Some(root)) = (self.infer(rhs), &target) {
                    self.locals.insert(root.local, found);
                }
            }
            (Some(op), Some(place)) => {
                if let Some(found) = self.binary(expr, op, lhs, rhs)
                    && found != place
                {
                    let span = self.design.expr_node(expr).span;
                    self.mismatch(file, span, &place, &found);
                }
            }
            (Some(_), None) => {
                self.infer(rhs);
            }
        }
        if let Some(root) = target {
            self.flow.assigned.assign(root.local);
        }
        Some(Ty::Unit)
    }

    /// The type of the place `lhs` without reading it: a local's recorded type, or, below a local,
    /// the type of the whole expression.
    fn place_type(&mut self, lhs: ExprId, target: Option<&Root>) -> Option<Ty> {
        match target {
            Some(Root { local, projected: false }) => self.locals.get(local).cloned(),
            Some(_) => self.infer(lhs),
            None => None,
        }
    }

    /// The local `lhs` is rooted in, when `lhs` is a place that may be assigned; otherwise
    /// `E0183` naming the target, and `None`.
    fn assignable(&mut self, file: FileId, lhs: ExprId) -> Option<Root> {
        let span = self.design.expr_node(lhs).span;
        let Some((decl, projected)) = self.place_root(lhs) else {
            self.not_a_place(file, span, "the left side of an assignment is not a place".to_string());
            return None;
        };
        let data = self.design.decl_data(decl).clone();
        let name = data.name.as_ref().map_or("", |n| n.text.as_str()).to_string();
        let why = match &data.kind {
            DeclKind::Binding(_) | DeclKind::FnParam(_) if self.mutable.contains(&decl) => return Some(Root { local: decl, projected }),
            DeclKind::Binding(_) if self.deferred.contains_key(&decl) && !projected => return self.assignable_once(file, span, decl, &name),
            DeclKind::Binding(_) if data.parent.is_some_and(|p| matches!(self.design.decl_data(p).kind, DeclKind::Let(_))) => "a body `let`",
            DeclKind::Binding(_) => "a local declared without `mut`",
            DeclKind::FnParam(_) => "a function parameter",
            DeclKind::Param(_) => "a param",
            DeclKind::Port(_) if !self.carriers.contains_key(&decl) => "a port",
            DeclKind::Port(_) | DeclKind::Signal(_) => "a carrier",
            DeclKind::Const(_) => "a `const`",
            _ => "not a place",
        };
        self.not_a_place(file, span, format!("cannot assign to `{name}`: it is {why}, and only a local declared with `mut` is a place (§1.5.12)"));
        None
    }

    /// A local declared without a value and without `mut` is assigned at most once on any path
    /// (§1.7.5): not where it may already be assigned, and not from a loop it is declared outside.
    fn assignable_once(&mut self, file: FileId, span: Span, local: DeclId, name: &str) -> Option<Root> {
        let declared_at = self.deferred.get(&local).copied().unwrap_or_default();
        if self.flow.assigned.possibly(local) || self.flow.loop_depth != declared_at {
            let message = format!("`{name}` is declared without `mut` and may be assigned twice: such a local is assigned at most once on any path (§1.7.5)");
            self.coded(file, "E0183", span, message, "assigned twice");
            return None;
        }
        Some(Root { local, projected: false })
    }

    /// The local a place expression is rooted in: through parentheses, fields and indexes to a name.
    fn place_root(&self, expr: ExprId) -> Option<(DeclId, bool)> {
        match &self.design.expr_node(expr).kind {
            ExprKind::Paren(inner) => self.place_root(*inner),
            ExprKind::Field { expr, .. } | ExprKind::Index { expr, .. } => self.place_root(*expr).map(|(local, _)| (local, true)),
            ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(decl)) => Some((decl, false)),
                _ => None,
            },
            _ => None,
        }
    }

    /// `E0183` for an assignment target that is not a place.
    fn not_a_place(&mut self, file: FileId, span: Span, message: String) {
        self.coded(file, "E0183", span, message, "not a place");
    }

    /// `E0183` for a read of a local declared without a value before it is assigned (§1.7.5).
    pub(super) fn read_before_assignment(&mut self, file: FileId, span: Span, local: DeclId) -> bool {
        if !self.deferred.contains_key(&local) || self.flow.assigned.definitely(local) {
            return false;
        }
        let name = self.design.decl_data(local).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
        self.coded(file, "E0183", span, format!("`{name}` is read before it is assigned on every path (§1.7.5)"), "read before assignment");
        true
    }
}
