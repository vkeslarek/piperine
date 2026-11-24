//! Reactions (DIG-05, part IV §12.1): the head of an `on` is an `Event` (`E0430`), its body holds
//! value statements and commits `place <- value [after d];`. A commit has the targets of a
//! definition (`E0222`, `E0220`), the value type of its target (`E0432`) and an `f64` delay
//! (`E0433`); a carrier is driven by definitions or by commits of a module, never both (`E0221`).

use crate::pom::{ExprId, FileId, ReactionData};

use super::check::{Checker, Context};
use super::element_target::{DriveKind, DriveTarget, Element};
use super::Ty;

impl Checker<'_> {
    /// `on head { body }`.
    pub(super) fn reaction(&mut self, reaction: &ReactionData, file: FileId) {
        let span = self.design.expr_node(reaction.event).span;
        match self.infer_event_position(reaction.event) {
            Some(Ty::Event) | None => {}
            Some(other) => {
                let message = format!("the head of `on` has type `{}`: it must be an `Event`; a `bool` alone is not an event (§12.1.10)", other.name(self.design));
                self.coded(file, "E0430", span, message, "not an `Event`");
            }
        }
        let outer = std::mem::replace(&mut self.context, Context::Reaction);
        self.statements(&reaction.body.stmts, Some(&Ty::Unit));
        self.context = outer;
    }

    /// `place <- value [after delay];`
    pub(super) fn commit(&mut self, file: FileId, span: crate::msl::Span, place: ExprId, value: ExprId, after: Option<ExprId>) {
        if self.context != Context::Reaction {
            self.unsupported(file, span, "commit `<-` outside a reaction body");
            return;
        }
        let place_span = self.design.expr_node(place).span;
        let target = self.drive_target(place, "commit");
        match &target {
            // §1 rule 9: a function value or a closure has no `Repr`, so no carrier holds one
            Some(_) if self.is_function_value(value) => {
                let found = self.infer(value).map_or_else(|| "a function value".to_string(), |t| t.name(self.design));
                let message = format!("`{found}` has no `Repr`: a carrier holds only a type that implements it (part I §2)");
                self.coded(file, "E0112", self.design.expr_node(value).span, message, "no `Repr`");
            }
            Some(target) => self.check_drive_value(target, value, "E0432"),
            None => {
                self.infer(value);
            }
        }
        if let Some(after) = after {
            self.check_as(after, &Ty::F64, "E0433");
        }
        let Some(DriveTarget { carrier, element, reached, .. }) = target else { return };
        if reached {
            return;
        }
        let element_defined = self.book.elements.iter().any(|(c, e, k, _)| *c == carrier && *k == DriveKind::Definition && e.overlaps(element));
        if element != Element::Whole || self.book.elements.iter().any(|(c, _, k, _)| *c == carrier && *k == DriveKind::Definition) {
            if element_defined || (self.book.defined.contains_key(&carrier) && element == Element::Whole) {
                let name = self.design.decl_data(carrier).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
                self.coded(file, "E0221", place_span, format!("`{name}` has a definition and a commit of one element in one module: an element is driven by one or the other (§5.2, §8.2.20)"), "definition and commit");
            }
            if element != Element::Whole {
                self.book.elements.push((carrier, element, DriveKind::Commit, place_span));
                return;
            }
        }
        if self.book.defined.contains_key(&carrier) {
            let name = self.design.decl_data(carrier).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
            self.coded(file, "E0221", place_span, format!("`{name}` has a definition and a commit in one module: a carrier is driven by one or the other (§5.2, §8.2.20)"), "definition and commit");
        }
        self.book.committed.entry(carrier).or_insert(place_span);
    }
}
