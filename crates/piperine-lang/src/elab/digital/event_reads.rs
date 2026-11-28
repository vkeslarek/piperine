//! An event read as a `bool` (part IV §13 rules 2 and 9, design D7): in a reaction body and in a
//! guard an `Event` says whether it fired in this microstep, and lowers to `fired(k)` of an event
//! `k` of the body (`Expr::Fired`). `k` is an event the body already holds with the same source and
//! no guard (the head's own, first) or a new one appended to the body's events and to no reaction's
//! `on`: its pin and its function run every microstep like any other, so `fired(k)` is always
//! current.
//!
//! A read whose event has a guard (`ev && g`, an edge's own `rise`/`fall` condition) appends its
//! event: two equal guards are two pins that fire together, never two different values.

use super::DigitalLowering;
use piperine_ir::{Event, Expr, ExprId as LoweredExpr, Origin, ValueType};

use super::elements::Lookup;
use super::events::Disjunct;
use crate::elab::elaborator::Scope;
use crate::pom::{ExprId, ExprKind};
use crate::types::Builtin;

impl DigitalLowering<'_, '_, '_> {
    /// `expr`, an `Event` that the checker typed as a `bool` read (§13 rules 2 and 9): the
    /// disjunction, over the disjuncts of its event form, of `fired(k) && c` for each disjunct's
    /// event `k` and the continuous conjuncts `c` of its guard. `None` after a diagnostic.
    pub(in crate::elab) fn lower_event_read(&mut self, scope: &mut Scope, expr: ExprId) -> Option<LoweredExpr> {
        // `x[i].changed()` at an index only run time knows is not an event: it reads the element's change (CV-24)
        if let ExprKind::MethodCall { recv, .. } = &self.state.design.expr_node(self.peel_parens(expr)).kind.clone()
            && self.state.types.builtin(self.peel_parens(expr)) == Some(Builtin::Changed)
            && let Lookup::Found(element) = self.element_lookup(scope, *recv)
            && element.constant.is_none()
        {
            let origin = self.expr_origin(expr);
            return Some(self.element_changed(scope, element, origin));
        }
        let disjuncts = self.disjuncts(scope, expr)?;
        let mut arms = Vec::with_capacity(disjuncts.len());
        for disjunct in disjuncts {
            let event = self.read_event(&disjunct);
            arms.push(self.fired_arm(event, disjunct.continuous, disjunct.origin)?);
        }
        self.disjunction(&arms)
    }

    /// `fired(event) && c₁ && c₂ …`: the event fired and the continuous conjuncts of its guard
    /// hold (§13 rule 1). `None` only for an arm with no operand, which cannot happen.
    pub(super) fn fired_arm(&mut self, event: u32, continuous: Vec<LoweredExpr>, origin: Origin) -> Option<LoweredExpr> {
        let fired = self.emit(Expr::Fired(event), ValueType::Bool, origin);
        let operands: Vec<LoweredExpr> = std::iter::once(fired).chain(continuous).collect();
        self.conjunction(&operands)
    }

    /// The event of the body that `disjunct` reads: an unguarded one equal to it (§13 rule 2, the
    /// head's first), else one appended.
    fn read_event(&mut self, disjunct: &Disjunct) -> u32 {
        if disjunct.discrete.is_empty()
            && let Some(held) = self.state.digital.structure.events.iter().position(|e: &Event| e.guard.is_none() && e.source == disjunct.source)
        {
            return held as u32;
        }
        self.add_event(disjunct.source.clone(), &disjunct.discrete, disjunct.origin.clone())
    }
}
