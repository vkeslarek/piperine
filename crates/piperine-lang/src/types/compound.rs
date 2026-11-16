//! Compound value types: unit enums (part I §2 rule 5, DIG-03).
//!
//! An enum of the subset has unit variants only and derives `Repr`: its tag is its value. A
//! variant with a payload, an explicit discriminant or a `#[repr]` attribute is outside the
//! subset and refused by name.

use crate::msl::Span;
use crate::pom::{AttrArgNode, AttrInputNode, DeclId, DeclKind, FieldsForm, FileId};

use super::check::Checker;
use super::Ty;

impl Checker<'_> {
    /// The type of the enum `decl` named at `span` of `file`: `Ty::Enum` when it is in the
    /// subset, else one `P0001` naming what is not, and `None`.
    pub(super) fn enum_type(&mut self, decl: DeclId, file: FileId, span: Span) -> Option<Ty> {
        let data = self.design.decl_data(decl).clone();
        let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        if data.generics_span.is_some() {
            self.unsupported(file, span, &format!("generic enum `{name}`"));
            return None;
        }
        if self.has_attr(decl, "repr") {
            self.unsupported(file, span, &format!("enum `{name}` with a `#[repr]` attribute"));
            return None;
        }
        for variant in data.children.iter().copied() {
            let v = self.design.decl_data(variant);
            let DeclKind::Variant(vd) = &v.kind else { continue };
            let vname = v.name.as_ref().map_or("", |n| n.text.as_str());
            if vd.form != FieldsForm::Unit {
                self.unsupported(file, span, &format!("enum `{name}` with a variant with a payload (`{vname}`)"));
                return None;
            }
            if vd.discriminant.is_some() {
                self.unsupported(file, span, &format!("enum `{name}` with an explicit discriminant (`{vname}`)"));
                return None;
            }
        }
        if !self.derives(decl, "Repr") {
            self.unsupported(file, span, &format!("enum `{name}` without `#[derive(Repr)]`"));
            return None;
        }
        Some(Ty::Enum(decl))
    }

    /// Whether `decl` carries `#[derive(.., trait_name, ..)]`.
    pub(super) fn derives(&self, decl: DeclId, trait_name: &str) -> bool {
        self.design.decl_data(decl).meta.attrs.iter().any(|attr| {
            let is_derive = matches!(self.design.path_node(attr.path).segments.as_slice(), [only] if only.name.text == "derive");
            is_derive
                && matches!(&attr.input, AttrInputNode::List(args) if args.iter().any(|arg| matches!(arg, AttrArgNode::Path(p) if self.design.path_node(*p).segments.last().is_some_and(|s| s.name.text == trait_name))))
        })
    }

    /// Whether `decl` carries an attribute whose path is the one word `name`.
    fn has_attr(&self, decl: DeclId, name: &str) -> bool {
        self.design.decl_data(decl).meta.attrs.iter().any(|attr| matches!(self.design.path_node(attr.path).segments.as_slice(), [only] if only.name.text == name))
    }
}
