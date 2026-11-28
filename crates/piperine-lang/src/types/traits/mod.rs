//! Typing of traits and `impl`s (§3.7): [`TraitTyping`] and the table it reads.

mod coherence;
pub(super) mod derives;
mod repr;
pub(super) mod impl_table;

use std::ops::{Deref, DerefMut};

use crate::types::check::Checker;

/// Judges traits and `impl`s (§3.7): the owner of the checker's methods of `types/traits/`, over the trait table (`ImplTable`) the check builds once. It borrows the whole checker, because typing is mutually recursive and reaching a declaration re-enters the
/// declaration worklist.
pub(in crate::types) struct TraitTyping<'c, 'd> {
    /// The checker this view types through.
    checker: &'c mut Checker<'d>,
}

impl<'c, 'd> TraitTyping<'c, 'd> {
    /// The trait typing of `checker`.
    pub(in crate::types) fn new(checker: &'c mut Checker<'d>) -> Self {
        TraitTyping { checker }
    }
}

impl<'d> Deref for TraitTyping<'_, 'd> {
    type Target = Checker<'d>;

    /// The checker the view types through: what is no concern of its own (reporting, the worklist) is asked of it.
    fn deref(&self) -> &Checker<'d> {
        self.checker
    }
}

impl<'d> DerefMut for TraitTyping<'_, 'd> {
    /// The checker the view types through, for writing.
    fn deref_mut(&mut self) -> &mut Checker<'d> {
        self.checker
    }
}
