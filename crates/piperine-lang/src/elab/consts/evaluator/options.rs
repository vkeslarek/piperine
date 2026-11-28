//! `Option`s in a const expression (part I §3.2.10): `Some` and `None` as constants, and the rows of Appendix H that read them.

use piperine_ir::{Constant};
use super::{CompositeKind, Bindings, ConstEval, ConstFault};
use crate::refusal::forms::{ConstantForm};
use crate::pom::{ExprId, ExprKind};
use crate::types::{Method, Ty};

impl ConstEval<'_, '_> {
    /// `Some(x)` or `None` named as a value; `None` for any other call or name.
    pub(super) fn option_value(&mut self, expr: ExprId, bindings: &Bindings) -> Option<Result<Constant, ConstFault>> {
        match &self.design.expr_node(expr).kind.clone() {
            ExprKind::Path(path) if self.builtin_name(*path) == Some("None") => Some(Ok(self.make(CompositeKind::Option, Vec::new()))),
            ExprKind::Call { callee, args, .. } if self.builtin_callee(*callee) == Some("Some") && args.len() == 1 => {
                let inner = self.fold(args[0].value, bindings);
                Some(inner.map(|inner| self.make(CompositeKind::Option, vec![inner])))
            }
            _ => None,
        }
    }

    /// A method of `Option` that is not one of the sequence rows: `is_some`, `is_none`, `unwrap`,
    /// `expect`, `unwrap_or`, `unwrap_or_else`, `map`, `and_then` and `or` over a composite
    /// `Option`; `None` for any other method.
    pub(super) fn option_row(&mut self, expr: ExprId, method: Method, recv: ExprId, args: &[ExprId], bindings: &Bindings) -> Option<Result<Constant, ConstFault>> {
        if !matches!(method, Method::IsSome | Method::IsNone | Method::Unwrap | Method::Expect | Method::UnwrapOr | Method::UnwrapOrElse | Method::Map | Method::AndThen | Method::Or) || !matches!(self.typed(recv), Some(Ty::Option(_))) {
            return None;
        }
        Some(self.run_option_row(expr, method, recv, args, bindings))
    }

    /// One row of `Option`.
    pub(super) fn run_option_row(&mut self, expr: ExprId, method: Method, recv: ExprId, args: &[ExprId], bindings: &Bindings) -> Result<Constant, ConstFault> {
        let receiver = self.fold(recv, bindings)?;
        let (_, parts) = self.parts_of(expr, receiver, "an `Option` row over a value")?;
        let held = parts.first().copied();
        let known = |value: bool| Ok(Constant::Bool(value));
        match (method, held) {
            (Method::IsSome, held) => known(held.is_some()),
            (Method::IsNone, held) => known(held.is_none()),
            (Method::Unwrap | Method::Expect, Some(value)) => Ok(value),
            (Method::Unwrap | Method::Expect, None) => Err(ConstFault::Panic { expr, what: "`unwrap` of a `None` (H.4 rule 2)".to_string() }),
            (Method::UnwrapOr, Some(value)) => {
                self.fold(args[0], bindings)?;
                Ok(value)
            }
            (Method::UnwrapOr, None) => self.fold(args[0], bindings),
            (Method::UnwrapOrElse, Some(value)) => Ok(value),
            (Method::UnwrapOrElse, None) => {
                let function = self.closure_argument(expr, args[0])?;
                self.call_with(expr, &function, Vec::new(), bindings)
            }
            (Method::Map, Some(value)) => {
                let function = self.closure_argument(expr, args[0])?;
                let mapped = self.call_with(expr, &function, vec![value], bindings)?;
                Ok(self.make(CompositeKind::Option, vec![mapped]))
            }
            (Method::AndThen, Some(value)) => {
                let function = self.closure_argument(expr, args[0])?;
                self.call_with(expr, &function, vec![value], bindings)
            }
            (Method::Map | Method::AndThen, None) => Ok(self.make(CompositeKind::Option, Vec::new())),
            (Method::Or, Some(_)) => Ok(receiver),
            (Method::Or, None) => self.fold(args[0], bindings),
            _ => Err(ConstFault::refused(expr, ConstantForm::OptionRowInConstExpression)),
        }
    }
}
