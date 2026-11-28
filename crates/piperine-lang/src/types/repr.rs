//! A hand-written `impl Repr for T` (part I §2 rules 2 and 4, GN-05): the layout of a type whose
//! encoding is not the derived one, such as a one-hot or a Gray code. The `impl` gives a
//! `const WIDTH: u32`, an `encode` to `[bool; WIDTH]` and a `decode` back to `Option<Self>`; the
//! type then has `Repr` of `WIDTH` levels, so a carrier may hold it and `T::WIDTH` reads it.

use crate::pom::{DeclId, DeclKind, TypeId};

use super::check::Checker;
use super::{Length, Ty, Types};

impl Checker<'_> {
    /// The number of levels of `ty` when its `Repr` is written by hand: the folded `WIDTH` of its
    /// `impl Repr`. `None` for a type with no such `impl`, or a `WIDTH` the folder cannot read.
    pub(super) fn hand_written_levels(&mut self, ty: &Ty) -> Option<u32> {
        let entry = self.state.traits.hand_written_repr(ty)?.clone();
        let width = self.state.traits.item(entry.decl, "WIDTH")?;
        let DeclKind::Const(data) = &self.state.design.decl_data(width).kind else { return None };
        let value = data.value?;
        let values = self.impl_widths(entry.decl, ty)?;
        u32::try_from(super::value::lengths::IntegerFolder::with_values(self.state.design, values).fold(value, u8::MAX)?).ok()
    }

    /// What the parameters of the `impl` `owner` take for the type `ty` it is written for: the level count of each
    /// type argument (`impl<T: Repr> Repr for Pair<T>` meeting `Pair<u8>` gives `T` 8, what `T::WIDTH` reads) and the
    /// value of each const argument. Empty for an `impl` with no parameters; `None` when a parameter stays open or
    /// its argument has no `Repr`.
    fn impl_widths(&mut self, owner: DeclId, ty: &Ty) -> Option<std::collections::BTreeMap<DeclId, u128>> {
        let mut values = std::collections::BTreeMap::new();
        if self.state.design.decl_data(owner).generics_span.is_none() {
            return Some(values);
        }
        let DeclKind::Impl(data) = &self.state.design.decl_data(owner).kind else { return None };
        let self_ty = data.self_ty;
        let declared = self.expressions().lower_type_quietly(self_ty)?;
        let signature = super::GenericSignature::of(self.state.design, owner);
        let mut inference = super::inference::Inference::new(&signature);
        inference.unify(&declared, ty);
        for parameter in signature.parameters() {
            match inference.found().get(parameter.decl)? {
                super::GenericValue::Type(argument) => {
                    let levels = self.repr_levels(&argument.clone())?;
                    values.insert(parameter.decl, u128::from(levels));
                }
                super::GenericValue::Const(super::ConstTerm::Value(count)) => {
                    values.insert(parameter.decl, u128::try_from(*count).ok()?);
                }
                _ => return None,
            }
        }
        Some(values)
    }

    /// Puts the `impl Repr` of `ty`, with its `WIDTH`, `encode` and `decode`, on the worklist: a carrier of the type
    /// commits by `encode` and reads by `decode` (§2 rule 4), so the elaborator lowers both, and a type the run
    /// never carries leaves them unreached. A type with a derived layout has none.
    pub(super) fn reach_hand_written_repr(&mut self, ty: &Ty) {
        let Some(owner) = self.state.traits.hand_written_repr(ty).map(|entry| entry.decl) else { return };
        self.reach(owner);
        let items: Vec<DeclId> = ["WIDTH", "encode", "decode"].iter().filter_map(|name| self.state.traits.item(owner, name)).collect();
        for item in items {
            self.reach(item);
        }
    }

    /// Holds the `impl Repr for T` `decl` to §2: it defines `WIDTH`, `encode` and `decode` and no other
    /// item, `WIDTH` is a `u32`, `encode` takes `Self` and returns `[bool; WIDTH]`, `decode` takes
    /// `[bool; WIDTH]` and returns `Option<Self>`. Each is `E0187` naming `Repr` and the type.
    pub(super) fn repr_impl_rules(&mut self, decl: DeclId, self_ty: TypeId) {
        let (file, span) = (self.state.design.decl_data(decl).file, self.state.design.decl_data(decl).span);
        let Some(own) = self.lower_type(self_ty) else { return };
        let head = format!("`impl Repr for {}`", own.source_name(self.state.design));
        for name in ["WIDTH", "encode", "decode"] {
            if self.state.traits.item(decl, name).is_none() {
                self.coded(file, "E0187", span, format!("{head} leaves out `{name}`, which §2 requires"), "an item of `Repr` left out");
            }
        }
        let children = self.state.design.decl_data(decl).children.clone();
        for child in children {
            let item = self.state.design.decl_data(child);
            let (item_file, item_span, name) = (item.file, item.span, item.name.as_ref().map_or_else(String::new, |n| n.text.clone()));
            if !matches!(item.kind, DeclKind::Fn(_) | DeclKind::Const(_)) {
                continue;
            }
            if !["WIDTH", "encode", "decode"].contains(&name.as_str()) {
                self.coded(item_file, "E0187", item_span, format!("{head} defines `{name}`, which `Repr` does not have (§2)"), "an item `Repr` lacks");
            }
        }
        let Some(width) = self.hand_written_levels(&own) else { return };
        let levels = Ty::array(Ty::Bool, u64::from(width));
        let expected = [("encode", Ty::Fn(vec![own.clone()], Box::new(levels.clone()))), ("decode", Ty::Fn(vec![levels], Box::new(Ty::Option(Box::new(own.clone())))))];
        for (name, wanted) in expected {
            let Some(item) = self.state.traits.item(decl, name) else { continue };
            let Some((params, ret)) = self.declared_signature(item) else { continue };
            let found = Ty::Fn(params, Box::new(ret));
            if !found.fits(&wanted) {
                let (item_file, item_span) = (self.state.design.decl_data(item).file, self.state.design.decl_data(item).span);
                self.coded(item_file, "E0187", item_span, format!("{head}: `{name}` has the type `{}`, §2 gives it `{}`", found.name(self.state.design), wanted.name(self.state.design)), "a signature §2 does not give");
            }
        }
    }
}

impl Types<'_> {
    /// The number of levels of `ty`'s derived `Repr` (§2 rules 2 to 6): the scalars, a tuple, an array and an `Option` from
    /// their parts, a struct its fields end to end, an enum its tag then its widest payload. `None` for a type with no
    /// `Repr` among the types the check reached, and for one whose layout is written by hand (the elaborator reads that
    /// width from the `impl`).
    pub fn repr_levels(&self, ty: &Ty) -> Option<u32> {
        let sum = |parts: &[Ty]| parts.iter().try_fold(0u32, |total, part| total.checked_add(self.repr_levels(part)?));
        match ty {
            Ty::Tuple(items) => sum(items),
            Ty::Array(element, Length::Fixed(length)) => u32::try_from(*length).ok()?.checked_mul(self.repr_levels(element)?),
            Ty::Option(inner) => self.repr_levels(inner)?.checked_add(1),
            Ty::Struct(nominal) => sum(&self.instance_fields(nominal.decl, nominal).into_iter().map(|field| field.ty).collect::<Vec<_>>()),
            Ty::Enum(nominal) => {
                let tag = self.enum_tag(nominal.decl)?.tag_levels;
                let variants = self.design.decl_data(nominal.decl).children.iter().copied().filter(|child| matches!(self.design.decl_data(*child).kind, DeclKind::Variant(_)));
                let widest = variants.map(|variant| sum(&self.instance_fields(variant, nominal).into_iter().map(|field| field.ty).collect::<Vec<_>>())).try_fold(0u32, |widest, payload| Some(widest.max(payload?)))?;
                tag.checked_add(widest)
            }
            other => other.levels(self.design),
        }
    }
}
