//! The declaration rules of `impl` blocks (part I §3.1, §3.7, GN-03, GN-04, GN-10), checked on every
//! `impl` the check sees (the whole-model check sees them all): a method may not repeat the name of
//! a carrier method (`E0131`), an `impl` on a nature or a bundle holds functions only (`E0132`),
//! `impl Trait for` a bundle is `E0134` while a nature takes trait impls (traits are capabilities,
//! §3.1.9). A module type takes `impl`s too (§3.1.4), its functions pure (`E0122`).

use crate::pom::{Decl, DeclId, DeclKind, Reflect, Resolution};

use super::check::Checker;
use super::Ty;

/// The names of the methods of a carrier, which no `impl` may repeat (§3.1.3).
const CARRIER_METHOD_NAMES: [&str; 4] = ["changed", "prev", "others", "inflow"];

/// What an `impl` is written for, as far as its rules go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Subject {
    /// A nature.
    Nature,
    /// A struct with a carrier field.
    Bundle,
    /// A module type (§3.1.4): its `impl` functions are pure functions of the template.
    Module,
    /// Anything else: a value type, a generic parameter.
    Other,
}

impl Checker<'_> {
    /// Holds the `impl` `decl` to the rules of §3.1: `E0131` for a method named like a carrier method,
    /// `E0132` for an item that is not a function in an inherent `impl` on a nature or a bundle,
    /// `E0134` for a trait `impl` on a bundle.
    pub(super) fn coherence(&mut self, decl: DeclId) {
        let DeclKind::Impl(data) = &self.state.design.decl_data(decl).kind else { return };
        let (self_ty, inherent) = (data.self_ty, data.trait_.is_none());
        let subject = self.subject(self_ty);
        let children = self.state.design.decl_data(decl).children.clone();
        for child in children {
            let item = self.state.design.decl_data(child);
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
        if subject == Subject::Module {
            self.module_impl_purity(decl);
        }
        if !inherent && subject == Subject::Bundle {
            let (file, span) = (self.state.design.decl_data(decl).file, self.state.design.type_node(self_ty).span);
            self.coded(file, "E0134", span, "`impl Trait for` a bundle is refused: a trait is a capability of a nature or a value type (§3.1.9)".to_string(), "a bundle");
        } else if !inherent {
            self.trait_impl_rules(decl);
            self.overlap_rule(decl);
        }
    }

    /// §3.1.4 (DD3): the functions of an `impl` on a module type are pure functions of the template, run at elaboration, so one
    /// that probes or reads the environment of the solve is `E0122` naming it.
    fn module_impl_purity(&mut self, decl: DeclId) {
        for child in self.state.design.decl_data(decl).children.clone() {
            if !matches!(self.state.design.decl_data(child).kind, DeclKind::Fn(_)) || !self.expressions().function_class(child, &mut std::collections::BTreeSet::new()).solve_reading {
                continue;
            }
            let item = self.state.design.decl_data(child);
            let (file, span, name) = (item.file, item.span, item.name.as_ref().map_or_else(String::new, |n| n.text.clone()));
            self.coded(file, "E0122", span, format!("the function `{name}` of an `impl` on a module type is solve-reading (it probes or reads the environment of the solve), and such a function is a pure function of the template run at elaboration (§3.1.4)"), "a module impl function out of its class");
        }
    }

    /// §3.7.6: for one trait and one type at most one `impl` applies, used or not. Two `impl`s of a
    /// trait that have a type in common, and write the trait with the same arguments, are `E0187`
    /// at the later one, naming the trait and the type.
    fn overlap_rule(&mut self, decl: DeclId) {
        let table = std::rc::Rc::clone(&self.state.traits);
        let Some(position) = table.impls().iter().position(|entry| entry.decl == decl) else { return };
        let current = table.impls()[position].clone();
        // a built-in trait with an `impl` (H.10: the operators, `From`, `PartialEq`, `Default`) is judged like a declared one
        let written = current.builtin_trait.filter(|name| *name != "Repr");
        if current.trait_decl.is_none() && written.is_none() {
            return;
        }
        let (file, span) = (self.state.design.decl_data(decl).file, self.state.design.decl_data(decl).span);
        for earlier in &table.impls()[..position] {
            if earlier.trait_decl != current.trait_decl || earlier.builtin_trait != current.builtin_trait || !table.overlap(earlier, &current, &mut |ty, bound| super::bounds::BoundCheck::new(self).satisfies(ty, bound)) {
                continue;
            }
            // for a built-in trait the one argument that tells two `impl`s apart is the `Rhs` (H.10 rule 2), or the source of a `From`
            let mut arguments = |entry: &super::ImplEntry| -> Option<Vec<Ty>> {
                let listed = if written.is_some() { &entry.trait_arguments[..entry.trait_arguments.len().min(1)] } else { &entry.trait_arguments[..] };
                listed.iter().map(|argument| self.lower_type(*argument)).collect()
            };
            let given = arguments(&current);
            let before = arguments(earlier);
            if given != before {
                continue;
            }
            let name = match (current.trait_decl, written) {
                (Some(tr), _) => self.state.design.decl_data(tr).name.as_ref().map(|n| n.text.clone()).unwrap_or_default(),
                (None, Some(name)) => name.to_string(),
                (None, None) => String::new(),
            };
            let target = match &current.target {
                super::ImplTarget::Declared(decl) => self.state.design.decl_data(*decl).name.as_ref().map_or_else(String::new, |n| n.text.clone()),
                super::ImplTarget::Primitive(ty) => ty.name(self.state.design),
                _ => "a type its bound admits".to_string(),
            };
            self.coded(file, "E0187", span, format!("two `impl`s of `{name}` apply to `{target}`: at most one may (§3.7.6)"), "an impl that overlaps an earlier one");
            return;
        }
    }

    /// The rules of `impl Tr for T` (§3.7.3, §3.7.4, §3.7.7, §3.7.11), each `E0187` naming the trait and
    /// the type: the `impl` is in the crate of the trait or of the type, every supertrait is
    /// implemented, every item of the trait without a default is defined and none the trait lacks is,
    /// each with the trait's signature under `Self = T` and the trait's arguments, and no item is `pub`.
    fn trait_impl_rules(&mut self, decl: DeclId) {
        let DeclKind::Impl(data) = self.state.design.decl_data(decl).kind.clone() else { return };
        let Some(path) = data.trait_ else { return };
        if self.state.traits.impls().iter().any(|entry| entry.decl == decl && entry.builtin_trait == Some("Repr")) {
            self.repr_impl_rules(decl, data.self_ty);
            return;
        }
        if self.state.traits.impls().iter().any(|entry| entry.decl == decl && entry.builtin_trait == Some("Resolve")) {
            super::nature::ResolveImpls::new(self).check(decl);
            return;
        }
        if self.state.traits.impls().iter().any(|entry| entry.decl == decl && entry.builtin_trait.is_some()) {
            self.expressions().builtin_impl_rules(decl, data.self_ty);
            return;
        }
        let Some(Resolution::Decl(trait_decl)) = self.state.design.path_node(path).segments.last().map(|segment| segment.res) else { return };
        let (file, span) = (self.state.design.decl_data(decl).file, self.state.design.decl_data(decl).span);
        let Some(self_ty) = self.lower_type(data.self_ty) else { return };
        let trait_name = self.state.design.decl_data(trait_decl).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let head = format!("`impl {trait_name} for {}`", self_ty.source_name(self.state.design));
        self.impl_crate_rule(decl, trait_decl, data.self_ty, &head, file, span);
        self.supertrait_rule(trait_decl, &self_ty, &head, file, span);
        let substitution = self.trait_substitution(trait_decl, path, &self_ty, file, span);
        let (trait_items, impl_items) = (self.state.design.decl_data(trait_decl).children.clone(), self.state.design.decl_data(decl).children.clone());
        let items = |owner: &[DeclId], this: &Self| -> Vec<(String, DeclId)> {
            owner.iter().filter(|child| matches!(this.state.design.decl_data(**child).kind, DeclKind::Fn(_) | DeclKind::Const(_))).filter_map(|child| Some((this.state.design.decl_data(*child).name.as_ref()?.text.clone(), *child))).collect()
        };
        let (declared, defined) = (items(&trait_items, self), items(&impl_items, self));
        for (name, item) in &declared {
            if !defined.iter().any(|(own, _)| own == name) && !self.state.traits.has_default(*item) {
                self.coded(file, "E0187", span, format!("{head} leaves out `{name}`, which the trait has no default for (§3.7.4)"), "an item the trait requires");
            }
        }
        for (name, item) in &defined {
            let (item_file, item_span) = (self.state.design.decl_data(*item).file, self.state.design.decl_data(*item).span);
            if self.state.design.decl_data(*item).public {
                self.coded(item_file, "E0187", item_span, format!("{head}: the item `{name}` is written `pub`, and an item of a trait impl has the visibility of the trait (§3.7.11)"), "`pub` in a trait impl");
            }
            match declared.iter().find(|(declared, _)| declared == name) {
                None => self.coded(item_file, "E0187", item_span, format!("{head} defines `{name}`, which the trait does not have (§3.7.4)"), "an item the trait lacks"),
                Some((_, required)) => self.signature_rule(*required, *item, substitution.as_ref(), &head, name),
            }
        }
    }

    /// §3.7.7: the `impl` is in the crate that declares the trait or in the crate that declares the type.
    fn impl_crate_rule(&mut self, decl: DeclId, trait_decl: DeclId, self_ty: crate::pom::TypeId, head: &str, file: crate::pom::FileId, span: crate::msl::Span) {
        let krate = |item: DeclId| Decl::new(self.state.design, item).krate().id();
        let type_crate = match self.state.design.type_resolution(self_ty) {
            Resolution::Decl(target) if matches!(self.state.design.decl_data(target).kind, DeclKind::Struct(_) | DeclKind::Enum(_) | DeclKind::Nature(_)) => Some(krate(target)),
            _ => None,
        };
        if krate(decl) != krate(trait_decl) && type_crate != Some(krate(decl)) {
            self.coded(file, "E0187", span, format!("{head} is written in a crate that declares neither the trait nor the type (§3.7.7)"), "an impl in a foreign crate");
        }
    }

    /// §3.7.3: the type implements every supertrait of the trait.
    fn supertrait_rule(&mut self, trait_decl: DeclId, self_ty: &super::Ty, head: &str, file: crate::pom::FileId, span: crate::msl::Span) {
        let table = std::rc::Rc::clone(&self.state.traits);
        for supertrait in table.supertraits(trait_decl) {
            if !table.implements(self_ty, supertrait, &mut |ty, bound| super::bounds::BoundCheck::new(self).satisfies(ty, bound)) {
                let name = self.state.design.decl_data(supertrait).name.as_ref().map_or_else(String::new, |n| n.text.clone());
                self.coded(file, "E0187", span, format!("{head}: `{}` does not implement the supertrait `{name}` (§3.7.3)", self_ty.source_name(self.state.design)), "a supertrait that is not implemented");
            }
        }
    }

    /// What the trait's own types stand for in an `impl`: `Self` is the type and each generic parameter
    /// of the trait the argument the `impl` writes (`impl Resolve<Driven> for Wire`) or its default.
    fn trait_substitution(&mut self, trait_decl: DeclId, path: crate::pom::PathId, self_ty: &super::Ty, file: crate::pom::FileId, span: crate::msl::Span) -> Option<super::Substitution> {
        let signature = super::GenericSignature::of(self.state.design, trait_decl);
        let nodes = match self.state.design.path_node(path).segments.last().and_then(|segment| segment.args.clone()) {
            Some(crate::pom::GenericArgsNode::Angle { args, .. }) => args,
            _ => Vec::new(),
        };
        let mut substitution = super::Substitution::new();
        substitution.bind(trait_decl, super::GenericValue::Type(self_ty.clone()));
        if !self.generic_argument_count(trait_decl, nodes.len(), file, span) {
            return None;
        }
        for (index, parameter) in signature.parameters().iter().enumerate() {
            let value = match nodes.get(index) {
                Some(node) => self.generic_argument(parameter, node, file)?,
                None => self.default_argument(parameter, &substitution, file)?,
            };
            substitution.bind(parameter.decl, value);
        }
        Some(substitution)
    }

    /// §3.7.4: the `impl`'s item has the signature the trait gives it under `Self = T`.
    fn signature_rule(&mut self, required: DeclId, defined: DeclId, substitution: Option<&super::Substitution>, head: &str, name: &str) {
        let (file, span) = (self.state.design.decl_data(defined).file, self.state.design.decl_data(defined).span);
        let Some(substitution) = substitution else { return };
        let (wanted, found) = match (&self.state.design.decl_data(required).kind, &self.state.design.decl_data(defined).kind) {
            (DeclKind::Fn(_), DeclKind::Fn(_)) => {
                let (Some((wanted_params, wanted_ret)), Some((found_params, found_ret))) = (self.declared_signature(required), self.declared_signature(defined)) else { return };
                let wanted = Ty::Fn(wanted_params.iter().map(|ty| self.substitute(substitution, ty)).collect(), Box::new(self.substitute(substitution, &wanted_ret)));
                (wanted, Ty::Fn(found_params, Box::new(found_ret)))
            }
            (DeclKind::Const(wanted), DeclKind::Const(found)) => {
                let (wanted, found) = (wanted.ty, found.ty);
                let (Some(wanted), Some(found)) = (self.lower_type(wanted), self.lower_type(found)) else { return };
                (self.substitute(substitution, &wanted), found)
            }
            _ => {
                self.coded(file, "E0187", span, format!("{head}: `{name}` is not the kind of item the trait declares (§3.7.4)"), "a function for a constant or the reverse");
                return;
            }
        };
        if !found.fits(&wanted) {
            self.coded(file, "E0187", span, format!("{head}: `{name}` has the type `{}`, the trait gives it `{}` (§3.7.4)", found.name(self.state.design), wanted.name(self.state.design)), "a signature the trait does not give");
        }
    }

    /// What the self type `self_ty` of an `impl` names.
    fn subject(&self, self_ty: crate::pom::TypeId) -> Subject {
        match self.state.design.type_resolution(self_ty) {
            Resolution::Decl(target) => match &self.state.design.decl_data(target).kind {
                DeclKind::Nature(_) => Subject::Nature,
                DeclKind::Struct(_) if Decl::new(self.state.design, target).as_struct().is_some_and(|s| s.is_bundle()) => Subject::Bundle,
                DeclKind::Module(_) => Subject::Module,
                _ => Subject::Other,
            },
            _ => Subject::Other,
        }
    }
}
