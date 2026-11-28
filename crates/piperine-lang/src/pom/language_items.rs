//! The language items: the declarations of `std` the compiler gives a meaning to (code-clarity design.md, CC-11).
//!
//! Each one is found **once**, by its path from the root of `std`, when the design is resolved; from then on the
//! compiler compares declaration ids, never names. A model's own item of the same name is another declaration with
//! another id, so it is the model's (shadowing needs no rule). This file holds the only table from a `std` path to a
//! [`LanguageItem`]; a later item is one more variant, one more arm of [`LanguageItem::path`] and of
//! [`LanguageItem::is_declared_by`], and one more entry of [`LanguageItem::ALL`].

use std::collections::BTreeMap;

use super::decl::DeclKind;
use super::design::Design;
use super::ids::DeclId;

/// A `std` item the compiler gives meaning to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LanguageItem {
    /// `std::io::parse_table`, the table reader of §22.6.4.
    ParseTable,
    /// `std::io::parse_hex`, the hex image reader of §22.6.4.
    ParseHex,
    /// `std::analog::table_model`, the interpolated table of §22.2.15.
    TableModel,
    /// `std::values::logic`, the function that makes a `Logic<N>` from a pattern (§22.1.4).
    LogicFunction,
    /// `std::values::matches_bits`, the pattern test of §22.1.4.
    MatchesBits,
    /// `std::logic::Logic`, the four-state struct of §22.1.
    Logic,
}

impl LanguageItem {
    /// Every language item.
    pub const ALL: [LanguageItem; 6] =
        [LanguageItem::ParseTable, LanguageItem::ParseHex, LanguageItem::TableModel, LanguageItem::LogicFunction, LanguageItem::MatchesBits, LanguageItem::Logic];

    /// The path of the item's declaration from the root of `std`.
    pub fn path(self) -> &'static str {
        match self {
            LanguageItem::ParseTable => "io::parse_table",
            LanguageItem::ParseHex => "io::parse_hex",
            LanguageItem::TableModel => "analog::table_model",
            LanguageItem::LogicFunction => "values::logic",
            LanguageItem::MatchesBits => "values::matches_bits",
            LanguageItem::Logic => "logic::Logic",
        }
    }

    /// Whether a declaration of kind `kind` can be this item: a function for the five functions, a struct for `Logic`.
    fn is_declared_by(self, kind: &DeclKind) -> bool {
        match self {
            LanguageItem::ParseTable | LanguageItem::ParseHex | LanguageItem::TableModel | LanguageItem::LogicFunction | LanguageItem::MatchesBits => {
                matches!(kind, DeclKind::Fn(_))
            }
            LanguageItem::Logic => matches!(kind, DeclKind::Struct(_)),
        }
    }
}

/// The declarations of the language items of one design, found once from `std`'s own files.
///
/// A `std` written for a test may declare only some of them: an item it does not declare is absent here, and a use
/// of it by the compiler is an internal error (`P0004`) at that use, never a fallback.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LanguageItems {
    /// The declaration of each item `std` declares.
    by_item: BTreeMap<LanguageItem, DeclId>,
    /// The item each of those declarations is.
    by_declaration: BTreeMap<DeclId, LanguageItem>,
}

impl LanguageItems {
    /// The language items `design`'s `std` declares, each at its [`LanguageItem::path`] and of its kind.
    pub(crate) fn of(design: &Design) -> LanguageItems {
        let standard = design.std_crate();
        let mut items = LanguageItems::default();
        for item in LanguageItem::ALL {
            let Some(declaration) = standard.find(item.path()).filter(|declaration| item.is_declared_by(declaration.kind())) else { continue };
            items.by_item.insert(item, declaration.id());
            items.by_declaration.insert(declaration.id(), item);
        }
        items
    }

    /// The declaration of `item`, `None` when `std` does not declare it.
    pub fn declaration(&self, item: LanguageItem) -> Option<DeclId> {
        self.by_item.get(&item).copied()
    }

    /// The language item `declaration` is, `None` for any other declaration (a model's own among them).
    pub fn item(&self, declaration: DeclId) -> Option<LanguageItem> {
        self.by_declaration.get(&declaration).copied()
    }
}
