//! Functions on a bundle (feature `resolved-nets-bundles`, BN-05; spec part II §7 rule 13): in `impl Diff { fn vd(self) -> f64 }`
//! `self` is a read-only handle to the bundle the caller names. Its type is the bundle's `struct` ([`Ty::Struct`], the one
//! place a bundle is named as a type: it is no value), and a field of it (`self.p`) is the carrier or node the field
//! declares, read through the handle: read, probed and passed on, never written (`E0133`, §3.1.5).

use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, Resolution, TypeId, TypeKind};

use super::check::Checker;
use super::Ty;

impl Checker<'_> {
    /// The type of the bundle a type written as `Stream<u8>` or `flip Stream` names: its `struct`, with the arguments it is
    /// written with. `None` after a diagnostic.
    pub(super) fn bundle_type(&mut self, ty: TypeId) -> Option<Ty> {
        let mut at = ty;
        while let TypeKind::Flip(inner) = &self.design.type_node(at).kind {
            at = *inner;
        }
        let outer = std::mem::replace(&mut self.naming_bundle, true);
        let lowered = self.lower_type(at);
        self.naming_bundle = outer;
        lowered
    }

    /// Whether `ty` is the type of a bundle handle: the `struct` of a bundle.
    pub(super) fn is_bundle_type(&self, ty: &Ty) -> bool {
        matches!(ty, Ty::Struct(nominal) if matches!(&self.design.decl_data(nominal.decl).kind, DeclKind::Struct(data) if data.bundle == Some(true)))
    }

    /// The bundle a name stands for, as a receiver or the base of a field: a bundle port or signal of the module, a handle `self`,
    /// a nested bundle field of either. `None` for any other expression.
    pub(super) fn bundle_named_type(&mut self, expr: ExprId) -> Option<Ty> {
        match self.design.expr_node(expr).kind.clone() {
            ExprKind::Paren(inner) => self.bundle_named_type(inner),
            ExprKind::Path(path) => {
                let Some(Resolution::Decl(decl)) = self.design.path_node(path).segments.last().map(|segment| segment.res) else { return None };
                if self.design.is_bundle_owner(decl) || self.design.is_bundle_group(decl) {
                    return self.declared_bundle_type(decl);
                }
                self.handle_type(decl).filter(|ty| self.is_bundle_type(ty))
            }
            ExprKind::Field { .. } => match self.design.bundle_field(expr) {
                Some(group) if self.design.is_bundle_group(group) => self.declared_bundle_type(group),
                Some(_) => None,
                None => match self.bundle_handle_field(expr) {
                    Some((_, ty)) if self.is_bundle_type(&ty) => Some(ty),
                    _ => None,
                },
            },
            _ => None,
        }
    }

    /// The bundle type a port, signal or group `decl` is declared with.
    fn declared_bundle_type(&mut self, decl: DeclId) -> Option<Ty> {
        let ty = match &self.design.decl_data(decl).kind {
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
        let ExprKind::Field { expr: receiver, name, .. } = self.design.expr_node(expr).kind.clone() else { return None };
        let Some(Ty::Struct(nominal)) = self.bundle_named_type(receiver).filter(|ty| self.is_bundle_type(ty)) else { return None };
        let field = self.design.decl_data(nominal.decl).children.iter().copied().find(|child| self.design.decl_data(*child).name.as_ref().is_some_and(|n| n.text == name.text))?;
        let DeclKind::Field(written) = self.design.decl_data(field).kind.clone() else { return None };
        let substitution = self.nominal_substitution(&nominal);
        let ty = if self.design.bundle_struct_of_type(written.ty).is_some() { self.bundle_type(written.ty)? } else { self.port_type(written.ty)? };
        Some((field, substitution.apply(&ty)))
    }

    /// The handle `self` a field chain `self.inner.x` starts at, when it starts at a bundle handle.
    pub(super) fn bundle_handle_root(&self, expr: ExprId) -> Option<DeclId> {
        match self.design.expr_node(expr).kind.clone() {
            ExprKind::Paren(inner) => self.bundle_handle_root(inner),
            ExprKind::Field { expr: receiver, .. } => self.bundle_handle_root(receiver),
            ExprKind::Path(path) => match self.design.path_node(path).segments.last().map(|segment| segment.res) {
                Some(Resolution::Decl(decl)) if self.handle_type(decl).is_some_and(|ty| self.is_bundle_type(&ty)) => Some(decl),
                _ => None,
            },
            _ => None,
        }
    }

    /// The text of the field chain `expr` (`self.inner.x`), for a diagnostic.
    pub(super) fn field_chain_text(&self, expr: ExprId) -> String {
        match &self.design.expr_node(expr).kind {
            ExprKind::Paren(inner) => self.field_chain_text(*inner),
            ExprKind::Field { expr: receiver, name, .. } => format!("{}.{}", self.field_chain_text(*receiver), name.text),
            ExprKind::Path(path) => self.design.path_node(*path).segments.last().map(|segment| segment.name.text.clone()).unwrap_or_default(),
            _ => String::new(),
        }
    }
}
