//! Lowering of discrete code (§12, §13): [`DigitalLowering`] and the state it keeps.

mod carrier_arrays;
pub(super) mod carriers;
mod drive_loops;
pub(super) mod drive_overlaps;
mod drives;
pub(super) mod element_commits;
mod element_coverage;
pub(super) mod elements;
mod event_reads;
pub(super) mod events;
pub(super) mod loops;
mod reactions;
mod records;
pub(super) mod resolvers;
pub(super) mod state;

use std::ops::{Deref, DerefMut};

use super::elaborator::Elaboration;

/// Lowers discrete code (§12, §13): the owner of the methods of `elab/digital/`. It borrows the whole run: carriers, drives, events and reactions are read by the value code they sit in.
pub(in crate::elab) struct DigitalLowering<'e, 'd, 't> {
    /// The run this view lowers through.
    elaboration: &'e mut Elaboration<'d, 't>,
}

impl<'e, 'd, 't> DigitalLowering<'e, 'd, 't> {
    /// The digital lowering of `elaboration`.
    pub(in crate::elab) fn new(elaboration: &'e mut Elaboration<'d, 't>) -> Self {
        DigitalLowering { elaboration }
    }
}

impl<'d, 't> Deref for DigitalLowering<'_, 'd, 't> {
    type Target = Elaboration<'d, 't>;

    /// The run the view lowers through: what is no concern of its own (reporting, emitting, the other concerns) is asked of it.
    fn deref(&self) -> &Elaboration<'d, 't> {
        self.elaboration
    }
}

impl<'d, 't> DerefMut for DigitalLowering<'_, 'd, 't> {
    /// The run the view lowers through, for writing.
    fn deref_mut(&mut self) -> &mut Elaboration<'d, 't> {
        self.elaboration
    }
}
