//! Bundle connections and the places a bundle is no value (feature `resolved-nets-bundles`, BN-03; spec part II
//! §7 rules 8, 9 and 14). A bundle port of a placement takes a bundle carrier of the same `struct` (each field joins the
//! field of the same name, §7 rule 14) or a literal `Cell { wl: w, bl: b }` that names every field once. A field of a
//! literal is a carrier or a node of the field's nature (`E0263`), a bundle of another `struct` does not connect
//! (`E0251`), and a bundle read as a value, or a literal written anywhere but a connection, is `E0264`.

use super::check::Checker;
use super::nature_rules::NatureRules;
use super::Ty;
use crate::msl::ast::Dir;
use crate::msl::Span;
use crate::pom::{BundleLeaf, DeclId, DeclKind, ExprId, ExprKind, FieldInit, FileId, Resolution, StructClass};

impl Checker<'_> {
    /// The argument `arg` of a placement at the bundle port `port`: a bundle carrier of the port's `struct`, or a literal.
    pub(super) fn connect_bundle(&mut self, arg: ExprId, port: DeclId) {
        let (file, span) = (self.file_of_expr(arg), self.design.expr_node(arg).span);
        if let ExprKind::Struct { fields, class: StructClass::BundleLiteral, path } = self.design.expr_node(self.unparenthesized(arg)).kind.clone() {
            if self.literal_names(path) != self.bundle_struct_of(port) {
                let message = format!("a literal of `{}` is connected to a port of `{}` (§7 rule 8)", self.struct_name(self.literal_names(path)), self.struct_name(self.bundle_struct_of(port)));
                self.coded(file, "E0251", span, message, "bundle of another type");
                return;
            }
            self.connect_literal(&fields, port, "", file);
            return;
        }
        let Some(source) = self.bundle_carrier(arg) else {
            let message = "a bundle port takes a bundle carrier or a literal of the bundle (§7 rule 8)".to_string();
            self.coded(file, "E0264", span, message, "not a bundle");
            self.infer(arg);
            return;
        };
        if self.bundle_struct_of(source) != self.bundle_struct_of(port) {
            let message = format!("a bundle `{}` is connected to a port of `{}` (§7 rule 14)", self.struct_name(self.bundle_struct_of(source)), self.struct_name(self.bundle_struct_of(port)));
            self.coded(file, "E0251", span, message, "bundle of another type");
            return;
        }
        // an array of bundles connects to an array of as many elements (§7 rule 11)
        if self.design.is_bundle_owner(source) && self.design.bundle_leaves(source).len() != self.design.bundle_leaves(port).len() {
            let message = "an array of bundles is connected to an array of another length, or to a single bundle (§7 rule 11)".to_string();
            self.coded(file, "E0251", span, message, "bundle of another shape");
            return;
        }
        self.connect_fields(source, port, "", arg, file, span);
    }

    /// Every field of the bundle `port` at `prefix` joined to the field of the bundle `source` of the same name.
    fn connect_fields(&mut self, source: DeclId, port: DeclId, prefix: &str, place: ExprId, file: FileId, span: Span) {
        for leaf in self.design.bundle_leaves(port).to_vec() {
            if leaf.nested {
                continue;
            }
            let Some(rest) = leaf.path.strip_prefix(prefix) else { continue };
            let Some(from) = self.design.leaf_under(source, rest) else { continue };
            self.connect_leaf(from, &leaf, place, file, span);
        }
    }

    /// One field: the leaf `target` of a port joined to the leaf `from` of a bundle carrier, `place` the expression that names the bundle.
    fn connect_leaf(&mut self, from: DeclId, target: &BundleLeaf, place: ExprId, file: FileId, span: Span) {
        let (Some(expected), Some(found)) = (self.placed_leaf_type(target.decl), self.leaf_type(from)) else { return };
        let same = match (&expected, &found) {
            (Ty::Node(wanted), Ty::Node(given)) => NatureRules::compatible(self.design, *wanted, *given),
            _ => expected == found,
        };
        if !same {
            let message = format!("the field `{}` of a bundle is `{}` and the port takes `{}` (§7 rule 14)", target.path, found.name(self.design), expected.name(self.design));
            self.coded(file, "E0251", span, message, "field of another type");
            return;
        }
        if matches!(&self.design.decl_data(target.decl).kind, DeclKind::Port(p) if p.dir == Some(Dir::Out)) && matches!(found.innermost(), Ty::Carrier { .. }) {
            self.hand_over_whole(from, place, span, file);
        }
    }

    /// The fields a literal names at `prefix`, each given to the leaf of the port of the same name: a carrier or a node of the
    /// field's nature, or a nested literal (§7 rule 8).
    fn connect_literal(&mut self, fields: &[FieldInit], port: DeclId, prefix: &str, file: FileId) {
        for field in fields {
            let path = if prefix.is_empty() { field.name.text.clone() } else { format!("{prefix}.{}", field.name.text) };
            let Some(leaf) = self.design.bundle_leaves(port).iter().find(|leaf| leaf.path == path).cloned() else { continue };
            let value = self.unparenthesized(field.value);
            match (self.design.expr_node(value).kind.clone(), leaf.nested) {
                (ExprKind::Struct { fields: inner, class: StructClass::BundleLiteral, .. }, true) => self.connect_literal(&inner, port, &path, file),
                (_, true) => {
                    let Some(source) = self.bundle_carrier(value) else {
                        self.literal_field_mismatch(file, field, &path, "a bundle of the field's type or a nested literal");
                        continue;
                    };
                    self.connect_fields(source, port, &format!("{path}."), value, file, field.span);
                }
                (_, false) => self.connect_literal_leaf(field, &leaf, file),
            }
        }
    }

    /// One field of a literal given to the leaf `leaf`: a carrier or a node of the leaf's nature, else `E0263`.
    fn connect_literal_leaf(&mut self, field: &FieldInit, leaf: &BundleLeaf, file: FileId) {
        let Some(expected) = self.placed_leaf_type(leaf.decl) else { return };
        let given = self.declared_port(field.value);
        let fits = match (&expected, given.as_ref().map(|(_, ty)| ty)) {
            (Ty::Node(wanted), Some(Ty::Node(found))) => NatureRules::compatible(self.design, *wanted, *found),
            (wanted, Some(found)) => wanted == found,
            (_, None) => false,
        };
        if !fits {
            self.literal_field_mismatch(file, field, &leaf.path, &format!("a carrier or node of `{}`", expected.name(self.design)));
            self.infer(field.value);
            return;
        }
        if let Some((decl, Ty::Carrier { .. })) = given
            && matches!(&self.design.decl_data(leaf.decl).kind, DeclKind::Port(p) if p.dir == Some(Dir::Out))
        {
            self.hand_over(decl, field.value, self.design.expr_node(field.value).span, file);
        }
    }

    /// `E0263`: the field `path` of a literal is not `what` (§7 rule 8).
    fn literal_field_mismatch(&mut self, file: FileId, field: &FieldInit, path: &str, what: &str) {
        let message = format!("the field `{path}` of the bundle literal is not {what} (§7 rule 8)");
        self.coded(file, "E0263", field.span, message, "not a carrier of the field's nature");
    }

    /// The type of the port leaf `leaf` of the module being placed, under the arguments the placement gives its generic parameters.
    fn placed_leaf_type(&mut self, leaf: DeclId) -> Option<Ty> {
        match &self.design.decl_data(leaf).kind {
            DeclKind::Port(p) => self.port_type_under_placement(p.ty),
            _ => None,
        }
    }

    /// The type of the port or signal of a leaf: a node, a carrier or an array of them.
    fn leaf_type(&mut self, leaf: DeclId) -> Option<Ty> {
        match &self.design.decl_data(leaf).kind {
            DeclKind::Port(p) => self.port_type(p.ty),
            DeclKind::Signal(s) => self.port_type(s.ty),
            _ => None,
        }
    }

    /// The bundle port, signal or nested group `expr` names (`s`, `o.inner`), through parentheses.
    fn bundle_carrier(&self, expr: ExprId) -> Option<DeclId> {
        self.design.bundle_named(expr)
    }

    /// The `struct` a bundle port, signal or group is of.
    fn bundle_struct_of(&self, decl: DeclId) -> Option<DeclId> {
        let ty = match &self.design.decl_data(decl).kind {
            DeclKind::Port(p) => p.ty,
            DeclKind::Signal(s) => s.ty,
            _ => return None,
        };
        match self.design.type_resolution(ty) {
            Resolution::Decl(decl) if matches!(self.design.decl_data(decl).kind, DeclKind::Struct(_)) => Some(decl),
            _ => None,
        }
    }

    /// `expr` with its parentheses peeled.
    fn unparenthesized(&self, mut expr: ExprId) -> ExprId {
        while let ExprKind::Paren(inner) = &self.design.expr_node(expr).kind {
            expr = *inner;
        }
        expr
    }

    /// The `struct` a literal's path names.
    fn literal_names(&self, path: crate::pom::PathId) -> Option<DeclId> {
        match self.design.path_node(path).segments.last().map(|s| s.res) {
            Some(Resolution::Decl(decl)) if matches!(self.design.decl_data(decl).kind, DeclKind::Struct(_)) => Some(decl),
            _ => None,
        }
    }

    /// The name of the `struct` `decl`.
    fn struct_name(&self, decl: Option<DeclId>) -> String {
        decl.and_then(|d| self.design.decl_data(d).name.as_ref().map(|n| n.text.clone())).unwrap_or_default()
    }

    /// `E0264`: the bundle `what` is read as a value (§7 rule 9).
    pub(super) fn bundle_as_value(&mut self, expr: ExprId) -> Option<Ty> {
        let (file, span) = (self.file_of_expr(expr), self.design.expr_node(expr).span);
        let message = "a bundle is not a value: it is no operand, argument or result, and has no `Repr` (§7 rule 9)".to_string();
        self.coded(file, "E0264", span, message, "a bundle is not a value");
        None
    }

    /// `E0264`: a bundle literal written anywhere but a connection (§7 rule 8).
    pub(super) fn literal_outside_connection(&mut self, expr: ExprId) -> Option<Ty> {
        let (file, span) = (self.file_of_expr(expr), self.design.expr_node(expr).span);
        let message = "a bundle literal is written only as a connection: it is no value (§7 rules 8 and 9)".to_string();
        self.coded(file, "E0264", span, message, "a literal outside a connection");
        None
    }
}
