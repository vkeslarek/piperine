//! Lowering of analog code (§14): [`AnalogLowering`] and the state it keeps.

mod blocks;
mod contribute;
mod delays;
mod derivatives;
pub(super) mod expansion;
pub(super) mod host_sources;
mod inflow;
mod nature;
mod node_choice;
mod noise;
mod partial;
pub(super) mod selectors;
mod session;
pub(super) mod signal_flow;
pub(super) mod state;
pub(super) mod symbolic;
pub(super) mod unroll;

use std::ops::{Deref, DerefMut};

use super::elaborator::Elaboration;

/// Lowers analog code (§14): the owner of the methods of `elab/analog/`. It borrows the whole run: analog code is value code with branches, sites and operators around it.
pub(in crate::elab) struct AnalogLowering<'e, 'd, 't> {
    /// The run this view lowers through.
    elaboration: &'e mut Elaboration<'d, 't>,
}

impl<'e, 'd, 't> AnalogLowering<'e, 'd, 't> {
    /// The analog lowering of `elaboration`.
    pub(in crate::elab) fn new(elaboration: &'e mut Elaboration<'d, 't>) -> Self {
        AnalogLowering { elaboration }
    }
}

impl<'d, 't> Deref for AnalogLowering<'_, 'd, 't> {
    type Target = Elaboration<'d, 't>;

    /// The run the view lowers through: what is no concern of its own (reporting, emitting, the other concerns) is asked of it.
    fn deref(&self) -> &Elaboration<'d, 't> {
        self.elaboration
    }
}

impl<'d, 't> DerefMut for AnalogLowering<'_, 'd, 't> {
    /// The run the view lowers through, for writing.
    fn deref_mut(&mut self) -> &mut Elaboration<'d, 't> {
        self.elaboration
    }
}
