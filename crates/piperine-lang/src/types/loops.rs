//! Loops and the exits that leave them (§1.8.7 to §1.8.12): `while c`, `while let p = e`, `loop`,
//! `for p in a..b`, `break`, `break value` and `continue`. `break` and `continue` outside a loop,
//! and `break value` outside a `loop`, are `E0185`.

use crate::pom::{BlockNode, ExprId, ExprKind, PatId};

use super::check::Checker;
use super::flow::{Assigned, LoopFrame};
use super::Ty;

impl Checker<'_> {
    /// `while cond { body }`: the type `()`. The condition is evaluated before every iteration, so
    /// what the body assigns is not assigned after the loop (§1.8.16).
    pub(super) fn while_loop(&mut self, cond: ExprId, body: &BlockNode) -> Option<Ty> {
        self.condition(cond);
        let after_condition = self.state.values.flow.assigned.clone();
        self.loop_body(LoopFrame::new(false, None), body);
        self.state.values.flow.assigned = after_condition;
        Some(Ty::Unit)
    }

    /// `loop { body }`: the type of its `break` values, `()` when they give none, and `!` when it
    /// has no `break` (§1.8.8, §1.5.16). `expected` is the type the position gives it.
    pub(super) fn loop_expr(&mut self, body: &BlockNode, expected: Option<&Ty>) -> Option<Ty> {
        let frame = self.loop_body(LoopFrame::new(true, expected.cloned()), body);
        self.state.values.flow.assigned = Assigned::join(frame.exits);
        match (frame.broke, frame.value) {
            (false, _) => Some(Ty::Never),
            (true, value) => Some(value.unwrap_or(Ty::Unit)),
        }
    }

    /// `for pat in lo..hi { body }` and `lo..=hi`: the type `()`. The range is over one integer
    /// type, its elements ascend (§1.8.9); an array gives its elements by value, an iterator of H.7
    /// its own.
    pub(super) fn for_loop(&mut self, iter: ExprId, pat: PatId, body: &BlockNode) -> Option<Ty> {
        let element = if matches!(self.state.design.expr_node(iter).kind, ExprKind::Range { lo: Some(_), hi: Some(_), .. }) { self.range_element(iter) } else { self.iteration_element(iter) };
        if let Some(element) = &element {
            self.for_binding(pat, element);
        }
        let after_iter = self.state.values.flow.assigned.clone();
        self.loop_body(LoopFrame::new(false, None), body);
        self.state.values.flow.assigned = after_iter;
        Some(Ty::Unit)
    }

    /// The block of a loop, checked inside `frame`: its type is `()` (§1.8.7). Gives the frame
    /// with what the body's `break`s recorded.
    fn loop_body(&mut self, frame: LoopFrame, body: &BlockNode) -> LoopFrame {
        self.state.values.flow.loops.push(frame);
        self.statements(&body.stmts, Some(&Ty::Unit));
        self.state.values.flow.loops.pop().unwrap_or_else(|| LoopFrame::new(false, None))
    }

    /// The element type of the range `iter` of a `for`: an integer type both bounds have.
    pub(super) fn range_element(&mut self, iter: ExprId) -> Option<Ty> {
        let node = self.state.design.expr_node(iter).clone();
        let file = self.file_of_expr(iter);
        let ExprKind::Range { lo: Some(lo), hi: Some(hi), .. } = &node.kind else { return None };
        let (first, second) = self.literal_last(*lo, *hi);
        match self.infer(first) {
            Some(t) if t.is_integer() => {
                self.check(second, &t);
                self.state.result.exprs[iter.index()] = Some(t.clone());
                Some(t)
            }
            Some(other) => {
                let span = self.state.design.expr_node(first).span;
                self.mismatch_text(file, span, "an integer type", &other.name(self.state.design));
                self.infer(second);
                None
            }
            None => {
                self.infer(second);
                None
            }
        }
    }

    /// The pattern of a `for`: irrefutable (§1.6.11), its names typed as the element.
    fn for_binding(&mut self, pat: PatId, element: &Ty) {
        let file = self.state.design.decl_data(self.state.design.pat_node(pat).owner).file;
        self.irrefutable_pattern(file, pat, element, "a `for`");
    }

    /// `break` or `break value` (§1.8.10): leaves the innermost loop; the value is allowed in a
    /// `loop` only and has the type of the loop's other `break`s. Its type is `!`.
    pub(super) fn break_expr(&mut self, expr: ExprId, value: Option<ExprId>) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let span = self.state.design.expr_node(expr).span;
        let Some(frame) = self.state.values.flow.loops.last().cloned() else {
            self.coded(file, "E0185", span, "`break` outside a loop (§1.8.10)".to_string(), "not in a loop");
            if let Some(value) = value {
                self.infer(value);
            }
            return Some(Ty::Never);
        };
        let found = match value {
            Some(value) if !frame.valued => {
                self.coded(file, "E0185", span, "`break` with a value outside a `loop` (§1.8.10)".to_string(), "a value in a `while` or `for`");
                self.infer(value);
                None
            }
            Some(value) => match &frame.value {
                Some(expected) => {
                    self.check(value, expected);
                    Some(expected.clone())
                }
                None => self.infer(value),
            },
            None => {
                if let Some(expected) = &frame.value {
                    if *expected != Ty::Unit {
                        self.mismatch(file, span, expected, &Ty::Unit);
                    }
                }
                Some(Ty::Unit)
            }
        };
        let state = self.state.values.flow.assigned.clone();
        if let Some(frame) = self.state.values.flow.loops.last_mut() {
            frame.broke = true;
            frame.value = frame.value.take().or(found);
            frame.exits.push(state);
        }
        self.state.values.flow.assigned.diverge();
        Some(Ty::Never)
    }

    /// `continue` (§1.8.10): starts the innermost loop's next iteration. Its type is `!`.
    pub(super) fn continue_expr(&mut self, expr: ExprId) -> Option<Ty> {
        if self.state.values.flow.loops.is_empty() {
            let file = self.file_of_expr(expr);
            let span = self.state.design.expr_node(expr).span;
            self.coded(file, "E0185", span, "`continue` outside a loop (§1.8.10)".to_string(), "not in a loop");
        }
        self.state.values.flow.assigned.diverge();
        Some(Ty::Never)
    }
}
