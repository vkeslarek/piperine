//! `return` and `?` (§1.5.15, §1.8.11): the exits of the innermost function or closure.
//! `return` and `?` outside one are `E0185`, and so is `?` in one whose result is not an `Option`.

use crate::pom::ExprId;

use super::check::Checker;
use super::Ty;

impl Checker<'_> {
    /// `return` or `return value` (§1.8.11): the value has the innermost function's or closure's
    /// result type, `return` alone gives `()`. Its type is `!` (§1.5.16).
    pub(super) fn return_expr(&mut self, expr: ExprId, value: Option<ExprId>) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let span = self.state.design.expr_node(expr).span;
        match self.state.values.flow.exit.clone() {
            None => {
                self.coded(file, "E0185", span, "`return` outside a function or a closure (§1.8.11)".to_string(), "not in a function");
                if let Some(value) = value {
                    self.infer(value);
                }
            }
            Some(frame) => match (value, frame.result) {
                (Some(value), Some(result)) => self.check(value, &result),
                (Some(value), None) => {
                    // a closure whose result is not given: its first `return` gives it
                    let found = self.infer(value);
                    if let (Some(open), true) = (self.state.values.flow.exit.as_mut(), frame.inferred) {
                        open.returned = open.returned.take().or(found);
                    }
                }
                (None, Some(result)) if result != Ty::Unit => self.mismatch(file, span, &result, &Ty::Unit),
                (None, _) => {}
            },
        }
        self.state.values.flow.assigned.diverge();
        Some(Ty::Never)
    }

    /// `operand?` (§1.5.15): the `T` of an `Option<T>`, in a function or closure whose result is an
    /// `Option`.
    pub(super) fn try_expr(&mut self, expr: ExprId, operand: ExprId) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let span = self.state.design.expr_node(expr).span;
        match self.state.values.flow.exit.as_ref().map(|frame| frame.result.clone()) {
            None => self.coded(file, "E0185", span, "`?` outside a function or a closure (§1.5.15)".to_string(), "not in a function"),
            Some(Some(result)) if !matches!(result, Ty::Option(_)) => {
                let message = format!("`?` in a function whose result is `{}`, not an `Option` (§1.5.15)", result.name(self.state.design));
                self.coded(file, "E0185", span, message, "the result is not an `Option`");
            }
            Some(_) => {}
        }
        match self.infer(operand)? {
            Ty::Option(inner) => Some(*inner),
            other => {
                let operand_span = self.state.design.expr_node(operand).span;
                self.mismatch_text(file, operand_span, "an `Option`", &other.name(self.state.design));
                None
            }
        }
    }
}
