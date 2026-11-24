//! The carriers and nodes a bundle port or signal stands for (spec part II §7 rule 14: a bundle has no
//! semantics of its own, it is its fields). The POM keeps the bundle as written; this side table, filled
//! by the stage `resolve::leaves`, names the declaration each field of it becomes, so the checker and the
//! elaborator type and place every field like a port or a signal of its own. A leaf is a `Port` or `Signal`
//! declaration of the arena that is **no child** of its module: `children` stays the authored list and
//! reflection never shows a leaf.

use serde::{Deserialize, Serialize};

use super::body::{ExprKind, TypeKind};
use super::decl::{DeclKind, Dir};
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
    /// The number of element levels of each owner: 0 for a bundle, 1 for `[Lane; N]`, 2 for `[[Lane; 2]; 3]`. Each leaf of
    /// such an owner is the array of its field with those levels outermost (struct of arrays).
    pub(super) element_levels: std::collections::BTreeMap<DeclId, usize>,
}

/// What a chain of fields and indexes (`tx.valid`, `rx[k].valid`, `lanes[1]`) names of a bundle port or signal: a leaf, a
/// nested group or the bundle itself, and the indexes of the element of an array of bundles it is in, outermost first (empty
/// for a single bundle). `tx[k].a` is the leaf `tx.a` at the element `[k]`: the element `k` of that array of carriers.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BundlePlace {
    /// The leaf, the group or the bundle port or signal itself.
    pub(crate) decl: DeclId,
    /// The element indexes, one per element level the chain names.
    pub(crate) elements: Vec<ExprId>,
}

/// How a person reads the name of one element of a port or signal: the subscript goes between `before` and `after`. An array
/// of carriers is `x` and the subscript (`x[2]`); a field of an array of bundles puts the element before the field
/// (`lanes[1].a`), the name the source writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElementNaming {
    /// What comes before the subscript.
    before: String,
    /// What comes after it.
    after: String,
}

impl ElementNaming {
    /// The name of the element (or the run of elements) `subscript` names (`[1]`, `[0..2]`, `[1][0]`; empty for the whole).
    pub(crate) fn name(&self, subscript: &str) -> String {
        format!("{}{subscript}{}", self.before, self.after)
    }
}

/// One step of a chain from its root name outward.
enum ChainStep {
    /// `.name`.
    Field(String),
    /// `[index]`.
    Index(ExprId),
}

impl Design {
    /// Records the leaves of the bundle port or signal `owner`, an array of bundles of `levels` element levels.
    pub(super) fn add_bundle_leaves(&mut self, owner: DeclId, leaves: Vec<BundleLeaf>, levels: usize) {
        for leaf in &leaves {
            self.bundle_leaves.owner_of.insert(leaf.decl, owner);
        }
        self.bundle_leaves.of_owner.insert(owner, leaves);
        self.bundle_leaves.element_levels.insert(owner, levels);
    }

    /// The leaves of the bundle port or signal `owner`; empty for any other declaration.
    pub(crate) fn bundle_leaves(&self, owner: DeclId) -> &[BundleLeaf] {
        self.bundle_leaves.of_owner.get(&owner).map_or(&[], Vec::as_slice)
    }

    /// The bundle port or signal `leaf` is a field of, when it is a leaf.
    pub(crate) fn leaf_owner(&self, leaf: DeclId) -> Option<DeclId> {
        self.bundle_leaves.owner_of.get(&leaf).copied()
    }

    /// The number of element levels of the bundle port or signal `decl` is, or is a leaf of: 0 for a single bundle and for any
    /// declaration that is no bundle.
    pub(crate) fn element_levels(&self, decl: DeclId) -> usize {
        let owner = self.leaf_owner(decl).unwrap_or(decl);
        self.bundle_leaves.element_levels.get(&owner).copied().unwrap_or(0)
    }

    /// How the elements of the port or signal `decl` are named: a leaf whose levels are all element levels of its array of
    /// bundles (`lanes.a: [Bit; 3]`) puts the subscript after the owner (`lanes[1].a`); any other declaration after its name
    /// (an array field of an array of bundles keeps its flat subscript: `tx.bits[5]`).
    pub(crate) fn element_naming(&self, decl: DeclId) -> ElementNaming {
        let name = self.decl_data(decl).name.as_ref().map_or("", |name| name.text.as_str()).to_string();
        let plain = ElementNaming { before: name, after: String::new() };
        let Some(owner) = self.leaf_owner(decl) else { return plain };
        let levels = self.element_levels(owner);
        if levels == 0 || self.array_levels_of(decl) != levels {
            return plain;
        }
        let owner_name = self.decl_data(owner).name.as_ref().map_or("", |name| name.text.as_str()).to_string();
        let path = self.bundle_leaves(owner).iter().find(|leaf| leaf.decl == decl).map_or(String::new(), |leaf| leaf.path.clone());
        ElementNaming { before: owner_name, after: format!(".{path}") }
    }

    /// The number of array levels the type of the port or signal `decl` is written with.
    fn array_levels_of(&self, decl: DeclId) -> usize {
        let ty = match &self.decl_data(decl).kind {
            DeclKind::Port(port) => port.ty,
            DeclKind::Signal(signal) => signal.ty,
            _ => return 0,
        };
        let mut levels = 0;
        let mut at = ty;
        while let TypeKind::Array { elem, .. } = &self.type_node(at).kind {
            levels += 1;
            at = *elem;
        }
        levels
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

    /// The leaf the field access `expr` stands for (`tx.valid`, `tx.inner.x`, `rx[k].valid`): the declaration of the field of the
    /// bundle port or signal its chain starts at. The expression's own resolution stays the struct's field, as written; `None`
    /// when `expr` is no field of a bundle port or signal (a field of a handle, of a value, of an instance). The element of an
    /// array of bundles the chain names is [`Design::bundle_place`]'s.
    pub(crate) fn bundle_field(&self, expr: ExprId) -> Option<DeclId> {
        self.bundle_place(expr).map(|place| place.decl).filter(|decl| self.leaf_owner(*decl).is_some())
    }

    /// What the chain `expr` names of a bundle port or signal of the module: a leaf, a group or the bundle (an element of an
    /// array of bundles, `lanes[1]`), with its element indexes.
    pub(crate) fn bundle_place(&self, expr: ExprId) -> Option<BundlePlace> {
        self.bundle_place_through(expr, &|root| Some(BundlePlace { decl: root, elements: Vec::new() }))
    }

    /// The name a chain of fields and indexes (`rx[k].valid`) starts at, through parentheses: `None` when it starts at
    /// anything else than a name.
    pub(crate) fn bundle_chain_root(&self, expr: ExprId) -> Option<DeclId> {
        let mut at = expr;
        loop {
            match &self.expr_node(at).kind {
                ExprKind::Paren(inner) => at = *inner,
                ExprKind::Field { expr: receiver, .. } => at = *receiver,
                ExprKind::Index { expr: base, .. } => at = *base,
                ExprKind::Path(root) => {
                    return match self.path_node(*root).segments.last().map(|segment| segment.res) {
                        Some(Resolution::Decl(named)) => Some(named),
                        _ => None,
                    };
                }
                _ => return None,
            }
        }
    }

    /// [`Design::bundle_place`] where the root name of the chain is first mapped by `root_bundle`: a name that stands for a
    /// bundle port, signal or group gives what it stands for (a handle `self` an inlined function was given is the place the
    /// call named, `elab/bundles.rs`), any other name `None`. Each index of the chain before its first field names an element
    /// of an array of bundles, one per element level; the fields then need every level named (`tx.a` of `tx: [Lane; N]`
    /// names no field), and an index after a field is the field's own, which the caller reads (`None` here).
    pub(crate) fn bundle_place_through(&self, expr: ExprId, root_bundle: &dyn Fn(DeclId) -> Option<BundlePlace>) -> Option<BundlePlace> {
        let mut steps = Vec::new();
        let mut at = expr;
        loop {
            match &self.expr_node(at).kind {
                ExprKind::Paren(inner) if !steps.is_empty() => at = *inner,
                ExprKind::Field { expr: receiver, name, .. } => {
                    steps.push(ChainStep::Field(name.text.clone()));
                    at = *receiver;
                }
                ExprKind::Index { expr: base, index } => {
                    steps.push(ChainStep::Index(*index));
                    at = *base;
                }
                ExprKind::Path(root) if !steps.is_empty() => {
                    let Some(Resolution::Decl(named)) = self.path_node(*root).segments.last().map(|segment| segment.res) else { return None };
                    steps.reverse();
                    return self.walk_chain(root_bundle(named)?, steps);
                }
                _ => return None,
            }
        }
    }

    /// The place the steps of a chain reach from `start`, a bundle port, signal or group (`None` for any other start).
    fn walk_chain(&self, start: BundlePlace, steps: Vec<ChainStep>) -> Option<BundlePlace> {
        if !self.is_bundle_owner(start.decl) && !self.is_bundle_group(start.decl) {
            return None;
        }
        let levels = self.element_levels(start.decl);
        let mut elements = start.elements;
        let mut fields: Vec<String> = Vec::new();
        for step in steps {
            match step {
                ChainStep::Index(index) if fields.is_empty() && elements.len() < levels => elements.push(index),
                ChainStep::Index(_) => return None,
                ChainStep::Field(name) => fields.push(name),
            }
        }
        if fields.is_empty() {
            return Some(BundlePlace { decl: start.decl, elements });
        }
        if elements.len() != levels {
            return None;
        }
        let decl = self.leaf_under(start.decl, &fields.join("."))?;
        Some(BundlePlace { decl, elements })
    }

    /// The `struct` a bundle type names, through `flip` (`Stream<u8>`, `flip Stream`): `None` for any other type.
    pub(crate) fn bundle_struct_of_type(&self, ty: TypeId) -> Option<DeclId> {
        let mut at = ty;
        while let TypeKind::Flip(inner) = &self.type_node(at).kind {
            at = *inner;
        }
        let TypeKind::Path(path) = &self.type_node(at).kind else { return None };
        match self.path_node(*path).segments.last().map(|segment| segment.res) {
            Some(Resolution::Decl(decl)) if matches!(&self.decl_data(decl).kind, DeclKind::Struct(s) if s.bundle == Some(true)) => Some(decl),
            _ => None,
        }
    }

    /// Whether `expr` is the receiver of a field access (`d` of `d.p`).
    pub(crate) fn is_field_base(&self, expr: ExprId) -> bool {
        let parent = self.expr_parent.get(expr.index()).copied().flatten();
        parent.is_some_and(|parent| matches!(&self.expr_node(parent).kind, ExprKind::Field { expr: base, .. } if *base == expr))
    }

    /// Whether `expr` is the receiver of a method call (`rx` of `rx.beat(clk)`).
    pub(crate) fn is_method_receiver(&self, expr: ExprId) -> bool {
        let parent = self.expr_parent.get(expr.index()).copied().flatten();
        parent.is_some_and(|parent| matches!(&self.expr_node(parent).kind, ExprKind::MethodCall { recv, .. } if *recv == expr))
    }

    /// The bundle port, signal, nested group or element of an array of bundles `expr` names (`s`, `o.inner`, `lanes[1]`), through
    /// parentheses: what a connection passes.
    pub(crate) fn bundle_named(&self, expr: ExprId) -> Option<BundlePlace> {
        let mut expr = expr;
        while let ExprKind::Paren(inner) = &self.expr_node(expr).kind {
            expr = *inner;
        }
        match &self.expr_node(expr).kind {
            ExprKind::Path(path) => match self.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(decl)) if self.is_bundle_owner(decl) => Some(BundlePlace { decl, elements: Vec::new() }),
                _ => None,
            },
            ExprKind::Field { .. } | ExprKind::Index { .. } => self.bundle_place(expr).filter(|place| self.is_bundle_owner(place.decl) || self.is_bundle_group(place.decl)),
            _ => None,
        }
    }
}
