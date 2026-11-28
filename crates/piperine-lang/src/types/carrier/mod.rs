//! Typing of discrete carriers (§12, §13): [`CarrierTyping`] and the state it keeps.

mod carrier;
pub(super) mod coverage;
pub(super) mod definition;
pub(super) mod element_target;
mod event;
mod reaction;
pub(super) mod tables;

use std::ops::{Deref, DerefMut};

use crate::types::check::Checker;

/// Types discrete carriers, their definitions, events and reactions (§12, §13): the owner of the methods of `types/carrier/`. It borrows the whole checker, because typing is mutually recursive and reaching a declaration re-enters the
/// declaration worklist.
pub(in crate::types) struct CarrierTyping<'c, 'd> {
    /// The checker this view types through.
    checker: &'c mut Checker<'d>,
}

impl<'c, 'd> CarrierTyping<'c, 'd> {
    /// The carrier typing of `checker`.
    pub(in crate::types) fn new(checker: &'c mut Checker<'d>) -> Self {
        CarrierTyping { checker }
    }
}

impl<'d> Deref for CarrierTyping<'_, 'd> {
    type Target = Checker<'d>;

    /// The checker the view types through: what is no concern of its own (reporting, the worklist) is asked of it.
    fn deref(&self) -> &Checker<'d> {
        self.checker
    }
}

impl<'d> DerefMut for CarrierTyping<'_, 'd> {
    /// The checker the view types through, for writing.
    fn deref_mut(&mut self) -> &mut Checker<'d> {
        self.checker
    }
}
