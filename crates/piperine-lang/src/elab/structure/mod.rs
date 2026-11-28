//! Lowering of module structure (§8): [`StructureLowering`] and the state it keeps.

pub(super) mod body;
mod bundles;
mod children;
pub(super) mod generation;
mod generic_bodies;
pub(super) mod hoist;
pub(super) mod override_text;
mod placement;
pub(super) mod reach;
pub(super) mod state;
pub(super) mod structural;
pub(super) mod templates;

use std::ops::{Deref, DerefMut};

use super::elaborator::Elaboration;

/// Lowers module structure (§8): the owner of the methods of `elab/structure/`. It borrows the whole run: a body places children, which lower their own bodies and read through each other.
pub(in crate::elab) struct StructureLowering<'e, 'd, 't> {
    /// The run this view lowers through.
    elaboration: &'e mut Elaboration<'d, 't>,
}

impl<'e, 'd, 't> StructureLowering<'e, 'd, 't> {
    /// The structure lowering of `elaboration`.
    pub(in crate::elab) fn new(elaboration: &'e mut Elaboration<'d, 't>) -> Self {
        StructureLowering { elaboration }
    }
}

impl<'d, 't> Deref for StructureLowering<'_, 'd, 't> {
    type Target = Elaboration<'d, 't>;

    /// The run the view lowers through: what is no concern of its own (reporting, emitting, the other concerns) is asked of it.
    fn deref(&self) -> &Elaboration<'d, 't> {
        self.elaboration
    }
}

impl<'d, 't> DerefMut for StructureLowering<'_, 'd, 't> {
    /// The run the view lowers through, for writing.
    fn deref_mut(&mut self) -> &mut Elaboration<'d, 't> {
        self.elaboration
    }
}
