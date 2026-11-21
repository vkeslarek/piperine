//! Stage 8: structural checks over declarations alone, with no types and no
//! const evaluation: the shape of an `extern` module (`E0370`), the body rule
//! of functions (`E0125`), the fields of a nature (`E0201`) and the declaration
//! rules of a bundle (`E0260`, `E0261`, §7). Runs after classification: a value input is
//! a port the classifier made one.

use super::super::body::TypeKind;
use super::super::decl::{DeclKind, FieldData};
use super::super::design::Design;
use super::super::ids::{DeclId, FileId, TypeId};
use super::super::resolution::{NatureKind, PortKind, Resolution};
use crate::msl::ast::{Lit, LitKind};
use crate::msl::{Diagnostic, Span};

/// Every structural error of the design, in declaration order.
pub(super) fn run(design: &Design) -> Vec<(FileId, Diagnostic)> {
    let mut out = Vec::new();
    let bundles = BundleShape::new(design);
    for i in 0..design.decl_count() {
        let id = DeclId(i as u32);
        let data = design.decl_data(id);
        bundles.check(id, &mut out);
        match &data.kind {
            DeclKind::Module(m) if m.abi.is_some() => extern_module(design, id, &mut out),
            DeclKind::Fn(f) => {
                let in_trait = data.parent.is_some_and(|p| matches!(design.decl_data(p).kind, DeclKind::Trait(_)));
                let intrinsic_outside_std = matches!(&f.abi, Some(Lit { kind: LitKind::Str { text, .. }, .. }) if text == "intrinsic") && design.file_data(data.file).krate != design.std_crate;
                let message = match (f.abi.is_some(), f.body.is_some()) {
                    (true, true) => Some("an `extern` function cannot have a body"),
                    (false, false) if !in_trait => Some("this function has no body; only an `extern` function or a trait item may omit it"),
                    (true, false) if intrinsic_outside_std => Some("`extern \"intrinsic\"` is accepted only in the standard library"),
                    _ => None,
                };
                if let (Some(message), Some(name)) = (message, &data.name) {
                    let mut diagnostic = Diagnostic::new("E0125", message.to_string(), name.span, "here");
                    if intrinsic_outside_std && f.body.is_none() {
                        diagnostic = diagnostic.with_help("only the standard library declares intrinsic functions; write a body or use `extern \"C\"`");
                    }
                    out.push((data.file, diagnostic));
                }
            }
            DeclKind::Nature(n) if n.kind == NatureKind::Invalid => {
                if let Some(name) = &data.name {
                    let message = format!("nature `{}` repeats a field, mixes discrete and continuous fields, or has neither `value` nor `potential`", name.text);
                    out.push((data.file, Diagnostic::new("E0201", message, name.span, "invalid nature")));
                }
            }
            _ => {}
        }
    }
    out
}

/// §8.5.7: an extern module lists params only, has no result and no value input.
fn extern_module(design: &Design, module: DeclId, out: &mut Vec<(FileId, Diagnostic)>) {
    let data = design.decl_data(module);
    let DeclKind::Module(m) = &data.kind else { return };
    let mut report = |span, message: String, label: &str| out.push((data.file, Diagnostic::new("E0370", message, span, label.to_string())));
    if let Some(ret) = m.ret {
        report(design.type_node(ret).span, "an `extern` module has no result".to_string(), "result type");
    }
    for child in &data.children {
        let item = design.decl_data(*child);
        match &item.kind {
            DeclKind::GenericParam(_) | DeclKind::Param(_) | DeclKind::Error => {}
            DeclKind::Port(p) if p.kind == PortKind::ValueInput => report(item.span, "an `extern` module has no value input".to_string(), "value input"),
            DeclKind::Port(_) => {}
            other => report(item.span, format!("an `extern` module lists params only, not a `{}`", other.label()), "not a param"),
        }
    }
}

/// What a declared type is made of, for the bundle rules of §7.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TypeShape {
    /// A carrier: a nature, or a `Nature`-bounded generic, under arrays and `flip`.
    Carrier,
    /// A bundle: a `struct` the classification flagged.
    Bundle,
    /// A value: a builtin, a struct or enum that is no bundle, a tuple, a slice or a reference.
    Value,
    /// Unresolved or not yet known: no rule of §7 applies, an earlier error already speaks.
    Unknown,
}

/// §7 rule 1: the declaration rules of a bundle that need no types and no const evaluation. A
/// `struct` with a carrier field (directly or through a bundle field) is a bundle, and every one
/// of its fields is a carrier, an array of carriers or a bundle.
struct BundleShape<'design> {
    /// The classified design the rules read.
    design: &'design Design,
}

impl<'design> BundleShape<'design> {
    /// The rules over `design`.
    fn new(design: &'design Design) -> Self {
        BundleShape { design }
    }

    /// Every bundle rule that applies to the declaration `decl`.
    fn check(&self, decl: DeclId, out: &mut Vec<(FileId, Diagnostic)>) {
        let data = self.design.decl_data(decl);
        if let DeclKind::Struct(s) = &data.kind
            && s.bundle == Some(true)
        {
            self.check_fields_are_carriers(decl, out);
            self.check_field_directions(decl, out);
        }
    }

    /// What `ty` is made of.
    fn shape_of(&self, ty: TypeId) -> TypeShape {
        if self.design.is_carrier_type(ty) {
            return TypeShape::Carrier;
        }
        match self.design.type_resolution(ty) {
            Resolution::Decl(d) if matches!(&self.design.decl_data(d).kind, DeclKind::Struct(s) if s.bundle == Some(true)) => TypeShape::Bundle,
            Resolution::Decl(d) if self.design.has_unresolved_bound(d) => TypeShape::Unknown,
            Resolution::Decl(_) | Resolution::Builtin(_) => TypeShape::Value,
            Resolution::Unresolved | Resolution::Deferred(_) => match &self.design.type_node(self.design.peel_type(ty)).kind {
                TypeKind::Tuple(_) | TypeKind::Slice(_) | TypeKind::Ref { .. } => TypeShape::Value,
                _ => TypeShape::Unknown,
            },
        }
    }

    /// The name and the name's span of a field (a tuple field has none: its whole span).
    fn field_name(&self, field: DeclId) -> (String, Span) {
        let data = self.design.decl_data(field);
        match &data.name {
            Some(name) => (name.text.clone(), name.span),
            None => ("(unnamed)".to_string(), data.span),
        }
    }

    /// §7 rule 1: a bundle holds carriers, arrays of carriers and bundles only. A field of a value
    /// type is `E0260`, naming it and a field that is a carrier.
    fn check_fields_are_carriers(&self, bundle: DeclId, out: &mut Vec<(FileId, Diagnostic)>) {
        let data = self.design.decl_data(bundle);
        let typed: Vec<(DeclId, TypeShape)> = data
            .children
            .iter()
            .filter(|c| matches!(self.design.decl_data(**c).kind, DeclKind::Field(_)))
            .filter_map(|c| self.design.declared_type(*c).map(|ty| (*c, self.shape_of(ty))))
            .collect();
        let Some(carrier) = typed.iter().find(|(_, shape)| matches!(shape, TypeShape::Carrier | TypeShape::Bundle)) else { return };
        let (carrier_name, _) = self.field_name(carrier.0);
        let bundle_name = data.name.as_ref().map_or("", |n| n.text.as_str());
        for (field, _) in typed.iter().filter(|(_, shape)| *shape == TypeShape::Value) {
            let (name, span) = self.field_name(*field);
            let message = format!("the bundle `{bundle_name}` mixes the carrier field `{carrier_name}` with the value field `{name}` (§7 rule 1)");
            out.push((data.file, Diagnostic::new("E0260", message, span, "a value field in a bundle")));
        }
    }

    /// The fields of a bundle struct: the field declaration and its data.
    fn fields_of(&self, bundle: DeclId) -> impl Iterator<Item = (DeclId, &'design FieldData)> + use<'design, '_> {
        self.design.decl_data(bundle).children.iter().filter_map(|c| match &self.design.decl_data(*c).kind {
            DeclKind::Field(f) => Some((*c, f)),
            _ => None,
        })
    }

    /// §7 rule 2: a field of a one-driver nature (a discrete nature without `resolve`) has `in` or
    /// `out`; a field of any other nature has none. A field of a generic or an invalid nature is
    /// not judged, and the directions of a nested bundle are `E0262`'s concern.
    fn check_field_directions(&self, bundle: DeclId, out: &mut Vec<(FileId, Diagnostic)>) {
        let file = self.design.decl_data(bundle).file;
        for (field, data) in self.fields_of(bundle) {
            if self.shape_of(data.ty) != TypeShape::Carrier {
                continue;
            }
            let Some(nature) = self.design.nature_of(data.ty) else { continue };
            let DeclKind::Nature(nature_data) = &self.design.decl_data(nature).kind else { continue };
            let (name, span) = self.field_name(field);
            let message = match (nature_data.kind, data.dir) {
                (NatureKind::Discrete, None) => format!("the field `{name}` is of a one-driver nature and needs `in` or `out` (§7 rule 2)"),
                (NatureKind::Resolved | NatureKind::Conservative | NatureKind::SignalFlow, Some(_)) => {
                    format!("the field `{name}` is not of a one-driver nature and takes no direction (§7 rule 2)")
                }
                _ => continue,
            };
            out.push((file, Diagnostic::new("E0261", message, span, "a misplaced or missing direction")));
        }
    }
}
