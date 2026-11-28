//! Typing of value expressions (§1.5, §3): [`ExpressionTyping`] and the state it keeps.

pub(super) mod aggregate;
pub(super) mod assign;
mod associated;
mod call;
mod class;
mod classes;
mod closures;
mod compound;
mod control;
pub(super) mod enums;
mod exits;
mod expr;
pub(super) mod flow;
mod functions;
pub(super) mod intrinsic;
mod iterators;
pub(super) mod lengths;
mod literals;
mod lowering;
mod loops;
mod methods;
mod operators;
mod plain;
pub(super) mod tables;
pub(super) mod widths;

use std::ops::{Deref, DerefMut};
use crate::types::check::Checker;

/// Types value expressions (§1.5, §3): the owner of the methods of `types/value/`. It borrows the whole checker: typing is
/// mutually recursive and reaching a declaration re-enters the declaration worklist.
pub(in crate::types) struct ExpressionTyping<'c, 'd> {
    /// The checker this view types through.
    checker: &'c mut Checker<'d>,
}

impl<'c, 'd> ExpressionTyping<'c, 'd> {
    /// The expression typing of `checker`.
    pub(in crate::types) fn new(checker: &'c mut Checker<'d>) -> Self {
        ExpressionTyping { checker }
    }
}

impl<'d> Deref for ExpressionTyping<'_, 'd> {
    type Target = Checker<'d>;

    /// The checker the view types through: what is not a concern of its own (reporting, the worklist) is asked of it.
    fn deref(&self) -> &Checker<'d> {
        self.checker
    }
}

impl<'d> DerefMut for ExpressionTyping<'_, 'd> {
    /// The checker the view types through, for writing.
    fn deref_mut(&mut self) -> &mut Checker<'d> {
        self.checker
    }
}
