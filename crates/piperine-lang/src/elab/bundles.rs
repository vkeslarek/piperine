//! A bundle at a placement (feature `resolved-nets-bundles`, BN-03; spec part II §7 rules 8 and 14): a bundle port of the
//! placed module is given a bundle carrier of the parent (each field joins the field of the same name) or a literal
//! `Cell { wl: w, bl: b }` (each named field joins the carrier or node it is given). Each field of the port is one
//! carrier or terminal of the child, in the order of the port's leaves, the order `Design::module_items` gives the
//! child's own body.

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
    /// a handle an inlined function was given (`handles.rs`), which is the leaf of the bundle the call named (§7 rule 13).
    pub(super) fn bundle_leaf(&self, scope: &Scope, expr: ExprId) -> Option<DeclId> {
        self.design.bundle_field_through(expr, &|root| self.handled_bundle(scope, root))
    }

    /// The bundle port, signal or group the name `root` stands for in `scope`: itself, or what the handle `root` is bound to.
    fn handled_bundle(&self, scope: &Scope, root: DeclId) -> Option<DeclId> {
        match scope.handles.get(&root) {
            Some(bound) => self.named_bundle(scope, *bound),
            None => Some(root),
        }
    }

    /// The bundle port, signal or group the expression `expr` names in `scope` (`s`, `o.inner`, a handle bound to either).
    pub(super) fn named_bundle(&self, scope: &Scope, expr: ExprId) -> Option<DeclId> {
        let expr = self.peel_parens(expr);
        match &self.design.expr_node(expr).kind {
            ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|segment| segment.res) {
                Some(Resolution::Decl(decl)) => self.handled_bundle(scope, decl).filter(|named| self.design.is_bundle_owner(*named) || self.design.is_bundle_group(*named)),
                _ => None,
            },
            ExprKind::Field { .. } => self.bundle_leaf(scope, expr).filter(|leaf| self.design.is_bundle_group(*leaf)),
            _ => None,
        }
    }

    /// The fields of the bundle port `port` given by `arg`, joined to what `arg` gives them. `false` after a diagnostic.
    pub(super) fn bind_bundle_port(&mut self, scope: &Scope, instance: &str, port: DeclId, arg: ExprId, bound: &mut PortBindings) -> bool {
        let mut sources = Vec::new();
        self.leaf_sources(arg, port, "", &mut sources);
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

    /// The source of every field under `prefix` of the bundle port `port` that `arg` gives, by path.
    fn leaf_sources(&self, arg: ExprId, port: DeclId, prefix: &str, out: &mut Vec<(String, LeafSource)>) {
        if let Some(source) = self.design.bundle_named(arg) {
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
                self.leaf_sources(field.value, port, &format!("{path}."), out);
            } else {
                out.push((path, LeafSource::Expression(field.value)));
            }
        }
    }

    /// One field of a bundle port: the carrier or terminal `source` gives the leaf `port`.
    fn bind_leaf(&mut self, scope: &Scope, instance: &str, port: DeclId, source: LeafSource, arg: ExprId, bound: &mut PortBindings) -> bool {
        let span = self.design.expr_node(arg).span;
        if self.types.node_array(port).is_some() || self.types.carrier_array(port).is_some() {
            self.unsupported(scope.file, span, "an array field of a bundle");
            return false;
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
