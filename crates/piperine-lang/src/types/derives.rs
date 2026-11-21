//! The traits of Appendix H.8 and the types that implement them (docs/spec Part I, H.8 rules 2 to 7).
//!
//! A struct or an enum implements a trait exactly when it derives it, and a derive holds only when
//! every field implements the trait (rule 6) and the traits it requires are derived too (`Eq` and
//! `PartialOrd` need `PartialEq`, `Ord` needs both). The built-in types implement them by rule 7.
//! `Default` is never derived on an enum (rule 4). `Clone` and `Copy` are not traits: a value is
//! copied where it is used (§1.4.6), so the table holds seven.

use crate::msl::Span;
use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, FileId, Resolution};

use super::aggregate::FieldType;
use super::check::Checker;
use super::Ty;

/// One trait of the table of H.8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DerivedTrait {
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
    pub(super) const ALL: [DerivedTrait; 7] = [Self::PartialEq, Self::Eq, Self::PartialOrd, Self::Ord, Self::Default, Self::Debug, Self::Hash];

    /// The trait's spelling in `#[derive(..)]` and in a bound.
    pub(super) fn name(self) -> &'static str {
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

    /// The traits a type must implement before it implements this one (H.8 rules 2 and 3).
    fn requires(self) -> &'static [DerivedTrait] {
        match self {
            Self::Eq | Self::PartialOrd => &[Self::PartialEq],
            Self::Ord => &[Self::Eq, Self::PartialOrd],
            Self::PartialEq | Self::Default | Self::Debug | Self::Hash => &[],
        }
    }

    /// Whether `f64` implements it: no `Eq`, `Ord` or `Hash`, as NaN is not equal to itself (H.8 rule 7).
    fn of_float(self) -> bool {
        !matches!(self, Self::Eq | Self::Ord | Self::Hash)
    }

    /// Whether `str` implements it: everything but `Default` (H.8 rule 7).
    fn of_text(self) -> bool {
        self != Self::Default
    }
}

impl Checker<'_> {
    /// Whether `ty` implements `which` (H.8 rule 7 for the built-in types, a derive for a struct
    /// or an enum; the derive's own validity is judged where the type is declared).
    pub(super) fn implements(&self, ty: &Ty, which: DerivedTrait) -> bool {
        match ty {
            Ty::Bool | Ty::Unit | Ty::UInt(_) | Ty::Int(_) => true,
            Ty::F64 => which.of_float(),
            Ty::Str => which.of_text(),
            // H.8 rule 4: the default of an `Option` is `None`, whatever its payload.
            Ty::Option(_) if which == DerivedTrait::Default => true,
            Ty::Option(inner) | Ty::Array(inner, _) => self.implements(inner, which),
            Ty::Tuple(items) => items.iter().all(|item| self.implements(item, which)),
            Ty::Struct(decl) | Ty::Enum(decl) => self.derives(*decl, which.name()),
            Ty::Fn(..) => which == DerivedTrait::Debug,
            Ty::Node(_) | Ty::AnyNode | Ty::Module(_) | Ty::Carrier { .. } | Ty::Event | Ty::Never | Ty::Range { .. } | Ty::Iterator { .. } => false,
        }
    }

    /// Judges every derive of the struct or enum `decl` whose fields are `fields` (H.8 rules 4 and
    /// 6): `Default` on an enum, a required trait left out, and a field without the trait are
    /// `E0180` naming the trait. `false` when one is refused.
    pub(super) fn judge_derives(&mut self, decl: DeclId, file: FileId, span: Span, fields: &[Ty]) -> bool {
        let is_enum = matches!(self.design.decl_data(decl).kind, DeclKind::Enum(_));
        let name = if is_enum { Ty::Enum(decl) } else { Ty::Struct(decl) }.name(self.design);
        let mut clean = true;
        let derived: Vec<DerivedTrait> = DerivedTrait::ALL.into_iter().filter(|t| self.derives(decl, t.name())).collect();
        for which in derived {
            let trait_name = which.name();
            if is_enum && which == DerivedTrait::Default {
                self.derive_refused(file, span, format!("`#[derive(Default)]` on {name}: an enum does not derive `Default` (H.8 rule 4)"));
                clean = false;
            }
            if let Some(missing) = which.requires().iter().find(|needed| !self.derives(decl, needed.name())) {
                self.derive_refused(file, span, format!("`#[derive({trait_name})]` on {name} needs `#[derive({})]` too (H.8 rules 2 and 3)", missing.name()));
                clean = false;
            }
            if let Some(field) = fields.iter().find(|field| !self.implements(field, which)) {
                let field = field.name(self.design);
                self.derive_refused(file, span, format!("`#[derive({trait_name})]` on {name}: a field of type `{field}` does not implement `{trait_name}` (H.8 rule 6)"));
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
    pub(super) fn require_trait(&mut self, file: FileId, span: Span, ty: &Ty, which: DerivedTrait) {
        if !self.implements(ty, which) {
            let found = ty.name(self.design);
            self.mismatch_text(file, span, &format!("a type that derives `{}`", which.name()), &found);
        }
    }

    /// `T::default()` (H.8 rule 4): the call has no argument and `T` must implement `Default`; the
    /// call's type is `T`, recorded for the elaborator. `None` when `callee` is not such a path.
    pub(super) fn default_call(&mut self, expr: ExprId, callee: ExprId, argument_count: usize) -> Option<Option<Ty>> {
        let ExprKind::Path(path) = &self.design.expr_node(callee).kind else { return None };
        let segments = self.design.path_node(*path).segments.clone();
        let [owner, member] = segments.as_slice() else { return None };
        if member.name.text != "default" || !matches!(member.res, Resolution::Deferred(_) | Resolution::Builtin(_)) {
            return None;
        }
        let span = self.design.expr_node(expr).span;
        let file = self.file_of_expr(expr);
        let Some(ty) = self.path_owner_type(file, span, owner.res) else { return Some(None) };
        if argument_count != 0 {
            self.report(file, crate::msl::Diagnostic::new("E0141", format!("`default` takes no argument, found {argument_count}"), span, "no argument expected"));
            return Some(None);
        }
        self.require_trait(file, span, &ty, DerivedTrait::Default);
        self.defaults.insert(expr, ty.clone());
        Some(Some(ty))
    }
}

impl Checker<'_> {
    /// The number of levels of `ty`'s `Repr` (§2 table): the built-in layouts, a tuple, an array and
    /// an `Option` over parts that have one, a struct or an enum that derives `Repr`. `None` for a
    /// type without `Repr`.
    pub(super) fn repr_levels(&mut self, ty: &Ty) -> Option<u32> {
        match ty {
            Ty::Bool | Ty::F64 | Ty::UInt(_) | Ty::Int(_) => ty.levels(self.design),
            Ty::Tuple(items) => self.repr_sum(items),
            Ty::Array(element, length) => u32::try_from(*length).ok()?.checked_mul(self.repr_levels(element)?),
            Ty::Option(inner) => self.repr_levels(inner)?.checked_add(1),
            Ty::Struct(decl) if self.derives(*decl, "Repr") => {
                let fields: Vec<Ty> = self.fields_of(*decl)?.into_iter().map(|f| f.ty).collect();
                self.repr_sum(&fields)
            }
            Ty::Enum(decl) => self.enum_levels(*decl),
            _ => None,
        }
    }

    /// The levels of `parts` laid end to end (§2 rule 2); `None` when one has no `Repr`.
    fn repr_sum(&mut self, parts: &[Ty]) -> Option<u32> {
        parts.iter().try_fold(0u32, |total, part| total.checked_add(self.repr_levels(part)?))
    }

    /// The levels of a derived enum: the tag, then the widest payload (§2 rules 2, 5 and 6).
    fn enum_levels(&mut self, decl: DeclId) -> Option<u32> {
        if !self.derives(decl, "Repr") {
            return None;
        }
        let variants = self.variants_of(decl);
        let tag = match self.repr_width(decl) {
            None => usize::BITS - variants.len().saturating_sub(1).leading_zeros(),
            Some(Some(width)) => width,
            Some(None) => return None,
        };
        let mut widest = 0u32;
        for variant in variants {
            let fields: Vec<Ty> = self.fields_of(variant)?.into_iter().map(|f| f.ty).collect();
            widest = widest.max(self.repr_sum(&fields)?);
        }
        tag.checked_add(widest)
    }

    /// `#[derive(Repr)]` on `decl` whose fields are `fields` requires every field to have a `Repr`
    /// (§2 rule 3): a field without one is `E0110` naming it. `false` when one is refused.
    pub(super) fn judge_repr(&mut self, decl: DeclId, file: FileId, fields: &[FieldType]) -> bool {
        if !self.derives(decl, "Repr") {
            return true;
        }
        let mut clean = true;
        for (position, field) in fields.iter().enumerate() {
            if self.repr_levels(&field.ty).is_some() {
                continue;
            }
            let span = self.design.decl_data(field.decl).span;
            let name = field.name.clone().unwrap_or_else(|| position.to_string());
            let ty = field.ty.name(self.design);
            self.coded(file, "E0110", span, format!("the field `{name}` has type `{ty}`, which has no `Repr`: `#[derive(Repr)]` needs a field that does (§2 rule 3)"), "a field without `Repr`");
            clean = false;
        }
        clean
    }
}
