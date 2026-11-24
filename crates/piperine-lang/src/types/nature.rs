//! The `impl Resolve<X, V> for N` of a discrete nature (§4 rule 8, resolved-nets RN-02): the one
//! declaration that makes a nature resolved. `E0203` holds each `impl` to its shape, once, at the
//! `impl` (generics Q1 (c)); a drive of the nature picks the `impl` whose `X` it is (`E0180`,
//! `E0181`, §5.2.15).

use crate::msl::Span;
use crate::pom::{DeclId, DeclKind, FileId, GenericParamData, NatureFieldNode, NatureKind, Resolution};

use super::check::Checker;
use super::{ConstTerm, ImplEntry, ImplTarget, Length, Ty};

/// The name of the built-in trait whose `impl`s make a nature resolved.
const RESOLVE_TRAIT: &str = "Resolve";

/// One `impl Resolve<X, V> for N` of a nature: the type of one drive and the type of the value
/// the drives combine into.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ResolveImpl {
    /// The `impl` declaration.
    pub implementation: DeclId,
    /// The type `X` of one drive.
    pub driven: Ty,
    /// The type `V` of the combined value, the nature's `value`.
    pub value: Ty,
}

/// The `impl Resolve`s of a design, read through the checker: the one owner of "which `impl`
/// resolves this nature" and of the `E0203` shape rule.
pub(super) struct ResolveImpls<'checker, 'design> {
    checker: &'checker mut Checker<'design>,
}

impl<'checker, 'design> ResolveImpls<'checker, 'design> {
    /// The `impl Resolve`s asked through `checker`.
    pub(super) fn new(checker: &'checker mut Checker<'design>) -> Self {
        ResolveImpls { checker }
    }

    /// The `impl Resolve` blocks written for `nature`, in declaration order.
    pub(super) fn declarations(impls: &super::ImplTable<'_>, nature: DeclId) -> Vec<DeclId> {
        impls.impls().iter().filter(|entry| entry.builtin_trait == Some(RESOLVE_TRAIT) && entry.target == ImplTarget::Declared(nature)).map(|entry| entry.decl).collect()
    }

    /// The `impl` of `entry` with its argument types, `None` when an argument has no type or the
    /// list is neither `<V>` nor `<X, V>` (`E0203` reports the latter).
    fn typed(&mut self, entry: &ImplEntry) -> Option<ResolveImpl> {
        let types: Option<Vec<Ty>> = entry.trait_arguments.iter().map(|argument| self.checker.lower_type(*argument)).collect();
        match types?.as_slice() {
            [value] => Some(ResolveImpl { implementation: entry.decl, driven: value.clone(), value: value.clone() }),
            [driven, value] => Some(ResolveImpl { implementation: entry.decl, driven: driven.clone(), value: value.clone() }),
            _ => None,
        }
    }

    /// `E0203` at the `impl Resolve` `decl` when it is not `impl Resolve<X, V> for N` of a discrete
    /// nature `N` with `V` its `value`, holding one `resolve` with one const parameter `N: usize`,
    /// one parameter `[X; N]` and the result `V` (§4 rule 8).
    pub(super) fn check(&mut self, decl: DeclId) {
        let design = self.checker.design;
        let DeclKind::Impl(data) = &design.decl_data(decl).kind else { return };
        let (file, span) = (design.decl_data(decl).file, design.decl_data(decl).span);
        let Some(nature) = (match design.type_resolution(data.self_ty) {
            Resolution::Decl(target) if matches!(&design.decl_data(target).kind, DeclKind::Nature(nature) if nature.kind == NatureKind::Discrete) => Some(target),
            _ => None,
        }) else {
            self.refuse(file, span, "`impl Resolve` is written for a discrete nature (§4 rule 8)");
            return;
        };
        let nature_name = design.decl_data(nature).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let head = format!("`impl Resolve for {nature_name}`");
        let table = std::rc::Rc::clone(&self.checker.impls);
        let Some(entry) = table.impls().iter().find(|entry| entry.decl == decl) else { return };
        let Some(typed) = self.typed(entry) else {
            if entry.trait_arguments.len() != 1 && entry.trait_arguments.len() != 2 {
                self.refuse(file, span, &format!("{head} takes `<X, V>` (or `<V>`): the type of a drive and the nature's value (§4 rule 8)"));
            }
            return;
        };
        if let Some(value) = self.nature_value(nature)
            && value != typed.value
        {
            self.refuse(file, span, &format!("{head}: `V` is `{}`, and `{nature_name}` holds `{}` (§4 rule 8)", typed.value.name(design), value.name(design)));
        }
        self.check_function(decl, &head, &typed);
    }

    /// The type of the `value` field of `nature`, `None` when it is a type parameter of a generic
    /// nature (the carrier's type then depends on its arguments).
    fn nature_value(&mut self, nature: DeclId) -> Option<Ty> {
        let design = self.checker.design;
        let DeclKind::Nature(data) = &design.decl_data(nature).kind else { return None };
        let value = data.fields.iter().find_map(|field| match field {
            NatureFieldNode::Value { ty, .. } => Some(*ty),
            _ => None,
        })?;
        if matches!(design.type_resolution(value), Resolution::Decl(parameter) if matches!(design.decl_data(parameter).kind, DeclKind::GenericParam(_))) {
            return None;
        }
        self.checker.lower_type(value)
    }

    /// The `resolve` of the `impl` `decl`: the only function, `fn resolve<const N: usize>(drives: [X; N]) -> V`.
    fn check_function(&mut self, decl: DeclId, head: &str, typed: &ResolveImpl) {
        let design = self.checker.design;
        let (file, span) = (design.decl_data(decl).file, design.decl_data(decl).span);
        let items: Vec<DeclId> = design.decl_data(decl).children.iter().copied().filter(|child| matches!(design.decl_data(*child).kind, DeclKind::Fn(_) | DeclKind::Const(_))).collect();
        let named = |item: DeclId| design.decl_data(item).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
        let Some(function) = items.iter().copied().find(|item| named(*item) == "resolve" && matches!(design.decl_data(*item).kind, DeclKind::Fn(_))) else {
            self.refuse(file, span, &format!("{head} defines no `resolve` (§4 rule 8)"));
            return;
        };
        for other in items.iter().copied().filter(|item| *item != function) {
            let (other_file, other_span) = (design.decl_data(other).file, design.decl_data(other).span);
            self.refuse(other_file, other_span, &format!("{head} defines `{}`, which `Resolve` does not have (§4 rule 8)", named(other)));
        }
        let (function_file, function_span) = (design.decl_data(function).file, design.decl_data(function).span);
        let constants: Vec<DeclId> = design.decl_data(function).children.iter().copied().filter(|child| matches!(design.decl_data(*child).kind, DeclKind::GenericParam(_))).collect();
        let [length] = constants.as_slice() else {
            self.refuse(function_file, function_span, &format!("{head}: `resolve` takes one const parameter `N: usize`, the number of drives (§4 rule 8)"));
            return;
        };
        let counts_drives = matches!(&design.decl_data(*length).kind, DeclKind::GenericParam(GenericParamData::Const { ty, .. }) if matches!(self.checker.lower_type(*ty), Some(Ty::UInt(64))));
        if !counts_drives {
            self.refuse(function_file, function_span, &format!("{head}: the const parameter of `resolve` is `usize` (§4 rule 8)"));
            return;
        }
        let Some((parameters, result)) = self.checker.declared_signature(function) else { return };
        let takes_drives = matches!(parameters.as_slice(), [Ty::Array(element, Length::Generic(ConstTerm::Param(counted)))] if **element == typed.driven && counted == length);
        if !takes_drives {
            self.refuse(function_file, function_span, &format!("{head}: `resolve` takes one parameter `[{}; N]`, the drives (§4 rule 8)", typed.driven.name(design)));
        } else if result != typed.value {
            self.refuse(function_file, function_span, &format!("{head}: `resolve` returns `{}`, and `V` is `{}` (§4 rule 8)", result.name(design), typed.value.name(design)));
        }
    }

    /// One `E0203` at `span`.
    fn refuse(&mut self, file: FileId, span: Span, message: &str) {
        self.checker.coded(file, "E0203", span, message.to_string(), "not an `impl Resolve<X, V>`");
    }
}
