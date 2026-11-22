//! Enums (§1.3.5 to §1.3.7, part I §2 rule 5): unit variants and variants with positional or
//! named fields, `#[repr(uN)]` and explicit discriminants.
//!
//! A variant's fields are typed like a struct's (`Checker::fields_of`), and a type that contains
//! itself through them is `E0186` (§1.3.8). A discriminant is allowed only where no variant has a
//! field, must fit the tag's levels, and is never repeated (`E0111`).

use crate::msl::Span;
use crate::pom::{AttrArgNode, AttrInputNode, DeclId, DeclKind, FieldsForm, FileId};

use super::aggregate::FieldType;
use super::check::Checker;
use super::Ty;

/// The tag of a declared enum (§2 rule 5, §1.3.7): its width in levels and the discriminant of each
/// variant, in declaration order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumTag {
    /// The number of levels of the tag: `N` of `#[repr(uN)]`, else `ceil(log2 V)`, zero for one variant.
    pub tag_levels: u32,
    /// The discriminant of every variant, in declaration order.
    pub discriminants: Vec<i128>,
}

impl Checker<'_> {
    /// The type of the enum `decl` named at `span` of `file`: [`Ty::Enum`] when its variants and
    /// their fields are typed, else a diagnostic and `None` (a generic enum or one without
    /// `#[derive(Repr)]` is outside the subset; a duplicate variant or field and a self-containing
    /// type are `E0186`; a discriminant that does not fit or repeats is `E0111`).
    pub(super) fn enum_type(&mut self, decl: DeclId, file: FileId, span: Span) -> Option<Ty> {
        let data = self.design.decl_data(decl).clone();
        let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        if data.generics_span.is_some() {
            self.unsupported(file, span, &format!("generic enum `{name}`"));
            return None;
        }
        if self.broken.contains(&decl) {
            return None;
        }
        if self.declared.contains(&decl) {
            return Some(Ty::Enum(decl));
        }
        if self.declaring.contains(&decl) {
            return self.cyclic_type(decl, file, span);
        }
        if !self.derives(decl, "Repr") {
            self.unsupported(file, span, &format!("enum `{name}` without `#[derive(Repr)]`"));
            return None;
        }
        self.declaring.insert(decl);
        let declared = self.declare_enum(decl, file, span);
        self.declaring.remove(&decl);
        if declared {
            self.declared.insert(decl);
            Some(Ty::Enum(decl))
        } else {
            self.broken.insert(decl);
            None
        }
    }

    /// The variants of the enum `decl`, in declaration order.
    pub(super) fn variants_of(&self, decl: DeclId) -> Vec<DeclId> {
        self.design.decl_data(decl).children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::Variant(_))).collect()
    }

    /// Whether some variant of the enum `decl` has fields: a payload enum.
    pub(super) fn has_payload(&self, decl: DeclId) -> bool {
        self.variants_of(decl).into_iter().any(|v| matches!(&self.design.decl_data(v).kind, DeclKind::Variant(vd) if vd.form != FieldsForm::Unit))
    }

    /// Types every variant, then judges the tag: duplicate names (§1.3.2), the discriminants and
    /// `#[repr(uN)]` (§1.3.7, §2 rule 5).
    fn declare_enum(&mut self, decl: DeclId, file: FileId, span: Span) -> bool {
        let variants = self.variants_of(decl);
        let mut ok = true;
        let mut names: Vec<String> = Vec::new();
        let mut typed_fields: Vec<FieldType> = Vec::new();
        for variant in &variants {
            let data = self.design.decl_data(*variant).clone();
            let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
            if names.contains(&name) {
                self.coded(data.file, "E0186", data.span, format!("the variant `{name}` is declared twice (§1.3.2)"), "declared twice");
                ok = false;
            }
            names.push(name);
            match self.fields_of(*variant) {
                Some(fields) => typed_fields.extend(fields),
                None => ok = false,
            }
        }
        let field_types: Vec<Ty> = typed_fields.iter().map(|f| f.ty.clone()).collect();
        ok && self.judge_tag(decl, &variants, file, span) & self.judge_repr(decl, file, &typed_fields) & self.judge_derives(decl, file, span, &field_types)
    }

    /// The tag of the enum `decl` (§2 rule 5): `#[repr(uN)]` gives `N` levels, else
    /// `ceil(log2(V))`. A discriminant on an enum with a field is `E0186`, one that does not fit
    /// the levels or repeats is `E0111`.
    fn judge_tag(&mut self, decl: DeclId, variants: &[DeclId], file: FileId, span: Span) -> bool {
        let payload = self.has_payload(decl);
        let Some(levels) = self.tag_levels(decl, variants.len(), file, span) else { return false };
        let mut ok = true;
        let mut taken: Vec<(u128, String)> = Vec::new();
        let mut next: u128 = 0;
        for variant in variants {
            let data = self.design.decl_data(*variant).clone();
            let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
            let DeclKind::Variant(vd) = &data.kind else { continue };
            let value = match vd.discriminant {
                Some(_) if payload => {
                    self.coded(data.file, "E0186", data.span, format!("the variant `{name}` has a discriminant, and an enum with a variant that has fields has none (§1.3.7)"), "discriminant on an enum with fields");
                    ok = false;
                    continue;
                }
                Some(expr) => match self.fold_integer(expr, 16) {
                    Some(value) => value,
                    None => {
                        self.unsupported(data.file, data.span, &format!("a discriminant of `{name}` that is not an integer literal, a `const` or integer arithmetic of them"));
                        ok = false;
                        continue;
                    }
                },
                None => next,
            };
            next = value + 1;
            if value >> levels != 0 {
                self.coded(data.file, "E0111", data.span, format!("the discriminant {value} of `{name}` does not fit the {levels} levels of the tag (§2 rule 5)"), "does not fit the tag");
                ok = false;
            } else if let Some((_, first)) = taken.iter().find(|(v, _)| *v == value) {
                self.coded(data.file, "E0111", data.span, format!("`{name}` has the discriminant {value}, which `{first}` has too (§1.3.7)"), "repeated discriminant");
                ok = false;
            }
            taken.push((value, name));
        }
        if ok {
            let discriminants = taken.iter().map(|(value, _)| *value as i128).collect();
            self.enum_tags.insert(decl, EnumTag { tag_levels: levels, discriminants });
        }
        ok
    }

    /// The levels of the tag of the enum `decl` with `count` variants.
    fn tag_levels(&mut self, decl: DeclId, count: usize, file: FileId, span: Span) -> Option<u32> {
        match self.repr_width(decl) {
            None => Some(usize::BITS - count.saturating_sub(1).leading_zeros()),
            Some(Some(width)) => Some(width),
            Some(None) => {
                self.unsupported(file, span, "`#[repr(..)]` other than `#[repr(uN)]` with `N` from 1 to 64");
                None
            }
        }
    }

    /// The `N` of `#[repr(uN)]` on `decl`: `None` without the attribute, `Some(None)` for another
    /// argument.
    pub(super) fn repr_width(&self, decl: DeclId) -> Option<Option<u32>> {
        let attr = self.design.decl_data(decl).meta.attrs.iter().find(|a| matches!(self.design.path_node(a.path).segments.as_slice(), [only] if only.name.text == "repr"))?;
        let AttrInputNode::List(args) = &attr.input else { return Some(None) };
        let [AttrArgNode::Path(path)] = args.as_slice() else { return Some(None) };
        let word = self.design.path_node(*path).segments.last().map(|s| s.name.text.clone()).unwrap_or_default();
        let width = word.strip_prefix('u').and_then(|digits| digits.parse::<u32>().ok()).filter(|w| (1..=64).contains(w));
        Some(width)
    }

    /// Whether `ty` has a `Repr` layout (§2): what the unknown literal and the flag reads
    /// (`known_or`, `is_known`, `level`, `cast`, ..) take: a scalar, an enum, or a tuple, an array,
    /// a struct whose parts have one. An `Option` is not among them: value code holds it as a pair, and its
    /// flag reads stay refused.
    pub(super) fn repr_ok(&mut self, ty: &Ty) -> bool {
        !matches!(ty, Ty::Option(_)) && self.repr_levels(ty).is_some()
    }
}
