//! Stage 6: classification. Reads the resolved paths and fills the
//! classification slots: the kind of each port and the bundle flag of each
//! struct. The helpers on [`Design`] (`declared_type`, `nature_of`) are plain
//! reads of the resolved tree, shared with the reflection handles.

use std::collections::BTreeSet;

use super::super::body::TypeKind;
use super::super::decl::{DeclKind, FnParamData, GenericParamData};
use super::super::design::Design;
use super::super::ids::{DeclId, TypeId};
use super::super::resolution::{PortKind, Resolution};

/// What classification computed, not yet written into the design.
#[derive(Default)]
pub(super) struct Classified {
    pub port_kinds: Vec<(DeclId, PortKind)>,
    pub bundles: Vec<(DeclId, bool)>,
}

impl Design {
    /// The declared type of a port, param, signal, field, constant or
    /// function parameter; none for any other declaration.
    pub(crate) fn declared_type(&self, id: DeclId) -> Option<TypeId> {
        match &self.decl_data(id).kind {
            DeclKind::Port(p) => Some(p.ty),
            DeclKind::Param(p) => Some(p.ty),
            DeclKind::Signal(s) => Some(s.ty),
            DeclKind::Field(f) => Some(f.ty),
            DeclKind::Const(c) => Some(c.ty),
            DeclKind::FnParam(FnParamData::Named { ty }) => Some(*ty),
            DeclKind::GenericParam(GenericParamData::Const { ty, .. }) => Some(*ty),
            _ => None,
        }
    }

    /// The type under arrays and `flip`: the element a carrier type is made of.
    pub(crate) fn peel_type(&self, mut ty: TypeId) -> TypeId {
        loop {
            match &self.type_node(ty).kind {
                TypeKind::Array { elem, .. } | TypeKind::Flip(elem) => ty = *elem,
                _ => return ty,
            }
        }
    }

    /// What the path of a type names, under arrays and `flip`.
    pub(crate) fn type_resolution(&self, ty: TypeId) -> Resolution {
        match &self.type_node(self.peel_type(ty)).kind {
            TypeKind::Path(p) => self.path_node(*p).segments.last().map_or(Resolution::Unresolved, |s| s.res),
            _ => Resolution::Unresolved,
        }
    }

    /// The nature a type is made of, through arrays and `flip`.
    pub(crate) fn nature_of(&self, ty: TypeId) -> Option<DeclId> {
        match self.type_resolution(ty) {
            Resolution::Decl(d) if matches!(self.decl_data(d).kind, DeclKind::Nature(_)) => Some(d),
            _ => None,
        }
    }

    /// Is `decl` a generic parameter bounded by `Nature`?
    fn is_nature_generic(&self, decl: DeclId) -> bool {
        let DeclKind::GenericParam(GenericParamData::Type { bounds, .. }) = &self.decl_data(decl).kind else { return false };
        bounds.iter().any(|b| self.path_node(*b).segments.last().is_some_and(|s| s.name.text == "Nature"))
    }

    /// Is a type a carrier: a nature or a `Nature`-bounded generic, under arrays and `flip`?
    fn is_carrier_type(&self, ty: TypeId) -> bool {
        match self.type_resolution(ty) {
            Resolution::Decl(d) => matches!(self.decl_data(d).kind, DeclKind::Nature(_)) || self.is_nature_generic(d),
            _ => false,
        }
    }

    /// The `impl` blocks whose self type names `target`, in source order.
    pub(crate) fn impls_of(&self, target: DeclId) -> Vec<DeclId> {
        (0..self.decl_count())
            .map(|i| DeclId(i as u32))
            .filter(|id| match &self.decl_data(*id).kind {
                DeclKind::Impl(i) => self.type_resolution(i.self_ty) == Resolution::Decl(target),
                _ => false,
            })
            .collect()
    }
}

/// Classify a design whose names are resolved.
pub(super) fn run(design: &Design) -> Classified {
    let bundles = bundle_structs(design);
    let mut out = Classified::default();
    for i in 0..design.decl_count() {
        let id = DeclId(i as u32);
        match &design.decl_data(id).kind {
            DeclKind::Port(_) => out.port_kinds.push((id, port_kind(design, id, &bundles))),
            // SPEC_DEVIATION: the design says the slot is filled for every struct; only a bundle
            // is written (`Some(true)`), so a struct that is no bundle keeps `bundle() == None`.
            // Reason: the existing `pom_tree::struct_fields_and_enum_variants` asserts `None`
            // for a non-bundle struct, and a test is never changed by a worker; `is_bundle()` is
            // the question hosts ask and it answers the same either way.
            DeclKind::Struct(_) if bundles.contains(&id) => out.bundles.push((id, true)),
            _ => {}
        }
    }
    out
}

/// The structs with a carrier field, directly or through a bundle field: the
/// least fixpoint, so a cycle of structs ends and a carrier anywhere on it
/// makes every member a bundle.
fn bundle_structs(design: &Design) -> BTreeSet<DeclId> {
    let structs: Vec<DeclId> = (0..design.decl_count()).map(|i| DeclId(i as u32)).filter(|d| matches!(design.decl_data(*d).kind, DeclKind::Struct(_))).collect();
    let mut bundles = BTreeSet::new();
    loop {
        let before = bundles.len();
        for s in &structs {
            if bundles.contains(s) {
                continue;
            }
            let has_carrier = design.decl_data(*s).children.iter().filter_map(|c| design.declared_type(*c)).any(|ty| {
                design.is_carrier_type(ty) || matches!(design.type_resolution(ty), Resolution::Decl(d) if bundles.contains(&d))
            });
            if has_carrier {
                bundles.insert(*s);
            }
        }
        if bundles.len() == before {
            return bundles;
        }
    }
}

fn port_kind(design: &Design, port: DeclId, bundles: &BTreeSet<DeclId>) -> PortKind {
    let Some(ty) = design.declared_type(port) else { return PortKind::Unknown };
    if design.is_carrier_type(ty) {
        return PortKind::Carrier;
    }
    match design.type_resolution(ty) {
        Resolution::Decl(d) if bundles.contains(&d) => PortKind::Bundle,
        Resolution::Decl(_) | Resolution::Builtin(_) => PortKind::ValueInput,
        Resolution::Unresolved | Resolution::Deferred(_) => match &design.type_node(design.peel_type(ty)).kind {
            // Tuples and slices of values are values; `_` and a missing name tell nothing.
            TypeKind::Tuple(_) | TypeKind::Slice(_) | TypeKind::Ref { .. } => PortKind::ValueInput,
            _ => PortKind::Unknown,
        },
    }
}
