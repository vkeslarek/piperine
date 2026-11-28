//! Typing of generic items (§3.8): [`GenericsSolver`] and the state it keeps.

pub(super) mod bounds;
mod generic_values;
pub(super) mod generics;
pub(super) mod inference;
mod module_bounds;
mod module_generics;
pub(super) mod obligations;
pub(super) mod substitution;
pub(super) mod tables;

use std::ops::{Deref, DerefMut};

use crate::types::check::Checker;

/// Solves generic items (§3.8): the owner of the methods of `types/generics/`. It borrows the whole checker, because typing is mutually recursive and reaching a declaration re-enters the
/// declaration worklist.
pub(in crate::types) struct GenericsSolver<'c, 'd> {
    /// The checker this view types through.
    checker: &'c mut Checker<'d>,
}

impl<'c, 'd> GenericsSolver<'c, 'd> {
    /// The generics solver of `checker`.
    pub(in crate::types) fn new(checker: &'c mut Checker<'d>) -> Self {
        GenericsSolver { checker }
    }
}

impl<'d> Deref for GenericsSolver<'_, 'd> {
    type Target = Checker<'d>;

    /// The checker the view types through: what is no concern of its own (reporting, the worklist) is asked of it.
    fn deref(&self) -> &Checker<'d> {
        self.checker
    }
}

impl<'d> DerefMut for GenericsSolver<'_, 'd> {
    /// The checker the view types through, for writing.
    fn deref_mut(&mut self) -> &mut Checker<'d> {
        self.checker
    }
}
