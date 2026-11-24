//! Which Appendix H.8 traits a value type implements: the built-in types by rule 7, a struct or an
//! enum by what it derives. `Ir::check` asks it where an aggregate is compared (§1.3.10), where
//! `Default` builds one, and where a derive demands each field to implement the same trait (rule 6).

use super::Checker;
use crate::model::{Derives, TypeKind, ValueType};

/// One of the traits of H.8 a model may derive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Trait {
    /// `Clone`: a value is copied.
    Clone,
    /// `Copy`: a value is copied implicitly.
    Copy,
    /// `PartialEq`: `==` and `!=`.
    PartialEq,
    /// `Eq`: equality that holds for every value (no NaN).
    Eq,
    /// `PartialOrd`: `<`, `<=`, `>` and `>=`.
    PartialOrd,
    /// `Ord`: an order that holds for every value.
    Ord,
    /// `Default`: a value `T::default()`.
    Default,
    /// `Debug`: accepted, with no run-time effect.
    Debug,
    /// `Hash`: accepted, with no run-time effect.
    Hash,
}

impl Trait {
    /// Every trait, in the order H.8 lists them.
    pub(super) const ALL: [Trait; 9] = [Trait::Clone, Trait::Copy, Trait::PartialEq, Trait::Eq, Trait::PartialOrd, Trait::Ord, Trait::Default, Trait::Debug, Trait::Hash];

    /// The name as `#[derive(..)]` writes it.
    pub(super) fn name(self) -> &'static str {
        match self {
            Trait::Clone => "Clone",
            Trait::Copy => "Copy",
            Trait::PartialEq => "PartialEq",
            Trait::Eq => "Eq",
            Trait::PartialOrd => "PartialOrd",
            Trait::Ord => "Ord",
            Trait::Default => "Default",
            Trait::Debug => "Debug",
            Trait::Hash => "Hash",
        }
    }

    /// Whether `derives` states this trait.
    pub(super) fn derived(self, derives: &Derives) -> bool {
        match self {
            Trait::Clone => derives.clone,
            Trait::Copy => derives.copy,
            Trait::PartialEq => derives.partial_eq,
            Trait::Eq => derives.eq,
            Trait::PartialOrd => derives.partial_ord,
            Trait::Ord => derives.ord,
            Trait::Default => derives.default,
            Trait::Debug => derives.debug,
            Trait::Hash => derives.hash,
        }
    }
}

impl Checker<'_> {
    /// Whether a value of `ty` implements `which` (H.8 rule 7 for the built-in types, the derives
    /// for a struct or an enum). A type that reaches itself is reported elsewhere and counts as
    /// implementing it here, so the walk ends.
    pub(super) fn implements(&self, ty: &ValueType, which: Trait) -> bool {
        self.implements_visiting(ty, which, &mut Vec::new())
    }

    /// Whether `ty` implements `which`; `visiting` holds the arena types being walked, so a type that
    /// contains itself ends the walk.
    fn implements_visiting(&self, ty: &ValueType, which: Trait, visiting: &mut Vec<usize>) -> bool {
        match ty {
            ValueType::Bool | ValueType::Unit | ValueType::UInt(_) | ValueType::Int(_) => true,
            // A float has no `Eq`, `Ord` or `Hash`: NaN is not equal to itself (H.8 resolution).
            ValueType::F64 | ValueType::F32 | ValueType::F16 | ValueType::F128 => !matches!(which, Trait::Eq | Trait::Ord | Trait::Hash),
            ValueType::Fn(..) => matches!(which, Trait::Clone | Trait::Copy | Trait::Debug),
            ValueType::Tuple(parts) => parts.iter().all(|part| self.implements_visiting(part, which, visiting)),
            ValueType::Array(element, _) | ValueType::Option(element) => self.implements_visiting(element, which, visiting),
            ValueType::Struct(id) | ValueType::Enum(id) => {
                let Some(data) = self.ir.types.get(id.index()) else { return false };
                if visiting.contains(&id.index()) {
                    return true;
                }
                visiting.push(id.index());
                let fields_do = match &data.kind {
                    TypeKind::Struct { fields, .. } => fields.iter().all(|(_, field)| self.implements_visiting(field, which, visiting)),
                    TypeKind::Enum { variants, .. } => variants.iter().all(|v| v.fields.iter().all(|(_, field)| self.implements_visiting(field, which, visiting))),
                };
                visiting.pop();
                which.derived(&data.derives) && fields_do
            }
        }
    }
}
