//! `inst.name` (part III §8.2.8): a read of a `pub` item of a placed child, typed as the item
//! itself: a node is a node, a carrier gives its value, a param its type, a `let` the type of its
//! value. An item that is not `pub` is `E0343` (the name resolver reports it); a commit or a
//! definition of a child's carrier is `E0222` (`definition.rs`). A field of anything else keeps
//! the `field access` refusal.

use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, Resolution};

use super::check::Checker;
use super::element_target::{DriveTarget, Element};
use super::Ty;

impl Checker<'_> {
    /// The type of the field read `expr`: the type of the child's item it names, or `None` after
    /// a diagnostic (`E0343` was reported at resolution for an item that is not `pub`).
    pub(super) fn field_read(&mut self, expr: ExprId) -> Option<Ty> {
        let node = self.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        let ExprKind::Field { expr: receiver, name, res } = &node.kind else { return None };
        // a field of a bundle port or signal is the carrier or node its leaf stands for (§7 rule 12); the bundle is no value to read
        if let Some(leaf) = self.design.bundle_field(expr) {
            if self.design.is_bundle_group(leaf) {
                return self.bundle_as_value(expr);
            }
            return self.item_type(leaf, file, node.span);
        }
        // a field of a bundle handle (`self.p`): read through the handle (`bundle_handles.rs`)
        if let Some((_, ty)) = self.bundle_handle_field(expr) {
            return if self.is_bundle_type(&ty) && !self.design.is_method_receiver(expr) && !self.design.is_field_base(expr) { self.bundle_as_value(expr) } else { Some(Self::handle_read_type(&ty)) };
        }
        match res {
            Resolution::Decl(item) if self.is_child_item(*item) => {
                self.infer_receiver(*receiver)?;
                self.item_type(*item, file, node.span)
            }
            // A field of a value (§1.3.4): the type says whether it exists. A missing or private
            // item of a placed child already has its `E0343`.
            _ => self.field_of_value(expr, *receiver, name),
        }
    }

    /// The target `inst.name` of a definition or a commit when `item` is a resolved carrier of the child that this module may
    /// drive (§5.2.2: any module that sees a `pub` signal drives it, §22.3): the nature has an `impl Resolve`, the item is no
    /// `in` port and holds no array. `None` leaves the field to `E0222`: a carrier with one driver is written by its own module.
    pub(super) fn reached_drive_target(&mut self, place: ExprId, item: DeclId) -> Option<DriveTarget> {
        let (declared, direction) = match self.design.decl_data(item).kind.clone() {
            DeclKind::Port(p) => (p.ty, p.dir),
            DeclKind::Signal(s) => (s.ty, None),
            _ => return None,
        };
        let Some(Ty::Carrier { nature, .. }) = self.port_type(declared) else { return None };
        if direction == Some(crate::msl::ast::Dir::In) || !super::nature_rules::NatureRules::of(self.design, nature).resolved {
            return None;
        }
        let ExprKind::Field { expr: receiver, .. } = self.design.expr_node(place).kind.clone() else { return None };
        self.infer_receiver(receiver)?;
        let held = self.read_type(item, declared)?;
        self.exprs[place.index()] = Some(held.clone());
        Some(DriveTarget { carrier: item, held, element: Element::Whole, reached: true })
    }

    /// The receiver of `inst.name`: an instance named there is the place a member is read from, not
    /// a value read (§8.2.14, so no `E0333`); any other receiver is inferred as usual.
    pub(super) fn infer_receiver(&mut self, receiver: ExprId) -> Option<Ty> {
        if let ExprKind::Path(path) = &self.design.expr_node(receiver).kind {
            if let Some(Resolution::Decl(decl)) = self.design.path_node(*path).segments.last().map(|s| s.res) {
                // a placement in generation is the array of its instances; inside its loops the name is the iteration's (§8.2.23)
                let local = self.locals.get(&decl).cloned().map(|whole| self.generated_local(receiver, decl, whole));
                if let Some(instance @ Ty::Module(_)) = local {
                    return Some(instance);
                }
            }
        }
        self.infer(receiver)
    }

    /// The param `recv` names (through parentheses) and its type: a param of the module being
    /// checked, `f64` or `bool`.
    pub(super) fn param_receiver(&mut self, recv: ExprId) -> Option<(DeclId, Ty)> {
        match &self.design.expr_node(recv).kind {
            ExprKind::Paren(inner) => self.param_receiver(*inner),
            ExprKind::Path(path) => {
                let Some(Resolution::Decl(decl)) = self.design.path_node(*path).segments.last().map(|s| s.res) else { return None };
                let DeclKind::Param(p) = self.design.decl_data(decl).kind.clone() else { return None };
                self.lower_type(p.ty).map(|ty| (decl, ty))
            }
            _ => None,
        }
    }

    /// `r.changed()` (an `Event`: the param changed by a host `set`) or `r.prev()` (the param's value
    /// before its latest `set`, its own value before any) on the param `recv` (§5.3.1). A param is
    /// known, so `prev()` has the param's type and is not flagged.
    pub(super) fn param_history(&mut self, expr: ExprId, changed: bool, recv: ExprId, args: &[crate::pom::Arg]) -> Option<Ty> {
        let (_, ty) = self.param_receiver(recv)?;
        self.apply(expr, &[], args)?;
        self.infer(recv);
        self.builtins.insert(expr, if changed { super::Builtin::Changed } else { super::Builtin::Prev });
        Some(if changed { Ty::Event } else { ty })
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
