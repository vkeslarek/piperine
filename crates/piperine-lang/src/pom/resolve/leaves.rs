//! Stage 6b: the leaves of bundle ports and signals (spec part II §7 rules 3 to 7 and 14). Each port or
//! signal of a module whose type is a bundle gets one declaration per field, named by its dotted path
//! (`tx.valid`), typed as the field is, with the direction the module's view gives it. They are recorded in
//! [`BundleLeaves`](super::super::bundles::BundleLeaves), never in a module's `children` (the POM mirrors
//! the source).

use super::super::body::TypeKind;
use super::super::bundles::{BundleLeaf, BundleView};
use super::super::decl::{DeclData, DeclKind, Dir, Name, PortData, SignalData};
use super::super::design::Design;
use super::super::ids::{DeclId, TypeId};
use super::super::meta::Meta;
use super::super::resolution::{PortKind, Resolution};

/// A leaf the stage will add: its data and where it sits in its owner's bundle.
struct PlannedLeaf {
    data: DeclData,
    path: String,
    nested: bool,
}

/// The leaves to add to every bundle port and signal of a design.
pub(super) struct LeafPlan<'design> {
    design: &'design Design,
}

impl<'design> LeafPlan<'design> {
    /// The plan over a classified design.
    pub(super) fn new(design: &'design Design) -> Self {
        LeafPlan { design }
    }

    /// The leaves of every owner, in the order of the arena.
    fn plan(&self) -> Vec<(DeclId, Vec<PlannedLeaf>)> {
        let mut out = Vec::new();
        for index in 0..self.design.decl_count() {
            let owner = DeclId(index as u32);
            let data = self.design.decl_data(owner);
            let (ty, view) = match &data.kind {
                DeclKind::Port(port) => (port.ty, self.port_view(port.dir, port.ty)),
                DeclKind::Signal(signal) => (signal.ty, BundleView::Producer),
                _ => continue,
            };
            let Some(bundle) = self.bundle_of(ty) else { continue };
            // a port of the module, not of a function or a generation arm already unrolled elsewhere
            if !data.parent.is_some_and(|p| matches!(self.design.decl_data(p).kind, DeclKind::Module(_))) {
                continue;
            }
            let mut leaves = Vec::new();
            self.expand(owner, bundle, view, "", &mut leaves, 0);
            out.push((owner, leaves));
        }
        out
    }

    /// The view of a port: `in` an observer, `flip` a consumer, else a producer (§7 rules 3 to 5).
    fn port_view(&self, dir: Option<Dir>, ty: TypeId) -> BundleView {
        match (dir, &self.design.type_node(ty).kind) {
            (Some(Dir::In), _) => BundleView::Observer,
            (_, TypeKind::Flip(_)) => BundleView::Consumer,
            _ => BundleView::Producer,
        }
    }

    /// The bundle struct `ty` names, through `flip` only (an array of bundles is `composite-values` C's).
    fn bundle_of(&self, ty: TypeId) -> Option<DeclId> {
        let mut at = ty;
        while let TypeKind::Flip(inner) = &self.design.type_node(at).kind {
            at = *inner;
        }
        let TypeKind::Path(path) = &self.design.type_node(at).kind else { return None };
        match self.design.path_node(*path).segments.last().map(|segment| segment.res) {
            Some(Resolution::Decl(decl)) if matches!(&self.design.decl_data(decl).kind, DeclKind::Struct(s) if s.bundle == Some(true)) => Some(decl),
            _ => None,
        }
    }

    /// Appends the leaves of `bundle` seen with `view` under the prefix `prefix` of `owner`'s name; a nested bundle
    /// is a leaf of its own followed by its fields (a bundle that contains itself stops at a fixed depth: the checker
    /// reports it).
    fn expand(&self, owner: DeclId, bundle: DeclId, view: BundleView, prefix: &str, out: &mut Vec<PlannedLeaf>, depth: usize) {
        if depth > 8 {
            return;
        }
        let owner_data = self.design.decl_data(owner);
        let owner_name = owner_data.name.as_ref().map_or("", |name| name.text.as_str());
        for field in self.design.decl_data(bundle).children.clone() {
            let field_data = self.design.decl_data(field);
            let DeclKind::Field(written) = &field_data.kind else { continue };
            let Some(name) = field_data.name.as_ref() else { continue };
            let path = if prefix.is_empty() { name.text.clone() } else { format!("{prefix}.{}", name.text) };
            let nested = self.bundle_of(written.ty);
            let direction = if nested.is_some() { None } else { view.field_direction(written.dir) };
            let kind = match (&owner_data.kind, nested) {
                (DeclKind::Port(_), nested) => DeclKind::Port(PortData { dir: direction, ty: written.ty, default: None, kind: if nested.is_some() { PortKind::Bundle } else { PortKind::Carrier } }),
                _ => DeclKind::Signal(SignalData { ty: written.ty, init: None }),
            };
            let data = DeclData {
                kind,
                name: Some(Name { text: format!("{owner_name}.{path}"), raw: false, span: name.span }),
                parent: owner_data.parent,
                children: Vec::new(),
                file: owner_data.file,
                span: field_data.span,
                generics_span: None,
                public: owner_data.public,
                meta: Meta::default(),
            };
            out.push(PlannedLeaf { data, path: path.clone(), nested: nested.is_some() });
            if let Some(inner) = nested {
                let inner_view = if matches!(self.design.type_node(written.ty).kind, TypeKind::Flip(_)) { view.flipped() } else { view };
                self.expand(owner, inner, inner_view, &path, out, depth + 1);
            }
        }
    }
}

/// Adds the leaves of every bundle port and signal to the design.
pub(super) fn run(design: &mut Design) {
    let plan = LeafPlan::new(design).plan();
    for (owner, planned) in plan {
        let mut leaves = Vec::new();
        for leaf in planned {
            let decl = design.push_decl(leaf.data);
            leaves.push(BundleLeaf { decl, path: leaf.path, nested: leaf.nested });
        }
        design.add_bundle_leaves(owner, leaves);
    }
}
