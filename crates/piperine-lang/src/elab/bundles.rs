//! A bundle at a placement (feature `resolved-nets-bundles`, BN-03; spec part II §7 rules 8 and 14): a bundle port of the
//! placed module is given a bundle carrier of the parent (each field joins the field of the same name) or a literal
//! `Cell { wl: w, bl: b }` (each named field joins the carrier or node it is given). Each field of the port is one
//! carrier or terminal of the child, in the order of the port's leaves, the order `Design::module_items` gives the
//! child's own body.

use std::collections::BTreeMap;

use piperine_ir::{CarrierWindow, Terminal};

use super::drive_overlaps::{DriveSite, DriveWay, ElementReach};
use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::pom::{DeclId, DeclKind, Dir, ExprId, ExprKind, Resolution, StructClass};

/// What a placement binds at the ports of the child so far: the terminals of its node ports, the carriers of its carrier ports
/// and the window of an array of them each is given, each in port order.
pub(super) struct PortBindings {
    /// The terminal each node port is given.
    pub(super) terminals: Vec<Terminal>,
    /// The carrier of the parent each carrier port is given.
    pub(super) carriers: Vec<Option<u32>>,
    /// The part of an array of carriers each carrier port is given.
    pub(super) windows: Vec<Option<CarrierWindow>>,
}

/// What one field of a bundle port is given.
#[derive(Clone, Copy)]
enum LeafSource {
    /// A carrier or node written at the field in a literal.
    Expression(ExprId),
    /// The leaf of a bundle carrier of the parent that has the field's name.
    Leaf(DeclId),
}

impl Elaboration<'_, '_> {
    /// The leaf the field access `expr` stands for in `scope`: `tx.valid` of a bundle port or signal of the frame, or `self.valid` of
    /// a handle an inlined function was given (`handles.rs`), which is the leaf of the bundle the call named (§7 rule 13). An index
    /// of an array of bundles (`rx[k].valid`) is folded over the loop variables of the iteration `scope` is in.
    pub(super) fn bundle_leaf(&mut self, scope: &Scope, expr: ExprId) -> Option<DeclId> {
        let root = self.design.bundle_chain_root(expr)?;
        let rooted = self.handled_bundle(scope, root);
        let folded = self.folded_bundle_indexes(scope, expr);
        self.design.bundle_field_through(expr, &|_| rooted, &|index| self.design.literal_index(index).or_else(|| folded.get(&index).copied()))
    }

    /// The value each index of the chain `expr` (`rx[k].valid`) that is not a written integer has in `scope`, where it folds.
    fn folded_bundle_indexes(&mut self, scope: &Scope, expr: ExprId) -> BTreeMap<ExprId, usize> {
        let bindings = self.frame_bindings(scope.frame).overlaid(&scope.unrolled);
        let mut folded = BTreeMap::new();
        for index in self.design.bundle_chain_indexes(expr) {
            if self.design.literal_index(index).is_some() {
                continue;
            }
            if let Ok(piperine_ir::Constant::Int(at)) = self.consts.value(index, &bindings)
                && let Ok(at) = usize::try_from(at)
            {
                folded.insert(index, at);
            }
        }
        folded
    }

    /// The bundle port, signal or group the name `root` stands for in `scope`: itself, or what the handle `root` is bound to.
    fn handled_bundle(&mut self, scope: &Scope, root: DeclId) -> Option<DeclId> {
        match scope.handles.get(&root) {
            Some(bound) => self.named_bundle(scope, *bound),
            None => Some(root),
        }
    }

    /// The bundle port, signal or group the expression `expr` names in `scope` (`s`, `o.inner`, `rx[k]`, a handle bound to either).
    pub(super) fn named_bundle(&mut self, scope: &Scope, expr: ExprId) -> Option<DeclId> {
        let expr = self.peel_parens(expr);
        match &self.design.expr_node(expr).kind {
            ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|segment| segment.res) {
                Some(Resolution::Decl(decl)) => self.handled_bundle(scope, decl).filter(|named| self.design.is_bundle_owner(*named) || self.design.is_bundle_group(*named)),
                _ => None,
            },
            ExprKind::Field { .. } | ExprKind::Index { .. } => self.bundle_leaf(scope, expr).filter(|leaf| self.design.is_bundle_group(*leaf)),
            _ => None,
        }
    }

    /// The fields of the bundle port `port` given by `arg`, joined to what `arg` gives them. `false` after a diagnostic.
    pub(super) fn bind_bundle_port(&mut self, scope: &Scope, instance: &str, port: DeclId, arg: ExprId, bound: &mut PortBindings) -> bool {
        let mut sources = Vec::new();
        self.leaf_sources(scope, arg, port, "", &mut sources);
        let mut ok = true;
        for leaf in self.design.bundle_leaves(port).to_vec() {
            if leaf.nested {
                continue;
            }
            // a field the argument leaves out was refused (`E0263`) before: nothing is bound for it
            let Some(source) = sources.iter().find(|(path, _)| *path == leaf.path).map(|(_, source)| *source) else { continue };
            ok &= self.bind_leaf(scope, instance, leaf.decl, source, arg, bound);
        }
        ok
    }

    /// The nodes of the array-of-nodes field `from` of a bundle of the frame, one terminal each, in order.
    fn leaf_node_run(&mut self, scope: &Scope, from: DeclId) -> Option<Vec<Terminal>> {
        let length: u64 = self.frames.get(scope.frame).and_then(|frame| frame.node_shapes.get(&from)).map(|shape| shape.dims.iter().product())?;
        let Some(Terminal::Node(first)) = self.frame_node(scope.frame, from) else { return None };
        Some((0..length as u32).map(|k| Terminal::Node(first + k)).collect())
    }

    /// The source of every field under `prefix` of the bundle port `port` that `arg` gives, by path.
    fn leaf_sources(&mut self, scope: &Scope, arg: ExprId, port: DeclId, prefix: &str, out: &mut Vec<(String, LeafSource)>) {
        if let Some(source) = self.named_bundle(scope, arg) {
            for leaf in self.design.bundle_leaves(port).iter().filter(|leaf| !leaf.nested) {
                let Some(rest) = leaf.path.strip_prefix(prefix) else { continue };
                if let Some(from) = self.design.leaf_under(source, rest) {
                    out.push((leaf.path.clone(), LeafSource::Leaf(from)));
                }
            }
            return;
        }
        let ExprKind::Struct { fields, class: StructClass::BundleLiteral, .. } = &self.design.expr_node(arg).kind else { return };
        for field in fields {
            let path = if prefix.is_empty() { field.name.text.clone() } else { format!("{prefix}{}", field.name.text) };
            let nested = self.design.bundle_leaves(port).iter().any(|leaf| leaf.nested && leaf.path == path);
            if nested {
                self.leaf_sources(scope, field.value, port, &format!("{path}."), out);
            } else {
                out.push((path, LeafSource::Expression(field.value)));
            }
        }
    }

    /// One field of a bundle port: the carrier or terminal `source` gives the leaf `port`.
    fn bind_leaf(&mut self, scope: &Scope, instance: &str, port: DeclId, source: LeafSource, arg: ExprId, bound: &mut PortBindings) -> bool {
        let span = self.design.expr_node(arg).span;
        // an array of nodes as a field: its nodes in order, from the carrier of the parent or the nodes a literal names
        if self.types.node_array(port).is_some() {
            let nodes = match source {
                LeafSource::Expression(expr) => self.node_array_terminals(scope, expr),
                LeafSource::Leaf(from) => self.leaf_node_run(scope, from),
            };
            let Some(nodes) = nodes else { return false };
            bound.terminals.extend(nodes);
            return true;
        }
        // a field that is an array of length 0 has no carrier in the child, so it binds nothing (§1.2.3)
        if self.types.carrier_array(port).is_some_and(crate::types::Ty::has_fixed_zero_length) {
            return true;
        }
        if self.types.carrier(port).is_none() {
            let terminal = match source {
                LeafSource::Expression(expr) => self.lower_terminal(scope, expr),
                LeafSource::Leaf(from) => self.frame_node(scope.frame, from),
            };
            let Some(terminal) = terminal else {
                self.unsupported(scope.file, span, "a field of a bundle that is no node of this module");
                return false;
            };
            bound.terminals.push(terminal);
            return true;
        }
        let found = match source {
            LeafSource::Expression(expr) => self.lower_carrier_argument(scope, instance, port, expr),
            LeafSource::Leaf(from) => self.frames.get(scope.frame).and_then(|frame| frame.carriers.get(&from)).map(|index| (*index, None)),
        };
        let Some((carrier, window)) = found else {
            self.unsupported(scope.file, span, "a field of a bundle that is no carrier of this module");
            return false;
        };
        if matches!(&self.design.decl_data(port).kind, DeclKind::Port(data) if data.dir == Some(Dir::Out)) {
            self.structure.handed.push((carrier, window));
            let reach = ElementReach::of_window(window);
            self.structure.drive_sites.push(DriveSite { carrier, reach, fields: Vec::new(), way: DriveWay::Handed, file: scope.file, span });
        }
        bound.carriers.push(Some(carrier));
        bound.windows.push(window);
        true
    }
}
