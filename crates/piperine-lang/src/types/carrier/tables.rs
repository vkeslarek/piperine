//! The state of discrete carriers: what the module being checked drives, and how many definitions the check is inside.

use crate::msl::Span;
use crate::pom::{DeclId, FileId};

use super::definition::DriveBook;
use super::element_target::Element;

/// What checking discrete carriers (§12, §13) keeps: what the module being checked drives, and how many definitions
/// the check is inside.
#[derive(Default)]
pub(in crate::types) struct CarrierTables {
    /// What the module being checked drives, by definition and by commit.
    pub(super) book: DriveBook,
    /// How many definitions the check is inside: their values are not read from the solve (`classes.rs`).
    pub(super) in_definition: u32,
}

impl CarrierTables {
    /// Whether the check is inside a definition.
    pub(in crate::types) fn inside_definition(&self) -> bool {
        self.in_definition > 0
    }

    /// Record that `element` of the carrier `decl`, written at `span` of `file`, is handed to an `out` port of an instance.
    pub(in crate::types) fn hand_over(&mut self, decl: DeclId, element: Element, span: Span, file: FileId) {
        self.book.handed.push((decl, element, span, file));
    }

    /// Whether the carrier `decl` is already handed over at `span`.
    pub(in crate::types) fn is_handed_over_at(&self, decl: DeclId, span: Span) -> bool {
        self.book.handed.iter().any(|(held, _, at, _)| *held == decl && *at == span)
    }

    /// Take the carriers handed to an `out` port, in source order.
    pub(in crate::types) fn take_handed(&mut self) -> Vec<(DeclId, Element, Span, FileId)> {
        std::mem::take(&mut self.book.handed)
    }

    /// Whether the module drives the whole carrier `decl`, by a definition or a commit.
    pub(in crate::types) fn drives_whole(&self, decl: DeclId) -> bool {
        self.book.defined.contains_key(&decl) || self.book.committed.contains_key(&decl)
    }

    /// Whether the module drives an element of the carrier `decl` that overlaps `element`.
    pub(in crate::types) fn drives_element(&self, decl: DeclId, element: Element) -> bool {
        self.book.elements.iter().any(|(carrier, driven, _, _)| *carrier == decl && driven.overlaps(element))
    }

    /// Whether a definition without a delay drives the carrier `decl`.
    pub(in crate::types) fn is_driven_without_delay(&self, decl: DeclId) -> bool {
        self.book.undelayed.contains(&decl)
    }

    /// Take what the module being checked drives, for a module checked ahead of the queue.
    pub(in crate::types) fn save(&mut self) -> DriveBook {
        std::mem::take(&mut self.book)
    }

    /// Put back what [`CarrierTables::save`] took.
    pub(in crate::types) fn restore(&mut self, book: DriveBook) {
        self.book = book;
    }

    /// Forget what was driven: a new module starts.
    pub(in crate::types) fn reset_book(&mut self) {
        self.book = DriveBook::default();
    }
}
