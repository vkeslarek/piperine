//! The traits of Appendix H.8 and the types that implement them (docs/spec Part I, H.8 rules 2 to 7).
//!
//! A struct or an enum implements a trait exactly when it derives it, and a derive holds only when
//! every field implements the trait (rule 6) and the traits it requires are derived too (`Eq` and
//! `PartialOrd` need `PartialEq`, `Ord` needs both). The built-in types implement them by rule 7.
//! `Default` is never derived on an enum (rule 4). `Clone` and `Copy` are not traits: a value is
//! copied where it is used (§1.4.6), so the table holds seven.

use crate::refusal::forms::GenericsForm;
use crate::msl::Span;
use crate::pom::{AttrArgNode, AttrInputNode, DeclId, DeclKind, Design, ExprId, ExprKind, FileId, Resolution};

use crate::types::value::aggregate::FieldType;
use super::TraitTyping;
use crate::types::{GenericValue, Length, Nominal, Ty};

/// One trait of the table of H.8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::types) enum DerivedTrait {
    /// `==` and `!=`.
    PartialEq,
    /// Every value equals itself; adds nothing a model calls.
    Eq,
    /// `<`, `<=`, `>` and `>=`.
    PartialOrd,
    /// Any two values are ordered; adds nothing a model calls.
    Ord,
    /// `T::default()`.
    Default,
    /// Accepted, no effect.
    Debug,
    /// Accepted, no effect.
    Hash,
}

impl DerivedTrait {
    /// The seven traits of H.8.
    pub(in crate::types) const ALL: [DerivedTrait; 7] = [Self::PartialEq, Self::Eq, Self::PartialOrd, Self::Ord, Self::Default, Self::Debug, Self::Hash];

    /// The trait's spelling in `#[derive(..)]` and in a bound.
    pub(in crate::types) fn name(self) -> &'static str {
        match self {
            Self::PartialEq => "PartialEq",
            Self::Eq => "Eq",
            Self::PartialOrd => "PartialOrd",
            Self::Ord => "Ord",
            Self::Default => "Default",
            Self::Debug => "Debug",
            Self::Hash => "Hash",
        }
    }

    /// Whether `decl` carries `#[derive(.., name, ..)]`: any trait name, `Repr` included.
    pub(in crate::types) fn is_listed(design: &Design, decl: DeclId, name: &str) -> bool {
        design.decl_data(decl).meta.attrs.iter().any(|attr| {
            let is_derive = matches!(design.path_node(attr.path).segments.as_slice(), [only] if only.name.text == "derive");
            is_derive && matches!(&attr.input, AttrInputNode::List(args) if args.iter().any(|arg| matches!(arg, AttrArgNode::Path(p) if design.path_node(*p).segments.last().is_some_and(|s| s.name.text == name))))
        })
    }

    /// Whether implementing this trait gives `which`: itself and every trait it requires (`Ord` gives
    /// `Eq`, `PartialOrd` and `PartialEq`, H.8 rules 2 and 3).
    pub(in crate::types) fn gives(self, which: DerivedTrait) -> bool {
        self == which || self.requires().iter().any(|required| required.gives(which))
    }

    /// The traits a type must implement before it implements this one (H.8 rules 2 and 3).
    fn requires(self) -> &'static [DerivedTrait] {
        match self {
            Self::Eq | Self::PartialOrd => &[Self::PartialEq],
            Self::Ord => &[Self::Eq, Self::PartialOrd],
            Self::PartialEq | Self::Default | Self::Debug | Self::Hash => &[],
        }
    }

    /// Whether `f64` implements it: no `Eq`, `Ord` or `Hash`, as NaN is not equal to itself, and no
    /// `Default`, as no number is the value of a carrier with none (H.8 rules 4 and 7).
    fn of_float(self) -> bool {
        !matches!(self, Self::Eq | Self::Ord | Self::Hash | Self::Default)
    }

    /// Whether `bool`, `UInt<N>` and `Int<N>` implement it: all but `Default` (H.8 rules 4 and 7).
    fn of_integer(self) -> bool {
        self != Self::Default
    }

    /// Whether `str` implements it: everything but `Default` (H.8 rule 7).
    fn of_text(self) -> bool {
        self != Self::Default
    }
}

impl TraitTyping<'_, '_> {
    /// Whether `ty` implements `which` (H.8 rules 4 and 7 for the built-in types, a derive for a struct
    /// or an enum; the derive's own validity is judged where the type is declared).
    pub(in crate::types) fn implements(&mut self, ty: &Ty, which: DerivedTrait) -> bool {
        match ty {
            Ty::Unit => true,
            Ty::Bool | Ty::UInt(_) | Ty::Int(_) | Ty::GenericInteger { .. } => which.of_integer(),
            Ty::F64 | Ty::F32 | Ty::F16 | Ty::F128 => which.of_float(),
            Ty::Str => which.of_text(),
            // H.8 rule 4: the default of an `Option` is `None`, whatever its payload.
            Ty::Option(_) if which == DerivedTrait::Default => true,
            Ty::Option(inner) | Ty::Array(inner, _) => self.implements(inner, which),
            Ty::Tuple(items) => items.iter().all(|item| self.implements(item, which)),
            // H.8 rule 6: a generic type implements a trait when it derives it and its arguments do
            Ty::Struct(nominal) | Ty::Enum(nominal) => {
                (self.expressions().derives(nominal.decl, which.name()) || self.written_impl(ty, which))
                    && nominal.arguments.iter().all(|argument| match argument {
                        GenericValue::Type(ty) => self.implements(ty, which),
                        _ => true,
                    })
            }
            Ty::Fn(..) => which == DerivedTrait::Debug,
            Ty::Node(_) | Ty::AnyNode | Ty::Module(_) | Ty::Carrier { .. } | Ty::Event | Ty::Never | Ty::Range { .. } | Ty::Iterator { .. } => false,
            // what a parameter implements is what its bounds give (§3 rule 9)
            Ty::Param(param) => self.generics().param_implements(*param, which),
        }
    }

    /// Whether the project writes the `impl` that gives `ty` the trait `which` itself: `impl Default for T`, or the plain
    /// `impl PartialEq for T` whose `==` gives a `bool` (H.8). No other trait of H.8 is written by hand.
    fn written_impl(&self, ty: &Ty, which: DerivedTrait) -> bool {
        match which {
            DerivedTrait::Default => self.state.traits.has_written_default(ty),
            DerivedTrait::PartialEq => self.state.traits.has_plain_partial_eq(ty),
            _ => false,
        }
    }

    /// Judges every derive of the struct or enum `decl` whose fields are `fields` (H.8 rules 4 and
    /// 6): a required trait left out, a field without the trait and `Default` on an enum are
    /// `E0180` naming the trait, and a field without `Default` is `E0187` naming the field and its
    /// type. `false` when one is refused.
    pub(in crate::types) fn judge_derives(&mut self, decl: DeclId, file: FileId, span: Span, fields: &[FieldType]) -> bool {
        let is_enum = matches!(self.state.design.decl_data(decl).kind, DeclKind::Enum(_));
        let name = if is_enum { Ty::enumeration(decl) } else { Ty::structure(decl) }.name(self.state.design);
        let mut clean = true;
        let derived: Vec<DerivedTrait> = DerivedTrait::ALL.into_iter().filter(|t| self.expressions().derives(decl, t.name())).collect();
        for which in derived {
            let trait_name = which.name();
            if is_enum && which == DerivedTrait::Default {
                self.derive_refused(file, span, format!("`#[derive(Default)]` on {name}: an enum does not derive `Default` (H.8 rule 4)"));
                clean = false;
            }
            if let Some(missing) = which.requires().iter().find(|needed| !self.expressions().derives(decl, needed.name())) {
                self.derive_refused(file, span, format!("`#[derive({trait_name})]` on {name} needs `#[derive({})]` too (H.8 rules 2 and 3)", missing.name()));
                clean = false;
            }
            if let Some(field) = fields.iter().find(|field| !field.ty.has_param() && !self.implements(&field.ty, which)) {
                let type_name = field.ty.name(self.state.design);
                if which == DerivedTrait::PartialEq && !self.state.traits.builtin_functions("PartialEq", "eq", &field.ty).is_empty() {
                    let field_name = field.name.as_deref().unwrap_or("a positional field");
                    let message = format!("`#[derive(PartialEq)]` on {name}: the field `{field_name}` of type `{type_name}` compares with an `impl PartialEq` whose result is not a `bool`, so the derived `==` has no `bool` to give (H.10, §22.1.2 rule 5)");
                    self.coded(file, "E0187", span, message, "a field whose `==` is not a `bool`");
                } else if which == DerivedTrait::Default {
                    let field_name = field.name.as_deref().unwrap_or("a positional field");
                    let message = format!("`#[derive(Default)]` on {name}: the field `{field_name}` of type `{type_name}` does not implement `Default` (H.8 rule 4)");
                    self.coded(file, "E0187", span, message, "a field without a default");
                } else {
                    self.derive_refused(file, span, format!("`#[derive({trait_name})]` on {name}: a field of type `{type_name}` does not implement `{trait_name}` (H.8 rule 6)"));
                }
                clean = false;
            }
        }
        clean
    }

    /// A derive that does not hold: `E0180`, the type the position needs lacks the trait.
    fn derive_refused(&mut self, file: FileId, span: Span, message: String) {
        self.coded(file, "E0180", span, message, "a derive that does not hold");
    }

    /// `==` or `!=` on a value of `ty` needs `PartialEq`; an ordering operator needs `PartialOrd`
    /// (§1.3.10). Reports `E0180` naming the trait when `ty` lacks it.
    pub(in crate::types) fn require_trait(&mut self, file: FileId, span: Span, ty: &Ty, which: DerivedTrait) {
        if self.implements(ty, which) {
            return;
        }
        let found = ty.name(self.state.design);
        if let Ty::Param(_) = ty {
            let message = format!("mismatched types: `{found}` has no `{}`: the bounds of `{found}` do not give it (§3 rule 9), add `{found}: {}`", which.name(), which.name());
            self.coded(file, "E0180", span, message, "an operation no bound gives");
        } else if which == DerivedTrait::Default {
            self.coded(file, "E0187", span, format!("`{found}` does not implement `Default`: `{found}::default()` has no value (H.8 rule 4)"), "a type without a default");
        } else {
            self.mismatch_text(file, span, &format!("a type that derives `{}`", which.name()), &found);
        }
    }

    /// `T::default()` (H.8 rule 4): the call has no argument and `T` must implement `Default`; the
    /// call's type is `T`, recorded for the elaborator. `None` when `callee` is not such a path.
    pub(in crate::types) fn default_call(&mut self, expr: ExprId, callee: ExprId, argument_count: usize) -> Option<Option<Ty>> {
        let ExprKind::Path(path) = &self.state.design.expr_node(callee).kind else { return None };
        let segments = self.state.design.path_node(*path).segments.clone();
        let [owner, member] = segments.as_slice() else { return None };
        if member.name.text != "default" || !matches!(member.res, Resolution::Deferred(_) | Resolution::Builtin(_)) {
            return None;
        }
        let span = self.state.design.expr_node(expr).span;
        let file = self.file_of_expr(expr);
        let owner_type = if owner.name.text == "Option" && matches!(owner.res, Resolution::Builtin(_)) { self.option_owner_type(file, span, owner.args.as_ref()) } else { self.path_owner_type(file, span, owner.res) };
        let Some(ty) = owner_type else { return Some(None) };
        if argument_count != 0 {
            self.report(file, crate::msl::Diagnostic::new("E0141", format!("`default` takes no argument, found {argument_count}"), span, "no argument expected"));
            return Some(None);
        }
        self.require_trait(file, span, &ty, DerivedTrait::Default);
        self.state.result.defaults.insert(expr, ty.clone());
        Some(Some(ty))
    }
}

impl TraitTyping<'_, '_> {
    /// The type `Option::<T>` of the owner of `Option::<T>::default()` (H.8 rule 7): `Option` of the
    /// one type argument the turbofish gives. `None` after a diagnostic when it gives none.
    fn option_owner_type(&mut self, file: FileId, span: Span, args: Option<&crate::pom::GenericArgsNode>) -> Option<Ty> {
        match args {
            Some(crate::pom::GenericArgsNode::Angle { args, .. }) if let [crate::pom::GenericArgNode::Type(payload)] = args.as_slice() => Some(Ty::Option(Box::new(self.lower_type(*payload)?))),
            _ => {
                self.refuse(file, span, GenericsForm::OptionDefaultWithoutPayloadTypeWrittenAs);
                None
            }
        }
    }
}

impl TraitTyping<'_, '_> {
    /// The number of levels of `ty`'s `Repr` (§2 table): the built-in layouts, a tuple, an array and
    /// an `Option` over parts that have one, a struct or an enum that derives `Repr`. `None` for a
    /// type without `Repr`.
    pub(in crate::types) fn repr_levels(&mut self, ty: &Ty) -> Option<u32> {
        match ty {
            Ty::Bool | Ty::F64 | Ty::F32 | Ty::F16 | Ty::F128 | Ty::UInt(_) | Ty::Int(_) => ty.levels(self.state.design),
            Ty::Tuple(items) => self.repr_sum(items),
            Ty::Array(element, Length::Fixed(length)) => u32::try_from(*length).ok()?.checked_mul(self.repr_levels(element)?),
            Ty::Option(inner) => self.repr_levels(inner)?.checked_add(1),
            Ty::Struct(_) | Ty::Enum(_) if self.hand_written_levels(ty).is_some() => self.hand_written_levels(ty),
            Ty::Struct(nominal) if self.expressions().derives(nominal.decl, "Repr") => {
                let substitution = self.generics().nominal_substitution(nominal);
                let fields: Vec<Ty> = self.expressions().fields_of(nominal.decl)?.into_iter().map(|f| self.generics().substitute(&substitution, &f.ty)).collect();
                self.repr_sum(&fields)
            }
            Ty::Enum(nominal) => self.enum_levels(nominal),
            _ => None,
        }
    }

    /// The levels of `parts` laid end to end (§2 rule 2); `None` when one has no `Repr`.
    fn repr_sum(&mut self, parts: &[Ty]) -> Option<u32> {
        parts.iter().try_fold(0u32, |total, part| total.checked_add(self.repr_levels(part)?))
    }

    /// The levels of a derived enum: the tag, then the widest payload (§2 rules 2, 5 and 6).
    fn enum_levels(&mut self, nominal: &Nominal) -> Option<u32> {
        let decl = nominal.decl;
        if !self.expressions().derives(decl, "Repr") {
            return None;
        }
        let substitution = self.generics().nominal_substitution(nominal);
        let variants = self.expressions().variants_of(decl);
        let tag = match self.expressions().repr_width(decl) {
            None => usize::BITS - variants.len().saturating_sub(1).leading_zeros(),
            Some(Some(width)) => width,
            Some(None) => return None,
        };
        let mut widest = 0u32;
        for variant in variants {
            let fields: Vec<Ty> = self.expressions().fields_of(variant)?.into_iter().map(|f| self.generics().substitute(&substitution, &f.ty)).collect();
            widest = widest.max(self.repr_sum(&fields)?);
        }
        tag.checked_add(widest)
    }

    /// `#[derive(Repr)]` on `decl` whose fields are `fields` requires every field to have a `Repr`
    /// (§2 rule 3): a field without one is `E0110` naming it. `false` when one is refused.
    pub(in crate::types) fn judge_repr(&mut self, decl: DeclId, file: FileId, fields: &[FieldType]) -> bool {
        if !self.expressions().derives(decl, "Repr") {
            return true;
        }
        let mut clean = true;
        for (position, field) in fields.iter().enumerate() {
            // a field of a type parameter has a `Repr` where the instance has: each use checks its arguments
            if field.ty.has_param() || self.repr_levels(&field.ty).is_some() {
                continue;
            }
            let span = self.state.design.decl_data(field.decl).span;
            let name = field.name.clone().unwrap_or_else(|| position.to_string());
            let ty = field.ty.name(self.state.design);
            self.coded(file, "E0110", span, format!("the field `{name}` has type `{ty}`, which has no `Repr`: `#[derive(Repr)]` needs a field that does (§2 rule 3)"), "a field without `Repr`");
            clean = false;
        }
        clean
    }
}
