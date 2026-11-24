//! The carriers and nodes a bundle port or signal stands for (spec part II §7 rule 14: a bundle has no
//! semantics of its own, it is its fields). The POM keeps the bundle as written; this side table, filled
//! by the stage `resolve::leaves`, names the declaration each field of it becomes, so the checker and the
//! elaborator type and place every field like a port or a signal of its own. A leaf is a `Port` or `Signal`
//! declaration of the arena that is **no child** of its module: `children` stays the authored list and
//! reflection never shows a leaf.

use serde::{Deserialize, Serialize};

use super::body::{ExprKind, TypeKind};
use super::decl::Dir;
use super::design::Design;
use super::ids::{DeclId, ExprId, TypeId};
use super::resolution::Resolution;

/// The way one module sees a bundle (§7, the use table).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BundleView {
    /// `tx: Stream`: drives the `out` fields, reads the `in` fields.
    Producer,
    /// `rx: flip Stream`: every direction of a one-driver field reversed.
    Consumer,
    /// `in s: Stream`: every one-driver field an input.
    Observer,
}

impl BundleView {
    /// The view of a port written `dir name: ty` (§7 rules 3 to 5): `in` an observer, `flip` a consumer, else a producer.
    pub(crate) fn of_port(design: &Design, dir: Option<Dir>, ty: TypeId) -> BundleView {
        match (dir, &design.type_node(ty).kind) {
            (Some(Dir::In), _) => BundleView::Observer,
            (_, TypeKind::Flip(_)) => BundleView::Consumer,
            _ => BundleView::Producer,
        }
    }

    /// The view of a field written `flip T`, or of the type of a port written `flip`: producer and consumer swap,
    /// an observer stays one (§7 rule 5).
    pub fn flipped(self) -> BundleView {
        match self {
            BundleView::Producer => BundleView::Consumer,
            BundleView::Consumer => BundleView::Producer,
            BundleView::Observer => BundleView::Observer,
        }
    }

    /// The direction a field written with `written` has under this view (§7 rules 3 to 6): a field without
    /// direction has none in any view.
    pub fn field_direction(self, written: Option<Dir>) -> Option<Dir> {
        let written = written?;
        match self {
            BundleView::Producer => Some(written),
            BundleView::Consumer => Some(if written == Dir::Out { Dir::In } else { Dir::Out }),
            BundleView::Observer => Some(Dir::In),
        }
    }
}

/// One field of a bundle port or signal, as a declaration of the arena.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct BundleLeaf {
    /// The port or signal declaration standing for the field.
    pub(crate) decl: DeclId,
    /// The field's path under the bundle: `valid`, `inner.x`.
    pub(crate) path: String,
    /// Whether the field is a nested bundle (a group of leaves) and not a carrier or a node itself.
    pub(crate) nested: bool,
}

/// The leaves of every bundle port and signal of a design, in declaration order of the owners.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct BundleLeaves {
    /// The leaves each bundle owner has, depth first, a nested bundle before its own fields.
    pub(super) of_owner: std::collections::BTreeMap<DeclId, Vec<BundleLeaf>>,
    /// The bundle port or signal each leaf belongs to.
    pub(super) owner_of: std::collections::BTreeMap<DeclId, DeclId>,
}

impl Design {
    /// Records the leaves of the bundle port or signal `owner`.
    pub(super) fn add_bundle_leaves(&mut self, owner: DeclId, leaves: Vec<BundleLeaf>) {
        for leaf in &leaves {
            self.bundle_leaves.owner_of.insert(leaf.decl, owner);
        }
        self.bundle_leaves.of_owner.insert(owner, leaves);
    }

    /// The leaves of the bundle port or signal `owner`; empty for any other declaration.
    pub(crate) fn bundle_leaves(&self, owner: DeclId) -> &[BundleLeaf] {
        self.bundle_leaves.of_owner.get(&owner).map_or(&[], Vec::as_slice)
    }

    /// The bundle port or signal `leaf` is a field of, when it is a leaf.
    pub(crate) fn leaf_owner(&self, leaf: DeclId) -> Option<DeclId> {
        self.bundle_leaves.owner_of.get(&leaf).copied()
    }

    /// The leaf of the bundle `owner` at `path` (`valid`, `inner.x`).
    pub(crate) fn leaf_at(&self, owner: DeclId, path: &str) -> Option<DeclId> {
        self.bundle_leaves(owner).iter().find(|leaf| leaf.path == path).map(|leaf| leaf.decl)
    }

    /// The leaf at `relative` under `bundle`, a bundle port or signal or a nested group of one (`inner.x` under `tx`, `x` under
    /// the group `tx.inner`).
    pub(crate) fn leaf_under(&self, bundle: DeclId, relative: &str) -> Option<DeclId> {
        if self.is_bundle_owner(bundle) {
            return self.leaf_at(bundle, relative);
        }
        let owner = self.leaf_owner(bundle)?;
        let group = self.bundle_leaves(owner).iter().find(|leaf| leaf.decl == bundle)?;
        self.leaf_at(owner, &format!("{}.{relative}", group.path))
    }

    /// The ports and signals of `module` the checker and the elaborator type and place: its children, each bundle
    /// port or signal followed by the carriers and nodes it stands for (the groups of nested bundles are no carrier
    /// and no node, so they are left out).
    pub(crate) fn module_items(&self, items: &[DeclId]) -> Vec<DeclId> {
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            out.push(*item);
            out.extend(self.bundle_leaves(*item).iter().filter(|leaf| !leaf.nested).map(|leaf| leaf.decl));
        }
        out
    }

    /// Whether `decl` is the port or signal of a bundle owner: a bundle port or signal.
    pub(crate) fn is_bundle_owner(&self, decl: DeclId) -> bool {
        self.bundle_leaves.of_owner.contains_key(&decl)
    }

    /// Whether `decl` is the group a nested bundle field stands for: no carrier and no node of its own.
    pub(crate) fn is_bundle_group(&self, decl: DeclId) -> bool {
        self.leaf_owner(decl).is_some_and(|owner| self.bundle_leaves(owner).iter().any(|leaf| leaf.decl == decl && leaf.nested))
    }

    /// The leaf the field access `expr` stands for (`tx.valid`, `tx.inner.x`): the declaration of the field of the bundle port
    /// or signal its chain starts at. The expression's own resolution stays the struct's field, as written; `None` when
    /// `expr` is no field of a bundle port or signal (a field of a handle, of a value, of an instance).
    pub(crate) fn bundle_field(&self, expr: ExprId) -> Option<DeclId> {
        let mut path = Vec::new();
        let mut at = expr;
        loop {
            match &self.expr_node(at).kind {
                ExprKind::Paren(inner) if !path.is_empty() => at = *inner,
                ExprKind::Field { expr: receiver, name, .. } => {
                    path.push(name.text.as_str());
                    at = *receiver;
                    while let ExprKind::Paren(inner) = &self.expr_node(at).kind {
                        at = *inner;
                    }
                }
                ExprKind::Path(root) if !path.is_empty() => {
                    let Some(Resolution::Decl(owner)) = self.path_node(*root).segments.last().map(|segment| segment.res) else { return None };
                    path.reverse();
                    return self.leaf_at(owner, &path.join("."));
                }
                _ => return None,
            }
        }
    }

    /// Whether `expr` is the receiver of a method call (`rx` of `rx.beat(clk)`).
    pub(crate) fn is_method_receiver(&self, expr: ExprId) -> bool {
        let parent = self.expr_parent.get(expr.index()).copied().flatten();
        parent.is_some_and(|parent| matches!(&self.expr_node(parent).kind, ExprKind::MethodCall { recv, .. } if *recv == expr))
    }

    /// The bundle port, signal or nested group `expr` names (`s`, `o.inner`), through parentheses: what a connection passes.
    pub(crate) fn bundle_named(&self, expr: ExprId) -> Option<DeclId> {
        let mut expr = expr;
        while let ExprKind::Paren(inner) = &self.expr_node(expr).kind {
            expr = *inner;
        }
        match &self.expr_node(expr).kind {
            ExprKind::Path(path) => match self.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(decl)) if self.is_bundle_owner(decl) => Some(decl),
                _ => None,
            },
            ExprKind::Field { .. } => self.bundle_field(expr).filter(|leaf| self.is_bundle_group(*leaf)),
            _ => None,
        }
    }
}
