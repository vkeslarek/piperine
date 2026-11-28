//! Typing of module bodies (§8): [`ModuleTyping`] and the state it keeps.

mod bundle;
mod bundle_handles;
mod generation;
mod handles;
mod module;
pub(super) mod param_requirement;
mod reach;
pub(super) mod record;
mod result;
mod template_items;
mod templates;
pub(super) mod tables;

use std::ops::{Deref, DerefMut};

use crate::types::check::Checker;

/// Types module bodies (§8): the owner of the methods of `types/module/`. It borrows the whole checker, because typing is mutually recursive and reaching a declaration re-enters the
/// declaration worklist.
pub(in crate::types) struct ModuleTyping<'c, 'd> {
    /// The checker this view types through.
    checker: &'c mut Checker<'d>,
}

impl<'c, 'd> ModuleTyping<'c, 'd> {
    /// The module typing of `checker`.
    pub(in crate::types) fn new(checker: &'c mut Checker<'d>) -> Self {
        ModuleTyping { checker }
    }
}

impl<'d> Deref for ModuleTyping<'_, 'd> {
    type Target = Checker<'d>;

    /// The checker the view types through: what is no concern of its own (reporting, the worklist) is asked of it.
    fn deref(&self) -> &Checker<'d> {
        self.checker
    }
}

impl<'d> DerefMut for ModuleTyping<'_, 'd> {
    /// The checker the view types through, for writing.
    fn deref_mut(&mut self) -> &mut Checker<'d> {
        self.checker
    }
}
