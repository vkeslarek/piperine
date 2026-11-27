//! Places and assignment (§1.5.12, §1.7.5, §1.7.7): `place = value`, `place op= value`, and the
//! locals declared without a value (`let x;`). Every local is a place: a name a `let` or the
//! pattern of a `match` arm, an `if let`, a `while let` or a `for` binds. A parameter of a function
//! or of a closure, `self`, a name a closure captures, a body `let`, a `const`, a param and a
//! carrier are not. An assignment to anything that is not a place, and a read of a local before
//! it is assigned, are `E0183`, naming the target.

use std::collections::{BTreeMap, BTreeSet};

use crate::msl::Span;
use crate::pom::{BinOp, BindingOrigin, DeclId, DeclKind, ExprId, ExprKind, FileId, Resolution};

use super::check::Checker;
use super::Ty;

/// The local a place is rooted in, and whether the place goes below it (a field or an element).
struct Root {
    local: DeclId,
    projected: bool,
}

/// What the checker records about places while it types a body (§1.5.12).
#[derive(Debug, Default)]
pub(super) struct Places {
    /// The spans of the closures whose body is being typed, the innermost last.
    closures: Vec<Span>,
    /// The locals some assignment of the body writes: the elaborator makes exactly these assignable.
    assigned: BTreeSet<DeclId>,
    /// Each assignment of each local, in the order they were typed: what decides whether its value depends on the run (§1.4.7).
    writes: BTreeMap<DeclId, Vec<Write>>,
}

/// One assignment of a local.
#[derive(Debug, Clone, Copy)]
pub(super) struct Write {
    /// The assignment expression, `place = value` or `place op= value`.
    pub(super) assignment: ExprId,
    /// The value it writes.
    pub(super) value: ExprId,
    /// Whether it is written inside a closure, which may run any number of times.
    pub(super) in_closure: bool,
}

impl Places {
    /// A closure whose body starts being typed.
    pub(super) fn enter_closure(&mut self, span: Span) {
        self.closures.push(span);
    }

    /// The closure whose body was being typed is done.
    pub(super) fn leave_closure(&mut self) {
        self.closures.pop();
    }

    /// Whether an assignment at `at` to a name declared at `declared` writes a name a closure
    /// captures: the assignment is inside a closure that the declaration is outside of (§1.5.14).
    fn is_captured(&self, declared: Span, at: Span) -> bool {
        self.closures.iter().any(|closure| closure.start <= at.start && at.end <= closure.end && !(closure.start <= declared.start && declared.end <= closure.end))
    }

    /// Records that a body assigns `local` the value `value` at the assignment `assignment`.
    fn record(&mut self, local: DeclId, assignment: ExprId, value: ExprId) {
        self.assigned.insert(local);
        let in_closure = !self.closures.is_empty();
        self.writes.entry(local).or_default().push(Write { assignment, value, in_closure });
    }

    /// The assignments of `local`, in the order they were typed.
    pub(super) fn writes_of(&self, local: DeclId) -> &[Write] {
        self.writes.get(&local).map_or(&[], Vec::as_slice)
    }

    /// The locals the checked bodies assign.
    pub(super) fn into_assigned(self) -> BTreeSet<DeclId> {
        self.assigned
    }
}

impl Checker<'_> {
    /// `place = value` or `place op= value`: the type `()` (§1.5.12). The value is typed before
    /// the place is marked assigned, so `x = x + 1` reads an unassigned `x` as the error it is.
    pub(super) fn assignment(&mut self, expr: ExprId, op: Option<BinOp>, lhs: ExprId, rhs: ExprId) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let target = self.assignable(file, lhs);
        let pending = target.as_ref().is_some_and(|root| !root.projected && self.state.values.deferred.contains(&root.local) && !self.state.result.locals.contains_key(&root.local));
        let open = target.as_ref().is_some_and(|root| !root.projected && self.open_literal_local(lhs) == Some(root.local));
        let place = match (op, pending) {
            (None, true) => None,
            (None, false) => self.place_type(lhs, target.as_ref()),
            // An open literal local is read by the operator, which lets the other operand fix it.
            (Some(_), _) if open => target.as_ref().and_then(|root| self.state.result.locals.get(&root.local).cloned()),
            (Some(_), _) => self.infer(lhs),
        };
        match (op, place) {
            // `total = k`: the value, when it has a type of its own, fixes the open local.
            (None, Some(_)) if open && !self.integer_literal_waits(rhs) => {
                if let Some(found) = self.infer(rhs) {
                    self.check(lhs, &found);
                }
            }
            (None, Some(place)) => self.check(rhs, &place),
            (None, None) => {
                // `let x;` takes the type of its first assignment.
                if let (Some(found), Some(root)) = (self.infer(rhs), &target) {
                    self.state.result.locals.insert(root.local, found);
                }
            }
            (Some(op), Some(place)) => {
                let found = self.binary(expr, op, lhs, rhs);
                let place = match &target {
                    Some(root) if open => self.state.result.locals.get(&root.local).cloned().unwrap_or(place),
                    _ => place,
                };
                if let Some(found) = found
                    && !self.compatible(expr, &found, &place)
                {
                    let span = self.state.design.expr_node(expr).span;
                    self.mismatch(file, span, &place, &found);
                }
            }
            (Some(_), None) => {
                self.infer(rhs);
            }
        }
        if let Some(root) = target {
            self.state.values.flow.assigned.assign(root.local);
            self.state.values.places.record(root.local, expr, rhs);
        }
        Some(Ty::Unit)
    }

    /// The type of the place `lhs` without reading it: a local's recorded type, or, below a local,
    /// the type of the whole expression.
    fn place_type(&mut self, lhs: ExprId, target: Option<&Root>) -> Option<Ty> {
        match target {
            Some(Root { local, projected: false }) => self.state.result.locals.get(local).cloned(),
            Some(_) => self.infer(lhs),
            None => None,
        }
    }

    /// The local `lhs` is rooted in, when `lhs` is a place that may be assigned; otherwise
    /// `E0183` naming the target, and `None`.
    fn assignable(&mut self, file: FileId, lhs: ExprId) -> Option<Root> {
        let span = self.state.design.expr_node(lhs).span;
        let Some((decl, projected)) = self.place_root(lhs) else {
            self.not_a_place(file, span, "the left side of an assignment is not a place".to_string());
            return None;
        };
        let data = self.state.design.decl_data(decl).clone();
        let name = data.name.as_ref().map_or("", |n| n.text.as_str()).to_string();
        let why = match &data.kind {
            DeclKind::Binding(binding) => match binding.origin {
                BindingOrigin::Local | BindingOrigin::MatchArm | BindingOrigin::For | BindingOrigin::IfLet if self.state.values.places.is_captured(data.span, span) => "a name a closure captures",
                BindingOrigin::Local | BindingOrigin::MatchArm | BindingOrigin::For | BindingOrigin::IfLet => return Some(Root { local: decl, projected }),
                BindingOrigin::ClosureParam => "a closure parameter",
                BindingOrigin::BodyLet => "a body `let`",
                BindingOrigin::GenFor => "a generation `for` binding",
            },
            DeclKind::FnParam(_) if name == "self" => "`self`, a copy of the receiver",
            DeclKind::FnParam(_) => "a function parameter",
            DeclKind::Param(_) => "a param",
            DeclKind::Port(_) if !self.state.result.carriers.contains_key(&decl) => "a port",
            DeclKind::Port(_) | DeclKind::Signal(_) => "a carrier",
            DeclKind::Const(_) => "a `const`",
            _ => "not a place",
        };
        self.not_a_place(file, span, format!("cannot assign to `{name}`: it is {why}, and only a local that a `let` or a pattern binds is a place (§1.5.12)"));
        None
    }

    /// The local a place expression is rooted in: through parentheses, fields and indexes to a name.
    fn place_root(&self, expr: ExprId) -> Option<(DeclId, bool)> {
        match &self.state.design.expr_node(expr).kind {
            ExprKind::Paren(inner) => self.place_root(*inner),
            ExprKind::Field { expr, .. } | ExprKind::Index { expr, .. } => self.place_root(*expr).map(|(local, _)| (local, true)),
            ExprKind::Path(path) => match self.state.design.path_node(*path).segments.last().map(|s| s.res) {
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
        if !self.state.values.deferred.contains(&local) || self.state.values.flow.assigned.definitely(local) {
            return false;
        }
        let name = self.state.design.decl_data(local).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
        self.coded(file, "E0183", span, format!("`{name}` is read before it is assigned on every path (§1.7.5)"), "read before assignment");
        true
    }
}
