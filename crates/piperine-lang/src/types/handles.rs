//! Handles (generics GN-11, GN-12, part I §3.1.5): a function parameter of a nature type, and `self` of an
//! `impl` on a nature, stands for the carrier the caller passes. A handle is read (a discrete one gives its
//! value), probed (`v(a)`, `temp(self)`), passed on to another function and has carrier methods called on it
//! (`a.prev()`); it is never returned or stored (`E0133`, T45).
//!
//! The type of a handle is the type of the carrier: `Ty::Node` for a continuous nature, `Ty::Carrier` for a
//! discrete one. [`Types::handle`](super::Types::handle) says which parameters are handles.

use crate::pom::{DeclId, ExprId, ExprKind, Resolution};

use super::check::Checker;
use super::Ty;

impl Checker<'_> {
    /// Whether a parameter of type `ty` is a handle: the type is a node or a carrier of a nature.
    pub(super) fn is_handle_type(ty: &Ty) -> bool {
        matches!(ty, Ty::Node(_) | Ty::Carrier { .. })
    }

    /// Records the parameter `param` of type `ty` as a handle when `ty` is one.
    pub(super) fn note_handle(&mut self, param: DeclId, ty: &Ty) {
        if Self::is_handle_type(ty) {
            self.handles.insert(param);
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
            Some(found) if found == *expected => self.exprs[expr.index()] = Some(expected.clone()),
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
}
