//! Typing of patterns (§1.9): [`PatternTyping`].

mod aggregate_patterns;
pub(super) mod cover;
pub(super) mod logic_pattern;
mod patterns;

use std::ops::{Deref, DerefMut};

use crate::types::check::Checker;

/// Types patterns (§1.9): the owner of the methods of `types/pattern/`, a view with no table of its own, since a pattern
/// reads the control state, the locals and the layouts that other concerns keep. It borrows the whole checker, because
/// typing is mutually recursive and reaching a declaration re-enters the declaration worklist.
pub(in crate::types) struct PatternTyping<'c, 'd> {
    /// The checker this view types through.
    checker: &'c mut Checker<'d>,
}

impl<'c, 'd> PatternTyping<'c, 'd> {
    /// The pattern typing of `checker`.
    pub(in crate::types) fn new(checker: &'c mut Checker<'d>) -> Self {
        PatternTyping { checker }
    }
}

impl<'d> Deref for PatternTyping<'_, 'd> {
    type Target = Checker<'d>;

    /// The checker the view types through: what is no concern of its own (reporting, the worklist) is asked of it.
    fn deref(&self) -> &Checker<'d> {
        self.checker
    }
}

impl<'d> DerefMut for PatternTyping<'_, 'd> {
    /// The checker the view types through, for writing.
    fn deref_mut(&mut self) -> &mut Checker<'d> {
        self.checker
    }
}
