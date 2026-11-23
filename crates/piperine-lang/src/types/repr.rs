//! A hand-written `impl Repr for T` (part I §2 rules 2 and 4, GN-05): the layout of a type whose
//! encoding is not the derived one, such as a one-hot or a Gray code. The `impl` gives a
//! `const WIDTH: u32`, an `encode` to `[bool; WIDTH]` and a `decode` back to `Option<Self>`; the
//! type then has `Repr` of `WIDTH` levels, so a carrier may hold it and `T::WIDTH` reads it.

use crate::pom::{DeclId, DeclKind, TypeId};

use super::check::Checker;
use super::Ty;

impl Checker<'_> {
    /// The number of levels of `ty` when its `Repr` is written by hand: the folded `WIDTH` of its
    /// `impl Repr`. `None` for a type with no such `impl`, or a `WIDTH` the folder cannot read.
    pub(super) fn hand_written_levels(&self, ty: &Ty) -> Option<u32> {
        let entry = self.impls.hand_written_repr(ty)?;
        let width = self.impls.item(entry.decl, "WIDTH")?;
        let DeclKind::Const(data) = &self.design.decl_data(width).kind else { return None };
        u32::try_from(self.fold_integer(data.value?, u8::MAX)?).ok()
    }

    /// Holds the `impl Repr for T` `decl` to §2: it defines `WIDTH`, `encode` and `decode` and no other
    /// item, `WIDTH` is a `u32`, `encode` takes `Self` and returns `[bool; WIDTH]`, `decode` takes
    /// `[bool; WIDTH]` and returns `Option<Self>`. Each is `E0187` naming `Repr` and the type.
    pub(super) fn repr_impl_rules(&mut self, decl: DeclId, self_ty: TypeId) {
        let (file, span) = (self.design.decl_data(decl).file, self.design.decl_data(decl).span);
        let Some(own) = self.lower_type(self_ty) else { return };
        let head = format!("`impl Repr for {}`", own.source_name(self.design));
        for name in ["WIDTH", "encode", "decode"] {
            if self.impls.item(decl, name).is_none() {
                self.coded(file, "E0187", span, format!("{head} leaves out `{name}`, which §2 requires"), "an item of `Repr` left out");
            }
        }
        let children = self.design.decl_data(decl).children.clone();
        for child in children {
            let item = self.design.decl_data(child);
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
            let Some(item) = self.impls.item(decl, name) else { continue };
            let Some((params, ret)) = self.declared_signature(item) else { continue };
            let found = Ty::Fn(params, Box::new(ret));
            if !found.fits(&wanted) {
                let (item_file, item_span) = (self.design.decl_data(item).file, self.design.decl_data(item).span);
                self.coded(item_file, "E0187", item_span, format!("{head}: `{name}` has the type `{}`, §2 gives it `{}`", found.name(self.design), wanted.name(self.design)), "a signature §2 does not give");
            }
        }
    }
}
