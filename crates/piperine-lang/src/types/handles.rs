//! Handles (generics GN-11, GN-12, part I §3.1.5): a function parameter of a nature type, and `self` of an
//! `impl` on a nature, stands for the carrier the caller passes. A handle is read (a discrete one gives its
//! value), probed (`v(a)`, `temp(self)`), passed on to another function and has carrier methods called on it
//! (`a.prev()`); it is never returned or stored (`E0133`, `handles` rules below).
//!
//! The type of a handle is the type of the carrier: `Ty::Node` for a continuous nature, `Ty::Carrier` for a
//! discrete one. [`Types::handle`](super::Types::handle) says which parameters are handles.

use crate::msl::Span;
use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, Resolution, StmtKind};

use super::check::Checker;
use super::Bound;
use super::Ty;

impl Checker<'_> {
    /// What a read of a handle of type `ty` is: the value of a discrete carrier, the node itself.
    fn handle_read_type(ty: &Ty) -> Ty {
        match ty {
            Ty::Carrier { value, .. } => (**value).clone(),
            other => other.clone(),
        }
    }

    /// Whether a parameter of type `ty` is a handle: the type is a node or a carrier of a nature.
    pub(super) fn is_handle_type(ty: &Ty) -> bool {
        matches!(ty, Ty::Node(_) | Ty::Carrier { .. })
    }

    /// Whether `ty` is a type parameter bound by `Nature`: what a function generic over a nature takes (`fn f<N: Nature>(a: N)`).
    pub(super) fn is_nature_parameter(&self, ty: &Ty) -> bool {
        matches!(ty, Ty::Param(param) if self.bounds_of(*param).contains(&Bound::Nature))
    }

    /// Records the parameter `param` of type `ty` as a handle when `ty` is one: a node or a carrier of a nature, or a
    /// type parameter bound by `Nature`.
    pub(super) fn note_handle(&mut self, param: DeclId, ty: &Ty) {
        if Self::is_handle_type(ty) || self.is_nature_parameter(ty) {
            self.handles.insert(param);
        }
    }

    /// The type of the argument `expr` for a parameter generic over a nature: the carrier it names (a discrete one is
    /// the carrier, not the value it reads, so the call's nature is inferred from it), else the type it reads as.
    pub(super) fn infer_carrier(&mut self, expr: ExprId) -> Option<Ty> {
        let read = self.infer(expr)?;
        match self.carrier_named(expr) {
            Some(carrier @ Ty::Carrier { .. }) => {
                self.exprs[expr.index()] = Some(Self::handle_read_type(&carrier));
                Some(carrier)
            }
            _ => Some(read),
        }
    }

    /// The handle parameter `expr` names, through parentheses.
    pub(super) fn handle_named(&self, expr: ExprId) -> Option<DeclId> {
        match &self.design.expr_node(expr).kind {
            ExprKind::Paren(inner) => self.handle_named(*inner),
            ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|segment| segment.res) {
                Some(Resolution::Decl(decl)) if self.handles.contains(&decl) => Some(decl),
                _ => None,
            },
            _ => None,
        }
    }

    /// The expression of a handle that `expr` evaluates to: the handle's own path, or the one a block's tail, an
    /// `if` arm or a `match` arm yields (§3.1.5: storing a handle through a value position is storing it).
    pub(super) fn handle_held(&self, expr: ExprId) -> Option<ExprId> {
        if self.handle_named(expr).is_some() {
            return Some(expr);
        }
        match &self.design.expr_node(expr).kind {
            ExprKind::Block(block) => self.handle_in_block(block),
            ExprKind::If { then, else_, .. } => self.handle_in_block(then).or_else(|| else_.and_then(|other| self.handle_held(other))),
            ExprKind::Match { arms, .. } => arms.iter().find_map(|arm| self.handle_held(arm.body)),
            _ => None,
        }
    }

    /// The handle a block's tail expression yields, if it has a tail.
    fn handle_in_block(&self, block: &crate::pom::BlockNode) -> Option<ExprId> {
        match block.stmts.last().map(|stmt| &self.design.stmt_node(*stmt).kind) {
            Some(StmtKind::Expr { expr, semi: false }) => self.handle_held(*expr),
            _ => None,
        }
    }

    /// What a read of the handle `decl` is: the value of a discrete carrier (§3.1.6), the node itself for a
    /// continuous one. `None` when `decl` is not a handle.
    pub(super) fn handle_read(&self, decl: DeclId) -> Option<Ty> {
        if !self.handles.contains(&decl) {
            return None;
        }
        match self.locals.get(&decl)? {
            Ty::Carrier { value, .. } => Some((**value).clone()),
            node => Some(node.clone()),
        }
    }

    /// The carrier type of the handle `decl`, for the carrier methods and the arguments that pass it on.
    pub(super) fn handle_type(&self, decl: DeclId) -> Option<Ty> {
        self.handles.contains(&decl).then(|| self.locals.get(&decl).cloned()).flatten()
    }

    /// Whether every operand of a probe is a handle, which is what a function's probe may read (§3 rule 7).
    pub(super) fn probes_only_handles(&self, operands: &[ExprId]) -> bool {
        !operands.is_empty() && operands.iter().all(|operand| self.handle_named(*operand).is_some())
    }

    /// An argument for a parameter of a nature type: the carrier it names (a port, a signal or a handle of the
    /// same nature and value) is passed on as it is (§3.1.5); anything else is the plain mismatch.
    pub(super) fn handle_argument(&mut self, expr: ExprId, expected: &Ty) {
        let passed = match &self.design.expr_node(expr).kind {
            ExprKind::Path(_) | ExprKind::Paren(_) => self.carrier_named(expr),
            _ => None,
        };
        match passed {
            // the argument is read as the value the carrier holds; the elaborator binds the carrier itself (`elab/handles.rs`)
            Some(found) if found == *expected => self.exprs[expr.index()] = Some(Self::handle_read_type(&found)),
            _ => self.check(expr, expected),
        }
    }

    /// The type of the port, signal or handle `expr` names; `None` for any other expression.
    pub(super) fn carrier_named(&mut self, expr: ExprId) -> Option<Ty> {
        match self.declared_port(expr) {
            Some((_, ty)) => Some(ty),
            None => self.handle_named(expr).and_then(|decl| self.handle_type(decl)),
        }
    }

    /// `E0133` for every use of a handle in `function` that keeps it past a call (§3.1.5, D12): the result type of
    /// the function is a handle, a `return` names one, a `let`, a struct field, a tuple or array element or a closure
    /// holds one, or an assignment or a commit writes through one. Passing it as an argument is the use it is for.
    pub(super) fn check_handle_uses(&mut self, function: DeclId) {
        if self.handles.is_empty() {
            return;
        }
        let file = self.design.decl_data(function).file;
        let mut found: Vec<(Span, ExprId, &'static str)> = Vec::new();
        for index in 0..self.design.expr_count() as u32 {
            let expr = ExprId(index);
            let node = self.design.expr_node(expr);
            if node.owner != function {
                continue;
            }
            match &node.kind {
                ExprKind::Struct { fields, .. } => found.extend(fields.iter().filter(|field| self.handle_held(field.value).is_some()).map(|field| (field.span, field.value, "stored in a struct field"))),
                ExprKind::Tuple(items) => found.extend(items.iter().filter(|item| self.handle_held(**item).is_some()).map(|item| (self.design.expr_node(*item).span, *item, "stored in a tuple element"))),
                ExprKind::Array(items) => found.extend(items.iter().filter(|item| self.handle_held(**item).is_some()).map(|item| (self.design.expr_node(*item).span, *item, "stored in an array element"))),
                ExprKind::ArrayRepeat { elem, .. } if self.handle_held(*elem).is_some() => found.push((self.design.expr_node(*elem).span, *elem, "stored in an array element")),
                ExprKind::Return(Some(value)) if self.handle_held(*value).is_some() => found.push((self.design.expr_node(*value).span, *value, "returned")),
                ExprKind::Assign { lhs, .. } if self.handle_named(*lhs).is_some() => found.push((self.design.expr_node(*lhs).span, *lhs, "written through")),
                ExprKind::Closure { body, .. } => {
                    let mut inside = Vec::new();
                    crate::pom::preorder(self.design, *body, &mut inside);
                    found.extend(inside.into_iter().filter(|inner| self.handle_named(*inner).is_some()).map(|inner| (self.design.expr_node(inner).span, inner, "captured by a closure")));
                }
                _ => {}
            }
        }
        for index in 0..self.design.stmt_count() as u32 {
            let node = self.design.stmt_node(crate::pom::StmtId(index));
            if node.owner != function {
                continue;
            }
            match &node.kind {
                StmtKind::Let { init: Some(init), .. } if self.handle_held(*init).is_some() => found.push((self.design.expr_node(*init).span, *init, "stored in a `let`")),
                StmtKind::Commit { place, .. } if self.handle_named(*place).is_some() => found.push((self.design.expr_node(*place).span, *place, "written through")),
                _ => {}
            }
        }
        for (span, expr, how) in found {
            let expr = self.handle_held(expr).unwrap_or(expr);
            let name = self.handle_named(expr).and_then(|decl| self.design.decl_data(decl).name.as_ref().map(|name| name.text.clone())).unwrap_or_default();
            let message = format!("the handle `{name}` is {how}: a handle is read, probed, passed on and has carrier methods called on it, nothing else (§3.1.5)");
            self.coded(file, "E0133", span, message, "a handle kept past its call");
        }
    }

    /// `E0133` for a function whose result type is a handle (a node or a carrier of a nature).
    pub(super) fn check_handle_result(&mut self, function: DeclId, ret: Option<&Ty>) {
        let Some(ret) = ret.filter(|ty| Self::is_handle_type(ty)) else { return };
        let DeclKind::Fn(data) = &self.design.decl_data(function).kind else { return };
        let span = data.ret.map_or(self.design.decl_data(function).span, |ty| self.design.type_node(ty).span);
        let file = self.design.decl_data(function).file;
        let message = format!("a function returns `{}`, a handle: a handle is never returned (§3.1.5)", ret.name(self.design));
        self.coded(file, "E0133", span, message, "a handle returned");
    }
}
