//! A bundle at a placement (feature `resolved-nets-bundles`, BN-03; spec part II §7 rules 8 and 14): a bundle port of the
//! placed module is given a bundle carrier of the parent (each field joins the field of the same name) or a literal
//! `Cell { wl: w, bl: b }` (each named field joins the carrier or node it is given). Each field of the port is one
//! carrier or terminal of the child, in the order of the port's leaves, the order `Design::module_items` gives the
//! child's own body.

use super::StructureLowering;
use piperine_ir::{CarrierWindow, ExprId as LoweredExpr, Origin, Terminal};

use crate::elab::digital::drive_overlaps::{DriveSite, DriveWay, ElementReach};
use crate::elab::digital::elements::Lookup;
use crate::elab::elaborator::Scope;
use crate::refusal::forms::DigitalForm;
use crate::pom::{BundlePlace, DeclId, DeclKind, Dir, ExprId, ExprKind, Resolution, StructClass};

/// What a placement binds at the ports of the child so far: the terminals of its node ports, the carriers of its carrier ports
/// and the window of an array of them each is given, each in port order.
pub(super) struct PortBindings {
    /// The terminal each node port is given.
    pub(in crate::elab) terminals: Vec<Terminal>,
    /// The carrier of the parent each carrier port is given.
    pub(in crate::elab) carriers: Vec<Option<u32>>,
    /// The part of an array of carriers each carrier port is given.
    pub(in crate::elab) windows: Vec<Option<CarrierWindow>>,
}

/// What one field of a bundle port is given.
#[derive(Clone)]
enum LeafSource {
    /// A carrier or node written at the field in a literal.
    Expression(ExprId),
    /// The leaf of a bundle carrier of the parent that has the field's name, at the element of an array of bundles the
    /// argument names (`lanes[k]` gives the element `k` of the leaf `lanes.a`).
    Leaf(BundlePlace),
}

impl StructureLowering<'_, '_, '_> {
    /// What the chain `expr` names of a bundle in `scope` (`tx.valid`, `rx[k]`, `self.valid` of a handle an inlined function was
    /// given, `handles.rs`, which is the place the call named, §7 rule 13): a leaf, a group or a bundle, with the element indexes
    /// of the array of bundles it is in, left for the array machinery to fold over the iteration `scope` is in.
    fn bundle_place_in(&mut self, scope: &Scope, expr: ExprId) -> Option<BundlePlace> {
        let root = self.state.design.bundle_chain_root(expr)?;
        let rooted = self.handled_bundle(scope, root);
        self.state.design.bundle_place_through(expr, &|_| rooted.clone())
    }

    /// The leaf the field access `expr` stands for in `scope` (`tx.valid`, `self.valid`, `rx[k].valid` at its element): the place
    /// of a carrier, a node or a group, never of a bundle port or signal itself.
    pub(in crate::elab) fn bundle_leaf(&mut self, scope: &Scope, expr: ExprId) -> Option<BundlePlace> {
        self.bundle_place_in(scope, expr).filter(|place| self.state.design.leaf_owner(place.decl).is_some())
    }

    /// The read of a field of an element of an array of bundles (`ps[k].b`): the element `k` of the leaf `ps.b`. `None` after a
    /// diagnostic.
    pub(in crate::elab) fn lower_bundle_element(&mut self, scope: &mut Scope, expr: ExprId, origin: Origin) -> Option<LoweredExpr> {
        match self.digital().element_lookup(scope, expr) {
            Lookup::Found(element) => Some(self.digital().carrier_element_read(scope, element, origin)),
            Lookup::Failed => None,
            Lookup::No => {
                self.elaboration.refuse(scope.values.file, self.state.design.expr_node(expr).span, DigitalForm::FieldOfElementOfArrayOfBundles);
                None
            }
        }
    }

    /// The bundle port, signal or group the name `root` stands for in `scope`: itself, or what the handle `root` is bound to.
    fn handled_bundle(&mut self, scope: &Scope, root: DeclId) -> Option<BundlePlace> {
        match scope.digital.handles.get(&root) {
            Some(bound) => self.named_bundle(scope, *bound),
            None => Some(BundlePlace { decl: root, elements: Vec::new() }),
        }
    }

    /// The bundle port, signal, group or element of an array of bundles the expression `expr` names in `scope` (`s`, `o.inner`,
    /// `rx[k]`, a handle bound to either).
    pub(super) fn named_bundle(&mut self, scope: &Scope, expr: ExprId) -> Option<BundlePlace> {
        let expr = self.peel_parens(expr);
        let named = match &self.state.design.expr_node(expr).kind {
            ExprKind::Path(path) => match self.state.design.path_node(*path).segments.last().map(|segment| segment.res) {
                Some(Resolution::Decl(decl)) => self.handled_bundle(scope, decl),
                _ => None,
            },
            ExprKind::Field { .. } | ExprKind::Index { .. } => self.bundle_place_in(scope, expr),
            _ => None,
        };
        named.filter(|place| self.state.design.is_bundle_owner(place.decl) || self.state.design.is_bundle_group(place.decl))
    }

    /// The fields of the bundle port `port` given by `arg`, joined to what `arg` gives them. `false` after a diagnostic.
    pub(super) fn bind_bundle_port(&mut self, scope: &Scope, instance: &str, port: DeclId, arg: ExprId, bound: &mut PortBindings) -> bool {
        let mut sources = Vec::new();
        self.leaf_sources(scope, arg, port, "", &mut sources);
        let mut ok = true;
        for leaf in self.state.design.bundle_leaves(port).to_vec() {
            if leaf.nested {
                continue;
            }
            // a field the argument leaves out was refused (`E0263`) before: nothing is bound for it
            let Some(source) = sources.iter().find(|(path, _)| *path == leaf.path).map(|(_, source)| source.clone()) else { continue };
            ok &= self.bind_leaf(scope, instance, leaf.decl, source, arg, bound);
        }
        ok
    }

    /// The nodes the leaf `from` of a bundle of the frame gives at the element its place names: its node, every node of an
    /// array-of-nodes field, or those of the element of an array of bundles (`lanes[k]` of `lanes.p: [Electrical; N]`), one
    /// terminal each, in order. `None` after a diagnostic.
    fn leaf_node_run(&mut self, scope: &Scope, from: &BundlePlace) -> Option<Vec<Terminal>> {
        let terminal = self.frame_node(scope.values.frame, from.decl)?;
        let Some(shape) = self.state.structure.frames.get(scope.values.frame).and_then(|frame| frame.node_shapes.get(&from.decl)).cloned() else { return Some(vec![terminal]) };
        let Terminal::Node(first) = terminal else { return None };
        let (offset, length) = self.digital().element_run(scope, &shape, &from.elements)?;
        Some((0..length as u32).map(|k| Terminal::Node(first + offset as u32 + k)).collect())
    }

    /// The window of the array carrier of the leaf `from` its element indexes name: the whole leaf with none, one element when
    /// they name every level, the run below them otherwise. `None` after a diagnostic.
    fn leaf_window(&mut self, scope: &Scope, from: &BundlePlace) -> Option<Option<CarrierWindow>> {
        if from.elements.is_empty() {
            return Some(None);
        }
        let (shape, _) = self.digital().carrier_array_shape(from.decl, scope.values.frame)?;
        let (first, length) = self.digital().element_run(scope, &shape, &from.elements)?;
        Some(Some(CarrierWindow { first, length: (from.elements.len() < shape.dims.len()).then_some(length) }))
    }

    /// The source of every field under `prefix` of the bundle port `port` that `arg` gives, by path.
    fn leaf_sources(&mut self, scope: &Scope, arg: ExprId, port: DeclId, prefix: &str, out: &mut Vec<(String, LeafSource)>) {
        if let Some(source) = self.named_bundle(scope, arg) {
            for leaf in self.state.design.bundle_leaves(port).iter().filter(|leaf| !leaf.nested) {
                let Some(rest) = leaf.path.strip_prefix(prefix) else { continue };
                if let Some(from) = self.state.design.leaf_under(source.decl, rest) {
                    out.push((leaf.path.clone(), LeafSource::Leaf(BundlePlace { decl: from, elements: source.elements.clone() })));
                }
            }
            return;
        }
        let ExprKind::Struct { fields, class: StructClass::BundleLiteral, .. } = &self.state.design.expr_node(arg).kind else { return };
        for field in fields {
            let path = if prefix.is_empty() { field.name.text.clone() } else { format!("{prefix}{}", field.name.text) };
            let nested = self.state.design.bundle_leaves(port).iter().any(|leaf| leaf.nested && leaf.path == path);
            if nested {
                self.leaf_sources(scope, field.value, port, &format!("{path}."), out);
            } else {
                out.push((path, LeafSource::Expression(field.value)));
            }
        }
    }

    /// One field of a bundle port: the carrier or terminal `source` gives the leaf `port`.
    fn bind_leaf(&mut self, scope: &Scope, instance: &str, port: DeclId, source: LeafSource, arg: ExprId, bound: &mut PortBindings) -> bool {
        let span = self.state.design.expr_node(arg).span;
        // an array of nodes as a field: its nodes in order, from the carrier of the parent or the nodes a literal names
        if self.state.types.node_array(port).is_some() {
            let nodes = match &source {
                LeafSource::Expression(expr) => self.digital().node_array_terminals(scope, *expr),
                LeafSource::Leaf(from) => self.leaf_node_run(scope, from),
            };
            let Some(nodes) = nodes else { return false };
            bound.terminals.extend(nodes);
            return true;
        }
        // a field that is an array of length 0 has no carrier in the child, so it binds nothing (§1.2.3)
        if self.state.types.carrier_array(port).is_some_and(crate::types::Ty::has_fixed_zero_length) {
            return true;
        }
        if self.state.types.carrier(port).is_none() {
            let terminal = match &source {
                LeafSource::Expression(expr) => self.analog().lower_terminal(scope, *expr),
                // a node field, or the node of the element of an array of bundles the argument names
                LeafSource::Leaf(from) => self.leaf_node_run(scope, from).and_then(|nodes| nodes.first().copied()),
            };
            let Some(terminal) = terminal else {
                self.refuse(scope.values.file, span, DigitalForm::FieldOfBundleThatIsNoNode);
                return false;
            };
            bound.terminals.push(terminal);
            return true;
        }
        let found = match &source {
            LeafSource::Expression(expr) => self.lower_carrier_argument(scope, instance, port, *expr),
            // the carrier of the leaf, or the window of it the element of an array of bundles names (struct of arrays)
            LeafSource::Leaf(from) => match self.state.structure.frames.get(scope.values.frame).and_then(|frame| frame.carriers.get(&from.decl)).copied() {
                Some(index) => self.leaf_window(scope, from).map(|window| (index, window)),
                None => None,
            },
        };
        let Some((carrier, window)) = found else {
            self.refuse(scope.values.file, span, DigitalForm::FieldOfBundleThatIsNoCarrier);
            return false;
        };
        if matches!(&self.state.design.decl_data(port).kind, DeclKind::Port(data) if data.dir == Some(Dir::Out)) {
            self.state.digital.structure.handed.push((carrier, window));
            let reach = ElementReach::of_window(window);
            self.state.digital.structure.drive_sites.push(DriveSite { carrier, reach, fields: Vec::new(), way: DriveWay::Handed, file: scope.values.file, span });
        }
        bound.carriers.push(Some(carrier));
        bound.windows.push(window);
        true
    }
}
