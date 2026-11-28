//! The `impl Resolve<X, V> for N` of a discrete nature (§4 rule 8, resolved-nets RN-02): the one
//! declaration that makes a nature resolved. `E0203` holds each `impl` to its shape, once, at the
//! `impl` (generics Q1 (c)); a drive of the nature picks the `impl` whose `X` it is (`E0180`,
//! `E0181`, §5.2.15).

use crate::msl::{Diagnostic, Span};
use crate::pom::{DeclId, DeclKind, ExprId, FileId, GenericParamData, NatureFieldNode, NatureKind, Resolution};

use super::check::Checker;
use super::generics::generics::GenericSignature;
use super::{ConstTerm, ImplEntry, ImplTarget, Length, Ty};

/// The name of the built-in trait whose `impl`s make a nature resolved.
const RESOLVE_TRAIT: &str = "Resolve";

/// One `impl Resolve<X, V> for N` of a nature: the type of one drive and the type of the value
/// the drives combine into.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolveImpl {
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

    /// The `impl`s of `nature` whose arguments type, in declaration order. The one-argument form
    /// `impl Resolve<V>` has `X = V` (spec Q4).
    pub(super) fn of(&mut self, nature: DeclId) -> Vec<ResolveImpl> {
        if matches!(self.checker.state.design.decl_data(nature).kind, DeclKind::GenericParam(_)) {
            return self.of_parameter(nature);
        }
        let table = std::rc::Rc::clone(&self.checker.state.traits);
        let entries: Vec<ImplEntry> = table.impls().iter().filter(|entry| entry.builtin_trait == Some(RESOLVE_TRAIT) && entry.target == ImplTarget::Declared(nature)).cloned().collect();
        entries.iter().filter_map(|entry| self.typed(entry)).collect()
    }

    /// What the `Resolve<X, V>` bound of the nature parameter `parameter` says: one entry, whose `implementation` is the
    /// parameter itself (the concrete `impl` is the instance's nature's, found by `X` when the instance is lowered).
    fn of_parameter(&mut self, parameter: DeclId) -> Vec<ResolveImpl> {
        let Some(arguments) = GenericSignature::resolve_bound(self.checker.state.design, parameter) else { return Vec::new() };
        let types: Option<Vec<Ty>> = arguments.iter().map(|argument| self.checker.lower_type(*argument)).collect();
        match types.as_deref() {
            Some([value]) => vec![ResolveImpl { implementation: parameter, driven: value.clone(), value: value.clone() }],
            Some([driven, value]) => vec![ResolveImpl { implementation: parameter, driven: driven.clone(), value: value.clone() }],
            _ => Vec::new(),
        }
    }

    /// The `impl` of `nature` the drive `value` picks (§5.2.15): the one whose `X` is the type of
    /// the expression. A type no `impl` takes is `E0180` naming the nature and the types its
    /// `impl`s take; a literal several `impl`s accept is `E0181`. The pick is recorded for the
    /// elaborator; `None` after a report.
    pub(super) fn pick(&mut self, nature: DeclId, value: ExprId) -> Option<ResolveImpl> {
        let design = self.checker.state.design;
        let impls = self.of(nature);
        let nature_name = design.decl_data(nature).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let takes = impls.iter().map(|candidate| format!("`{}`", candidate.driven.name(design))).collect::<Vec<_>>().join(", ");
        let (file, span) = (self.checker.file_of_expr(value), design.expr_node(value).span);
        let picked = match impls.as_slice() {
            [] => return None,
            [only] => {
                let mark = self.checker.diagnostics_mark();
                self.checker.expressions().check_as(value, &only.driven, "E0180");
                self.checker.annotate_since(mark, "E0180", &format!(": a drive of `{nature_name}` is a `{}` (its `impl Resolve`, §5.2.15)", only.driven.name(design)));
                only.clone()
            }
            several => {
                let bare = self.checker.expressions().integer_literal_text(value).filter(|text| text.chars().all(|c| c.is_ascii_digit() || c == '_'));
                let accepting: Vec<&ResolveImpl> = match &bare {
                    Some(_) => several.iter().filter(|candidate| matches!(candidate.driven, Ty::UInt(_) | Ty::Int(_))).collect(),
                    None => {
                        let found = self.checker.expressions().infer(value)?;
                        several.iter().filter(|candidate| found.fits(&candidate.driven)).collect()
                    }
                };
                match accepting.as_slice() {
                    [] => {
                        let message = format!("a drive of `{nature_name}` has no `impl Resolve` that takes it: the `impl`s of `{nature_name}` take {takes} (§5.2.15)");
                        self.checker.coded(file, "E0180", span, message, "no `impl Resolve` takes this type");
                        return None;
                    }
                    [one] => {
                        if bare.is_some() {
                            self.checker.expressions().check_as(value, &one.driven, "E0180");
                        }
                        (*one).clone()
                    }
                    ambiguous => {
                        let names = ambiguous.iter().map(|candidate| format!("`{}`", candidate.driven.name(design))).collect::<Vec<_>>().join(", ");
                        let message = format!("this literal fits more than one `impl Resolve` of `{nature_name}`: {names}; write its type (§5.2.15)");
                        self.checker.coded(file, "E0181", span, message, "an ambiguous literal");
                        self.checker.expressions().infer(value);
                        return None;
                    }
                }
            }
        };
        self.checker.state.result.drive_impls.insert(value, picked.implementation);
        Some(picked)
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
        let design = self.checker.state.design;
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
        let table = std::rc::Rc::clone(&self.checker.state.traits);
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
        let design = self.checker.state.design;
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
        let design = self.checker.state.design;
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
        self.check_purity(function, head);
        let Some((parameters, result)) = self.checker.declared_signature(function) else { return };
        let takes_drives = matches!(parameters.as_slice(), [Ty::Array(element, Length::Generic(ConstTerm::Param(counted)))] if **element == typed.driven && counted == length);
        if !takes_drives {
            self.refuse(function_file, function_span, &format!("{head}: `resolve` takes one parameter `[{}; N]`, the drives (§4 rule 8)", typed.driven.name(design)));
        } else if result != typed.value {
            self.refuse(function_file, function_span, &format!("{head}: `resolve` returns `{}`, and `V` is `{}` (§4 rule 8)", result.name(design), typed.value.name(design)));
        }
    }

    /// `E0203` when `function`, or a function it reaches, reads the environment of the solve or
    /// emits: `resolve` MUST be of class pure (§4 rule 8), since it runs wherever drives combine.
    fn check_purity(&mut self, function: DeclId, head: &str) {
        let design = self.checker.state.design;
        let class = self.checker.expressions().function_class(function, &mut std::collections::BTreeSet::new());
        let impure = if class.solve_reading {
            "solve-reading"
        } else if class.emitting {
            "emitting"
        } else {
            return;
        };
        self.refuse(design.decl_data(function).file, design.decl_data(function).span, &format!("{head}: `resolve` is {impure}, and it MUST be pure (§4 rule 8)"));
    }

    /// One `E0203` at `span`.
    fn refuse(&mut self, file: FileId, span: Span, message: &str) {
        self.checker.coded(file, "E0203", span, message.to_string(), "not an `impl Resolve<X, V>`");
    }
}

impl Checker<'_> {
    /// The value of a definition or a commit to `target`: a carrier of a resolved nature takes the
    /// type of the `impl Resolve` its value picks (`ResolveImpls::pick`), any other the type it
    /// holds, mismatching with `code`.
    pub(super) fn check_drive_value(&mut self, target: &super::element_target::DriveTarget, value: ExprId, code: &'static str) {
        let resolved = match self.state.result.carriers.get(&target.carrier).map(Ty::innermost) {
            Some(Ty::Carrier { nature, .. }) if super::nature_rules::NatureRules::of(self.state.design, *nature).resolved && target.element == super::element_target::Element::Whole => Some(*nature),
            _ => None,
        };
        match resolved {
            Some(nature) => {
                if let Some(picked) = ResolveImpls::new(self).pick(nature, value) {
                    self.state.result.carrier_drive_impls.entry(target.carrier).or_default().push(picked.implementation);
                }
            }
            None => self.expressions().check_as(value, &target.held, code),
        }
    }
}

impl Checker<'_> {
    /// The initial value `init` of the resolved carrier `carrier` is the module's first drive of it, at `(0, 0)`: it
    /// picks the `impl Resolve` its type names like any drive (§5.2.3, §5.2.15).
    pub(super) fn resolved_initial(&mut self, carrier: DeclId, nature: DeclId, init: ExprId) {
        self.within(super::check::Context::Const, |this| {
            if let Some(picked) = ResolveImpls::new(this).pick(nature, init) {
                this.state.result.carrier_drive_impls.entry(carrier).or_default().push(picked.implementation);
            }
        });
    }

    /// `E0187` for a resolved carrier this module drives with no initial value whose driven type has no `Default`
    /// (§5.2.12): the drive has a value from `(0, 0)` on. An undelayed definition gives it one.
    pub(super) fn missing_resolved_initial_value(&mut self, carrier: DeclId) {
        let driven_here = self.state.carriers.book.defined.contains_key(&carrier) || self.state.carriers.book.committed.contains_key(&carrier);
        if !driven_here || self.has_initial(carrier) || self.state.carriers.book.undelayed.contains(&carrier) {
            return;
        }
        let Some(implementation) = self.state.result.carrier_drive_impls.get(&carrier).and_then(|picked| picked.first().copied()) else { return };
        let Some(Ty::Carrier { nature, .. }) = self.state.result.carriers.get(&carrier).cloned() else { return };
        let Some(driven) = self.state.result.resolve_tables.get(&nature).and_then(|table| table.iter().find(|candidate| candidate.implementation == implementation)).map(|candidate| candidate.driven.clone()) else { return };
        // a hand-written `Default` is a function the elaborator folds
        self.reach_written_defaults(&driven);
        if self.trait_typing().implements(&driven, super::traits::derives::DerivedTrait::Default) {
            return;
        }
        let item = self.state.design.decl_data(carrier).clone();
        let name = item.name.as_ref().map_or("", |n| n.text.as_str()).to_string();
        let found = driven.name(self.state.design);
        let message = format!("`{name}` has no initial value and its drive type `{found}` does not implement `Default`: a drive has a value from (0, 0) on (§5.2.12)");
        self.report(item.file, Diagnostic::new("E0187", message, item.span, "no initial value").with_help(format!("write `= value` after the type of `{name}`, or drive an `Option<{found}>`")));
    }
}
