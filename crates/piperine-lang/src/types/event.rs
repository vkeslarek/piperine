//! Events (DIG-05, part IV §13): `Ty::Event`, the forms that make one (`changed`, `rise`,
//! `fall`, `cross`, `at`, `||`, `&&` with a guard), where one may stand (an event position: the
//! head of an `on`, a body `let`, the result of a module, the value of a function, an operand of
//! `||` or `&&` in one of these) and the heads of the reactions of a `digital` block.
//!
//! An `Event` has no `Repr`. Written where only a value may stand it is `E0441`; read as a `bool`
//! it is `P0001` (the elaborator has no read of it yet, part IV §13 rule 2).

use crate::pom::{Arg, BehaviorKind, DeclId, DeclKind, ExprId};

use super::check::{Checker, Context};
use super::{Builtin, Ty};

impl Checker<'_> {
    /// `infer` of an expression written in an event position: an `Event` is allowed there.
    pub(super) fn infer_event_position(&mut self, expr: ExprId) -> Option<Ty> {
        self.event_slot = Some(expr);
        self.infer(expr)
    }

    /// An `Event` typed where `expr` stands: `E0441` where only a value may stand, `P0001` where
    /// it would be read as a `bool` (a guard).
    pub(super) fn event_out_of_place(&mut self, expr: ExprId) {
        let node = self.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        if self.in_guard || (self.context == Context::Reaction && !self.committing) {
            self.unsupported(file, node.span, "an event read as a `bool`");
        } else {
            let message = "an `Event` value outside an event position, a guard or a reaction body: an event is never committed, carried or stored (§13 rule 8)".to_string();
            self.coded(file, "E0441", node.span, message, "an `Event` outside an event position");
        }
    }

    /// `a || b` and `a && b`. `a || b` is an `Event` when both operands are, and `ev && guard`
    /// is an `Event` when `ev` is and `guard` is a `bool`; such an `Event` is judged where it
    /// stands, like any other. Otherwise both operands are `bool`.
    pub(super) fn logical(&mut self, whole: ExprId, is_and: bool, lhs: ExprId, rhs: ExprId) -> Option<Ty> {
        let file = self.file_of_expr(whole);
        let span = self.design.expr_node(whole).span;
        let in_event_position = self.event_slot == Some(whole);
        let left = self.infer_event_position(lhs);
        match (is_and, left) {
            (true, Some(Ty::Event)) => {
                let outer = std::mem::replace(&mut self.in_guard, true);
                self.check(rhs, &Ty::Bool);
                self.in_guard = outer;
                Some(Ty::Event)
            }
            (false, Some(Ty::Event)) => {
                match self.infer_event_position(rhs) {
                    Some(Ty::Event) | None => {}
                    Some(other) => self.mismatch(file, self.design.expr_node(rhs).span, &Ty::Event, &other),
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
                    self.mismatch(file, self.design.expr_node(lhs).span, &Ty::Event, &Ty::Bool);
                    None
                }
                Some(Ty::Bool) => Some(Ty::Bool),
                Some(other) => {
                    self.mismatch(file, self.design.expr_node(rhs).span, &Ty::Bool, &other);
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
                    self.mismatch(file, self.design.expr_node(lhs).span, &Ty::Bool, &other);
                }
                self.check(rhs, &Ty::Bool);
                Some(Ty::Bool)
            }
        }
    }

    /// `!` on an `Event`: `E0440` (§13 rule 1); inside a guard it would read the event as a `bool`.
    pub(super) fn negated_event(&mut self, whole: ExprId) {
        let file = self.file_of_expr(whole);
        let span = self.design.expr_node(whole).span;
        if self.in_guard {
            self.unsupported(file, span, "an event read as a `bool`");
        } else {
            let message = "`!` applied to an `Event` outside a guard or a reaction body: `!` applies to a `bool` (§13 rule 1)".to_string();
            self.coded(file, "E0440", span, message, "`!` on an `Event`");
        }
    }

    /// `c.changed()`, `c.rise()` and `c.fall()` on a discrete carrier: an `Event`. `changed` on a
    /// node is `E0230`; `rise` and `fall` are the edges of a `Bit` of `bool`.
    pub(super) fn edge_read(&mut self, expr: ExprId, builtin: Builtin, recv: ExprId, args: &[Arg]) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let span = self.design.expr_node(expr).span;
        let name = match builtin {
            Builtin::Changed => "changed",
            Builtin::Rise => "rise",
            _ => "fall",
        };
        match self.declared_port(recv) {
            Some((_, Ty::Carrier { value, .. })) if builtin == Builtin::Changed || *value == Ty::Bool => {
                self.apply(expr, &[], args)?;
                self.infer(recv);
                self.builtins.insert(expr, builtin);
                Some(Ty::Event)
            }
            Some((_, Ty::Carrier { value, .. })) => {
                self.unsupported(file, span, &format!("method `{name}` on `Bit<{}>`: it is the edge of a `Bit` of `bool`", value.name(self.design)));
                None
            }
            Some((_, Ty::Node(_))) => {
                self.coded(file, "E0230", span, format!("`{name}()` applies to a discrete carrier: a node has no change (§5.3.2)"), "`changed()` on a node");
                None
            }
            _ => {
                self.unsupported(file, span, &format!("method `{name}` on this receiver"));
                None
            }
        }
    }

    /// The argument of `at`, once typed: a continuous one is `E0442` (§13 rule 11).
    pub(super) fn event_arguments(&mut self, name: &str, args: &[Arg]) {
        match name {
            "at" => {
                let Some(arg) = args.first() else { return };
                if self.continuous(arg.value) {
                    let file = self.file_of_expr(arg.value);
                    let span = self.design.expr_node(arg.value).span;
                    let message = "the argument of `at` reads a probe, `now()`, `freq()`, `gmin()`, `source_scale()` or `analysis(Static)`: it must be a discrete expression (§13 rule 11)".to_string();
                    self.coded(file, "E0442", span, message, "a continuous `at`");
                }
            }
            _ => {}
        }
    }

    /// `cross` and `at` outside an event position: `E0443` (§13 rule 10).
    pub(super) fn core_event_out_of_place(&mut self, expr: ExprId, name: &str, args: &[Arg]) {
        let file = self.file_of_expr(expr);
        let span = self.design.expr_node(expr).span;
        let message = format!("`{name}` is a core module with an `Event` result: it is placed in an event position only (§13 rule 10)");
        self.coded(file, "E0443", span, message, "not in an event position");
        for arg in args {
            self.infer(arg.value);
        }
    }

    /// The `digital` block `behavior`: the head of each reaction is an `Event` (`E0430`). A
    /// generation in it is outside the subset; a reaction body is typed by `reaction.rs`.
    pub(super) fn digital_block(&mut self, behavior: DeclId) {
        let block = self.design.decl_data(behavior).clone();
        debug_assert!(matches!(&block.kind, DeclKind::Behavior(b) if b.kind == BehaviorKind::Digital));
        for child in block.children.iter().copied() {
            let item = self.design.decl_data(child).clone();
            match &item.kind {
                DeclKind::Reaction(r) => self.reaction(&r.clone(), item.file),
                DeclKind::Generation(_) => self.unsupported(item.file, item.span, "generation `for`/`if` in a `digital` block"),
                _ => {}
            }
        }
    }
}
