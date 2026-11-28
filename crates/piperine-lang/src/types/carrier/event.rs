//! Events (DIG-05, part IV §13): `Ty::Event`, the forms that make one (`changed`, `rise`,
//! `fall`, `cross`, `at`, `||`, `&&` with a guard), where one may stand (an event position: the
//! head of an `on`, a body `let`, the result of a module, the value of a function, an operand of
//! `||` or `&&` in one of these) and the heads of the reactions of a `digital` block.
//!
//! An `Event` has no `Repr`. Written where only a value may stand it is `E0441`; in a guard and in
//! a reaction body it reads as a `bool` that says whether it fired in the microstep (part IV §13
//! rules 2 and 9), recorded as an event read for the elaborator.

use crate::refusal::forms::AnalogForm;
use crate::pom::{Arg, BehaviorKind, DeclId, DeclKind, ExprId};

use crate::types::check::Context;
use super::CarrierTyping;
use crate::types::{Builtin, Ty};

impl CarrierTyping<'_, '_> {
    /// `infer` of an expression written in an event position: an `Event` is allowed there.
    pub(in crate::types) fn infer_event_position(&mut self, expr: ExprId) -> Option<Ty> {
        self.state.values.set_event_slot(expr);
        self.expressions().infer(expr)
    }

    /// An `Event` typed where `expr` stands, outside an event position: where an event is read
    /// (a guard or a reaction body) it is a `bool` (§13 rules 2 and 9), anywhere else it is
    /// `E0441` (`E0354` when it is the Event result of a placement, §8.3.10). The type the
    /// expression then has is returned.
    pub(in crate::types) fn event_out_of_place(&mut self, expr: ExprId) -> Ty {
        if self.reads_events() {
            self.event_read_as_bool(expr)
        } else {
            if !self.modules().event_result_out_of_place(expr) {
                self.event_outside_a_position(expr);
            }
            Ty::Event
        }
    }

    /// Whether an `Event` stands for a `bool` here: inside a guard (§13 rule 9) and in a
    /// reaction body (§13 rule 2).
    fn reads_events(&self) -> bool {
        self.state.values.in_guard() || self.state.values.context() == Context::Reaction
    }

    /// `expr`, an `Event`, read as a `bool`: whether it fired in this microstep (§13 rule 2). The
    /// read is recorded for the elaborator (`Types::event_read`).
    pub(super) fn event_read_as_bool(&mut self, expr: ExprId) -> Ty {
        self.state.result.event_reads.insert(expr);
        Ty::Bool
    }

    /// `E0441` for an `Event` that stands where no event may and no read makes it a `bool`: it is
    /// never carried or stored (§13 rule 8).
    fn event_outside_a_position(&mut self, expr: ExprId) {
        let node = self.state.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        let message = "an `Event` value outside an event position, a guard or a reaction body: an event is never carried or stored (§13 rule 8)".to_string();
        self.coded(file, "E0441", node.span, message, "an `Event` outside an event position");
    }

    /// `E0441` for an event read where a type other than `bool` is expected, a commit onto a
    /// carrier of another type (§13 rule 8; the read of §13 rule 2 is a `bool` only).
    pub(in crate::types) fn event_where_another_type_is_expected(&mut self, expr: ExprId, expected: &Ty) {
        let node = self.state.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        let message = format!("an `Event` where a `{}` is expected: an event reads as a `bool` only, it is never carried or stored (§13 rules 2 and 8)", expected.name(self.state.design));
        self.coded(file, "E0441", node.span, message, "an `Event` where a value is expected");
    }

    /// `settle`: the core `Event` value of §14.9 (§13 rule 10). It has no operand to check; where
    /// it stands is judged by `infer` like any `Event`: `E0441` outside an event position, a
    /// guard or a reaction body (§13 rule 8), and combined with `&&` and `||` like any event
    /// (§13 rule 14) by `logical`.
    pub(in crate::types) fn settle_event(&self) -> Ty {
        Ty::Event
    }

    /// `a || b` and `a && b`. `a || b` is an `Event` when both operands are, and `ev && guard`
    /// is an `Event` when `ev` is and `guard` is a `bool`; such an `Event` is judged where it
    /// stands, like any other. Otherwise both operands are `bool`.
    pub(in crate::types) fn logical(&mut self, whole: ExprId, is_and: bool, lhs: ExprId, rhs: ExprId) -> Option<Ty> {
        let file = self.file_of_expr(whole);
        let span = self.state.design.expr_node(whole).span;
        let in_event_position = self.state.values.event_slot() == Some(whole);
        let left = self.infer_event_position(lhs);
        match (is_and, left) {
            (true, Some(Ty::Event)) => {
                let outer = self.state.values.replace_in_guard(true);
                self.expressions().check(rhs, &Ty::Bool);
                self.state.values.replace_in_guard(outer);
                Some(Ty::Event)
            }
            (false, Some(Ty::Event)) => {
                match self.infer_event_position(rhs) {
                    Some(Ty::Event) | None => {}
                    Some(other) => self.checker.mismatch(file, self.checker.state.design.expr_node(rhs).span, &Ty::Event, &other),
                }
                Some(Ty::Event)
            }
            // `bool && Event` in an event position says where the event goes (§13 rule 1).
            (_, Some(Ty::Bool)) if in_event_position => match self.infer_event_position(rhs) {
                Some(Ty::Event) if is_and => {
                    let message = "`&&` with a `bool` on the left and an `Event` on the right: the event goes on the left, `ev && guard` (§13 rule 1)".to_string();
                    self.coded(file, "E0440", span, message, "a `bool` guards nothing here");
                    None
                }
                Some(Ty::Event) => {
                    self.checker.mismatch(file, self.checker.state.design.expr_node(lhs).span, &Ty::Event, &Ty::Bool);
                    None
                }
                Some(Ty::Bool) => Some(Ty::Bool),
                Some(other) => {
                    self.checker.mismatch(file, self.checker.state.design.expr_node(rhs).span, &Ty::Bool, &other);
                    Some(Ty::Bool)
                }
                None => None,
            },
            (_, None) => {
                self.infer_event_position(rhs);
                None
            }
            (_, left) => {
                if let Some(other) = left.filter(|t| *t != Ty::Bool) {
                    self.checker.mismatch(file, self.checker.state.design.expr_node(lhs).span, &Ty::Bool, &other);
                }
                self.expressions().check(rhs, &Ty::Bool);
                Some(Ty::Bool)
            }
        }
    }

    /// `!` on an `Event`, `operand` the event: inside a guard or a reaction body the event reads
    /// as a `bool` and `!` is the ordinary one (§13 rules 2 and 9); anywhere else `E0440` (§13
    /// rule 1).
    pub(in crate::types) fn negated_event(&mut self, whole: ExprId, operand: ExprId) -> Option<Ty> {
        if self.reads_events() {
            self.event_read_as_bool(operand);
            self.state.result.exprs[operand.index()] = Some(Ty::Bool);
            return Some(Ty::Bool);
        }
        let file = self.file_of_expr(whole);
        let span = self.state.design.expr_node(whole).span;
        let message = "`!` applied to an `Event` outside a guard or a reaction body: `!` applies to a `bool` (§13 rule 1)".to_string();
        self.coded(file, "E0440", span, message, "`!` on an `Event`");
        None
    }

    /// `c.changed()`, `c.rise()` and `c.fall()` on a discrete carrier: an `Event`. `changed` on a
    /// node is `E0230`; `rise` and `fall` are the edges of a `Bit` of `bool` and of a `Bit<Logic>` of one level
    /// (`impl Bit<Logic>`, §22.1.5).
    pub(in crate::types) fn edge_read(&mut self, expr: ExprId, builtin: Builtin, recv: ExprId, args: &[Arg]) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let span = self.state.design.expr_node(expr).span;
        let name = match builtin {
            Builtin::Changed => "changed",
            Builtin::Rise => "rise",
            _ => "fall",
        };
        match self.declared_port(recv) {
            Some((_, Ty::Carrier { value, .. })) if builtin == Builtin::Changed || *value == Ty::Bool || self.expressions().logic_width(&value) == Some(1) => {
                self.expressions().apply(expr, &[], args)?;
                self.expressions().infer(recv);
                self.state.result.builtins.insert(expr, builtin);
                Some(Ty::Event)
            }
            Some((_, Ty::Carrier { value, .. })) => {
                self.checker.refuse_with(file, span, AnalogForm::MethodOnBitItIsEdgeOf, [name.to_string(), value.name(self.checker.state.design).to_string()]);
                None
            }
            Some((_, Ty::Node(_))) => {
                self.coded(file, "E0230", span, format!("`{name}()` applies to a discrete carrier: a node has no change (§5.3.2)"), "`changed()` on a node");
                None
            }
            _ => {
                self.refuse_with(file, span, AnalogForm::MethodOnThisReceiver, [name.to_string()]);
                None
            }
        }
    }

    /// The argument of `at`, once typed: a continuous one is `E0442` (§13 rule 11).
    pub(in crate::types) fn event_arguments(&mut self, name: &str, args: &[Arg]) {
        match name {
            "at" => {
                let Some(arg) = args.first() else { return };
                if self.analog().continuous(arg.value) {
                    let file = self.file_of_expr(arg.value);
                    let span = self.state.design.expr_node(arg.value).span;
                    let message = "the argument of `at` reads a probe, `now()`, `freq()`, `gmin()`, `source_scale()` or `analysis(Static)`: it must be a discrete expression (§13 rule 11)".to_string();
                    self.coded(file, "E0442", span, message, "a continuous `at`");
                }
            }
            _ => {}
        }
    }

    /// `cross` and `at` outside an event position: `E0443` (§13 rule 10).
    pub(in crate::types) fn core_event_out_of_place(&mut self, expr: ExprId, name: &str, args: &[Arg]) {
        let file = self.file_of_expr(expr);
        let span = self.state.design.expr_node(expr).span;
        let message = format!("`{name}` is a core module with an `Event` result: it is placed in an event position only (§13 rule 10)");
        self.coded(file, "E0443", span, message, "not in an event position");
        for arg in args {
            self.expressions().infer(arg.value);
        }
    }

    /// The `digital` block `behavior`: the head of each reaction is an `Event` (`E0430`), and a generation `for`/`if` around
    /// reactions is typed per iteration (GN-19); a reaction body is typed by `reaction.rs`.
    pub(in crate::types) fn digital_block(&mut self, behavior: DeclId) {
        let block = self.state.design.decl_data(behavior).clone();
        debug_assert!(matches!(&block.kind, DeclKind::Behavior(b) if b.kind == BehaviorKind::Digital));
        for child in block.children.iter().copied() {
            self.digital_item(child);
        }
    }

    /// One item of a `digital` block, or of a generation in it: a reaction, or a generation whose items are these.
    fn digital_item(&mut self, child: DeclId) {
        let item = self.state.design.decl_data(child).clone();
        match &item.kind {
            DeclKind::Reaction(r) => self.reaction(&r.clone(), item.file),
            DeclKind::Generation(_) => self.modules().generation(child, true, |this, inner| this.carriers().digital_item(inner)),
            _ => {}
        }
    }
}
