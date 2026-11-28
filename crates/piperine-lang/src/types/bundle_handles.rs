//! Functions on a bundle (feature `resolved-nets-bundles`, BN-05; spec part II §7 rule 13): in `impl Diff { fn vd(self) -> f64 }`
//! `self` is a read-only handle to the bundle the caller names. Its type is the bundle's `struct` ([`Ty::Struct`], the one
//! place a bundle is named as a type: it is no value), and a field of it (`self.p`) is the carrier or node the field
//! declares, read through the handle: read, probed and passed on, never written (`E0133`, §3.1.5).

use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, Resolution, TypeId, TypeKind};

use super::check::Checker;
use super::Ty;

impl Checker<'_> {
    /// The type of the bundle a type written as `Stream<u8>` or `flip Stream` names: its `struct`, with the arguments it is
    /// written with; of an array of bundles the array of it. `None` after a diagnostic.
    pub(super) fn bundle_type(&mut self, ty: TypeId) -> Option<Ty> {
        let node = self.state.design.type_node(ty).clone();
        match node.kind {
            TypeKind::Flip(inner) => self.bundle_type(inner),
            // an array of bundles (`[flip Stream<T>; N]`): the array of the element's bundle type, its length as any array's
            TypeKind::Array { elem, len } => {
                let element = self.bundle_type(elem)?;
                let file = self.state.design.decl_data(node.owner).file;
                let length = self.expressions().array_length(file, node.span, len)?;
                Some(Ty::Array(Box::new(element), length))
            }
            _ => {
                let outer = std::mem::replace(&mut self.state.modules.naming_bundle, true);
                let lowered = self.lower_type(ty);
                self.state.modules.naming_bundle = outer;
                lowered
            }
        }
    }

    /// Whether `ty` is the type of a bundle handle: the `struct` of a bundle.
    pub(super) fn is_bundle_type(&self, ty: &Ty) -> bool {
        matches!(ty, Ty::Struct(nominal) if matches!(&self.state.design.decl_data(nominal.decl).kind, DeclKind::Struct(data) if data.bundle == Some(true)))
    }

    /// The bundle a name stands for, as a receiver or the base of a field: a bundle port or signal of the module, a handle `self`,
    /// a nested bundle field of either, an element of an array of bundles. `None` for any other expression.
    pub(super) fn bundle_named_type(&mut self, expr: ExprId) -> Option<Ty> {
        match self.state.design.expr_node(expr).kind.clone() {
            ExprKind::Paren(inner) => self.bundle_named_type(inner),
            ExprKind::Path(path) => {
                let Some(Resolution::Decl(decl)) = self.state.design.path_node(path).segments.last().map(|segment| segment.res) else { return None };
                if self.state.design.is_bundle_owner(decl) || self.state.design.is_bundle_group(decl) {
                    return self.declared_bundle_type(decl);
                }
                self.handle_type(decl).filter(|ty| self.is_bundle_type(ty))
            }
            // a nested group, or an element of an array of bundles (`lanes[k]`): the bundle type of the element
            ExprKind::Field { .. } | ExprKind::Index { .. } if let Some(place) = self.state.design.bundle_named(expr) => {
                self.element_indexes(&place);
                self.declared_bundle_type(place.decl)?.element_at(place.elements.len())
            }
            ExprKind::Field { .. } if self.state.design.bundle_field(expr).is_some() => None,
            ExprKind::Field { .. } => match self.bundle_handle_field(expr) {
                Some((_, ty)) if self.is_bundle_type(&ty) => Some(ty),
                _ => None,
            },
            _ => None,
        }
    }

    /// The bundle type a port, signal or group `decl` is declared with.
    fn declared_bundle_type(&mut self, decl: DeclId) -> Option<Ty> {
        let ty = match &self.state.design.decl_data(decl).kind {
            DeclKind::Port(port) => port.ty,
            DeclKind::Signal(signal) => signal.ty,
            _ => return None,
        };
        self.bundle_type(ty)
    }

    /// The field `expr` (`self.valid`, `self.inner.x`) of a bundle handle, and its type: the carrier or node the field
    /// declares with the arguments of the handle's type substituted, or a bundle type for a nested bundle. `None` when `expr`
    /// is no field of a bundle handle, or after a diagnostic.
    pub(super) fn bundle_handle_field(&mut self, expr: ExprId) -> Option<(DeclId, Ty)> {
        let ExprKind::Field { expr: receiver, name, .. } = self.state.design.expr_node(expr).kind.clone() else { return None };
        let Some(Ty::Struct(nominal)) = self.bundle_named_type(receiver).filter(|ty| self.is_bundle_type(ty)) else { return None };
        let field = self.state.design.decl_data(nominal.decl).children.iter().copied().find(|child| self.state.design.decl_data(*child).name.as_ref().is_some_and(|n| n.text == name.text))?;
        let DeclKind::Field(written) = self.state.design.decl_data(field).kind.clone() else { return None };
        let substitution = self.generics().nominal_substitution(&nominal);
        let ty = if self.state.design.bundle_struct_of_type(written.ty).is_some() { self.bundle_type(written.ty)? } else { self.port_type(written.ty)? };
        Some((field, substitution.apply(&ty)))
    }

    /// The handle `self` a field chain `self.inner.x` starts at, when it starts at a bundle handle.
    pub(super) fn bundle_handle_root(&self, expr: ExprId) -> Option<DeclId> {
        match self.state.design.expr_node(expr).kind.clone() {
            ExprKind::Paren(inner) => self.bundle_handle_root(inner),
            ExprKind::Field { expr: receiver, .. } => self.bundle_handle_root(receiver),
            ExprKind::Path(path) => match self.state.design.path_node(path).segments.last().map(|segment| segment.res) {
                Some(Resolution::Decl(decl)) if self.handle_type(decl).is_some_and(|ty| self.is_bundle_type(&ty)) => Some(decl),
                _ => None,
            },
            _ => None,
        }
    }

    /// The text of the field chain `expr` (`self.inner.x`), for a diagnostic.
    pub(super) fn field_chain_text(&self, expr: ExprId) -> String {
        match &self.state.design.expr_node(expr).kind {
            ExprKind::Paren(inner) => self.field_chain_text(*inner),
            ExprKind::Field { expr: receiver, name, .. } => format!("{}.{}", self.field_chain_text(*receiver), name.text),
            ExprKind::Path(path) => self.state.design.path_node(*path).segments.last().map(|segment| segment.name.text.clone()).unwrap_or_default(),
            _ => String::new(),
        }
    }
}
