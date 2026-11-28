//! Typing of analog behaviour (§14): [`AnalogTyping`] and the state it keeps.

mod analog;
pub(super) mod continuity;
pub(super) mod nature;
pub(super) mod nature_rules;
pub(super) mod tables;

use std::ops::{Deref, DerefMut};

use crate::types::check::Checker;

/// Types analog behaviour (§14): the owner of the methods of `types/analog/`. It borrows the whole checker, because typing is mutually recursive and reaching a declaration re-enters the
/// declaration worklist.
pub(in crate::types) struct AnalogTyping<'c, 'd> {
    /// The checker this view types through.
    checker: &'c mut Checker<'d>,
}

impl<'c, 'd> AnalogTyping<'c, 'd> {
    /// The analog typing of `checker`.
    pub(in crate::types) fn new(checker: &'c mut Checker<'d>) -> Self {
        AnalogTyping { checker }
    }
}

impl<'d> Deref for AnalogTyping<'_, 'd> {
    type Target = Checker<'d>;

    /// The checker the view types through: what is no concern of its own (reporting, the worklist) is asked of it.
    fn deref(&self) -> &Checker<'d> {
        self.checker
    }
}

impl<'d> DerefMut for AnalogTyping<'_, 'd> {
    /// The checker the view types through, for writing.
    fn deref_mut(&mut self) -> &mut Checker<'d> {
        self.checker
    }
}
