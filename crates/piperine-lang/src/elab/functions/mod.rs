//! Lowering of functions (§3.4): [`FunctionLowering`] and the state it keeps.

mod declarations;
mod events;
mod handles;
pub(super) mod instances;
pub(super) mod state;
pub(super) mod variants;

use std::ops::{Deref, DerefMut};

use super::elaborator::Elaboration;

/// Lowers functions (§3.4): the owner of the methods of `elab/functions/`. It borrows the whole run: a call reaches a function and lowers it where the call stands.
pub(in crate::elab) struct FunctionLowering<'e, 'd, 't> {
    /// The run this view lowers through.
    elaboration: &'e mut Elaboration<'d, 't>,
}

impl<'e, 'd, 't> FunctionLowering<'e, 'd, 't> {
    /// The function lowering of `elaboration`.
    pub(in crate::elab) fn new(elaboration: &'e mut Elaboration<'d, 't>) -> Self {
        FunctionLowering { elaboration }
    }
}

impl<'d, 't> Deref for FunctionLowering<'_, 'd, 't> {
    type Target = Elaboration<'d, 't>;

    /// The run the view lowers through: what is no concern of its own (reporting, emitting, the other concerns) is asked of it.
    fn deref(&self) -> &Elaboration<'d, 't> {
        self.elaboration
    }
}

impl<'d, 't> DerefMut for FunctionLowering<'_, 'd, 't> {
    /// The run the view lowers through, for writing.
    fn deref_mut(&mut self) -> &mut Elaboration<'d, 't> {
        self.elaboration
    }
}
