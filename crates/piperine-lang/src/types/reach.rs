//! `inst.name` (part III §8.2.8): a read of a `pub` item of a placed child, typed as the item
//! itself: a node is a node, a carrier gives its value, a param its type, a `let` the type of its
//! value. An item that is not `pub` is `E0343` (the name resolver reports it); a commit or a
//! definition of a child's carrier is `E0222` (`definition.rs`). A field of anything else keeps
//! the `field access` refusal.

use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, Resolution};

use super::check::Checker;
use super::unsupported::Unsupported;
use super::Ty;

impl Checker<'_> {
    /// The type of the field read `expr`: the type of the child's item it names, or `None` after
    /// a diagnostic (`E0343` was reported at resolution for an item that is not `pub`).
    pub(super) fn field_read(&mut self, expr: ExprId) -> Option<Ty> {
        let node = self.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        let ExprKind::Field { expr: receiver, res, .. } = &node.kind else { return None };
        match res {
            Resolution::Decl(item) if self.is_child_item(*item) => {
                self.infer(*receiver)?;
                self.item_type(*item, file, node.span)
            }
            // A missing or private item already has its `E0343`.
            Resolution::Unresolved => None,
            _ => {
                self.unsupported(file, node.span, Unsupported::expr(&node.kind));
                None
            }
        }
    }

    /// Whether `item` is a port, a signal, a param or a `let` name of a module: what `inst.name`
    /// may name.
    pub(super) fn is_child_item(&self, item: DeclId) -> bool {
        match &self.design.decl_data(item).kind {
            DeclKind::Port(_) | DeclKind::Signal(_) | DeclKind::Param(_) => true,
            DeclKind::Binding(_) => self.design.decl_data(item).parent.is_some_and(|l| matches!(self.design.decl_data(l).kind, DeclKind::Let(_))),
            _ => false,
        }
    }

    /// The type a read of the child's `item` has.
    fn item_type(&mut self, item: DeclId, file: crate::pom::FileId, span: crate::msl::Span) -> Option<Ty> {
        match self.design.decl_data(item).kind.clone() {
            DeclKind::Port(p) => self.read_type(item, p.ty),
            DeclKind::Signal(s) => self.read_type(item, s.ty),
            DeclKind::Param(p) => self.lower_type(p.ty),
            _ => {
                let module = self.module_of_let(item)?;
                self.check_ahead(module);
                let found = self.locals.get(&item).cloned();
                if found.is_none() && !self.poisoned.contains(&item) {
                    self.unsupported(file, span, "a `let` of a child whose type is not known");
                }
                found
            }
        }
    }

    /// The module whose body holds the `let` that binds `binding`.
    fn module_of_let(&self, binding: DeclId) -> Option<DeclId> {
        let mut at = self.design.decl_data(binding).parent;
        while let Some(decl) = at {
            if matches!(self.design.decl_data(decl).kind, DeclKind::Module(_)) {
                return Some(decl);
            }
            at = self.design.decl_data(decl).parent;
        }
        None
    }
}
