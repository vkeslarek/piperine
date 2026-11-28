//! Lowering of value code (§1.5, §3): [`ValueLowering`] and the state it keeps.

mod aggregate;
mod closures;
pub(super) mod compound;
mod control;
mod derived;
mod exits;
pub(super) mod expressions;
pub(super) mod files;
mod imported;
mod intrinsics;
mod iterators;
mod levels;
mod methods;
pub(super) mod patterns;
mod repr;
mod returns;
pub(super) mod state;
mod table_model;
pub(super) mod tables;
mod types;

use std::ops::{Deref, DerefMut};

use super::elaborator::Elaboration;

/// Lowers value code (§1.5, §3): the owner of the methods of `elab/value/`. It borrows the whole run: lowering is mutually recursive and reaches the other concerns.
pub(in crate::elab) struct ValueLowering<'e, 'd, 't> {
    /// The run this view lowers through.
    elaboration: &'e mut Elaboration<'d, 't>,
}

impl<'e, 'd, 't> ValueLowering<'e, 'd, 't> {
    /// The value lowering of `elaboration`.
    pub(in crate::elab) fn new(elaboration: &'e mut Elaboration<'d, 't>) -> Self {
        ValueLowering { elaboration }
    }
}

impl<'d, 't> Deref for ValueLowering<'_, 'd, 't> {
    type Target = Elaboration<'d, 't>;

    /// The run the view lowers through: what is no concern of its own (reporting, emitting, the other concerns) is asked of it.
    fn deref(&self) -> &Elaboration<'d, 't> {
        self.elaboration
    }
}

impl<'d, 't> DerefMut for ValueLowering<'_, 'd, 't> {
    /// The run the view lowers through, for writing.
    fn deref_mut(&mut self) -> &mut Elaboration<'d, 't> {
        self.elaboration
    }
}
