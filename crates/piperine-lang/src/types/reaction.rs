//! Reactions (DIG-05, part IV §12.1): the head of an `on` is an `Event`, its body holds value
//! statements and commits.

use crate::pom::{FileId, ReactionData};

use super::check::Checker;
use super::Ty;

impl Checker<'_> {
    /// `on head { body }`: the head is an `Event` (`E0430`, §12.1.10).
    pub(super) fn reaction(&mut self, reaction: &ReactionData, file: FileId) {
        let span = self.design.expr_node(reaction.event).span;
        match self.infer_event_position(reaction.event) {
            Some(Ty::Event) | None => {}
            Some(other) => {
                let message = format!("the head of `on` has type `{}`: it must be an `Event`; a `bool` alone is not an event (§12.1.10)", other.name(self.design));
                self.coded(file, "E0430", span, message, "not an `Event`");
            }
        }
        if !reaction.body.stmts.is_empty() {
            self.unsupported(file, reaction.body.span, "a reaction body");
        }
    }
}
