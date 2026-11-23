//! The declaration rules of `impl` blocks (part I §3.1, §3.7, GN-03, GN-04, GN-10), checked on every
//! `impl` the check sees (the whole-model check sees them all): a method may not repeat the name of
//! a carrier method (`E0131`), an `impl` on a nature or a bundle holds functions only (`E0132`),
//! `impl Trait for` a bundle is `E0134` while a nature takes trait impls (traits are capabilities,
//! §3.1.9). An `impl` on a module is `E0130`, which the POM classifier raises (§3.1.4).

use crate::pom::{Decl, DeclId, DeclKind, Resolution};

use super::check::Checker;

/// The names of the methods of a carrier, which no `impl` may repeat (§3.1.3).
const CARRIER_METHOD_NAMES: [&str; 4] = ["changed", "prev", "others", "inflow"];

/// What an `impl` is written for, as far as its rules go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Subject {
    /// A nature.
    Nature,
    /// A struct with a carrier field.
    Bundle,
    /// Anything else: a value type, a module, a generic parameter.
    Other,
}

impl Checker<'_> {
    /// Holds the `impl` `decl` to the rules of §3.1: `E0131` for a method named like a carrier method,
    /// `E0132` for an item that is not a function in an inherent `impl` on a nature or a bundle,
    /// `E0134` for a trait `impl` on a bundle.
    pub(super) fn coherence(&mut self, decl: DeclId) {
        let DeclKind::Impl(data) = &self.design.decl_data(decl).kind else { return };
        let (self_ty, inherent) = (data.self_ty, data.trait_.is_none());
        let subject = self.subject(self_ty);
        let children = self.design.decl_data(decl).children.clone();
        for child in children {
            let item = self.design.decl_data(child);
            let (file, span) = (item.file, item.span);
            let name = item.name.as_ref().map_or_else(String::new, |n| n.text.clone());
            match &item.kind {
                DeclKind::Fn(_) if CARRIER_METHOD_NAMES.contains(&name.as_str()) => {
                    self.coded(file, "E0131", span, format!("a method named `{name}` repeats a method of every carrier (§3.1.3)"), "a carrier method's name");
                }
                DeclKind::Const(_) if inherent && matches!(subject, Subject::Nature | Subject::Bundle) => {
                    self.coded(file, "E0132", span, format!("`{name}` is a constant, and an `impl` on a nature or a bundle holds functions only (§3.1)"), "an item that is not a function");
                }
                _ => {}
            }
        }
        if !inherent && subject == Subject::Bundle {
            let (file, span) = (self.design.decl_data(decl).file, self.design.type_node(self_ty).span);
            self.coded(file, "E0134", span, "`impl Trait for` a bundle is refused: a trait is a capability of a nature or a value type (§3.1.9)".to_string(), "a bundle");
        }
    }

    /// What the self type `self_ty` of an `impl` names.
    fn subject(&self, self_ty: crate::pom::TypeId) -> Subject {
        match self.design.type_resolution(self_ty) {
            Resolution::Decl(target) => match &self.design.decl_data(target).kind {
                DeclKind::Nature(_) => Subject::Nature,
                DeclKind::Struct(_) if Decl::new(self.design, target).as_struct().is_some_and(|s| s.is_bundle()) => Subject::Bundle,
                _ => Subject::Other,
            },
            _ => Subject::Other,
        }
    }
}
