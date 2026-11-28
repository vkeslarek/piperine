//! Comparison of composite constants: equality, order and the operators that read them (§3.3).

use std::cmp::Ordering;
use piperine_ir::{Constant};
use super::{CompositeKind, ConstEval, ConstFault};
use crate::refusal::forms::{ConstantForm};
use crate::pom::{BinOp, ExprId};

impl ConstEval<'_, '_> {
    /// Whether `a` and `b` are equal under the derived `==` (H.8 rule 2): the same variant and equal
    /// parts, field by field.
    pub(super) fn equal(&self, a: Constant, b: Constant) -> bool {
        match (self.composite_of(a), self.composite_of(b)) {
            (Some(x), Some(y)) => x.kind == y.kind && x.parts.len() == y.parts.len() && x.parts.iter().zip(&y.parts).all(|(p, q)| self.equal(*p, *q)),
            (None, None) => match (a, b) {
                (Constant::Real(p), Constant::Real(q)) => p.get() == q.get(),
                (p, q) => p == q,
            },
            _ => false,
        }
    }

    /// The order of `a` and `b` under the derived `PartialOrd` (H.8 rule 3, §1.5.23): fields in
    /// declaration order, a variant by its position then its fields, `None` below every `Some`.
    /// `None` is unordered (a NaN).
    pub(super) fn ordering(&self, a: Constant, b: Constant) -> Option<Ordering> {
        match (self.composite_of(a), self.composite_of(b)) {
            (Some(x), Some(y)) => {
                let by_kind = match (x.kind, y.kind) {
                    (CompositeKind::Variant(p), CompositeKind::Variant(q)) => p.cmp(&q),
                    (CompositeKind::Option, CompositeKind::Option) => x.parts.len().cmp(&y.parts.len()),
                    _ => Ordering::Equal,
                };
                if by_kind != Ordering::Equal {
                    return Some(by_kind);
                }
                for (p, q) in x.parts.iter().zip(&y.parts) {
                    match self.ordering(*p, *q) {
                        Some(Ordering::Equal) => {}
                        other => return other,
                    }
                }
                Some(x.parts.len().cmp(&y.parts.len()))
            }
            (None, None) => Self::order(a, b).or_else(|| match (a, b) {
                (Constant::Bool(p), Constant::Bool(q)) => Some(p.cmp(&q)),
                _ => None,
            }),
            _ => None,
        }
    }

    /// `a op b` over two operands one of which is a composite: `==`, `!=`, `<`, `<=`, `>`, `>=`.
    pub(super) fn composite_binary(&self, expr: ExprId, op: BinOp, a: Constant, b: Constant) -> Result<Constant, ConstFault> {
        let holds = |value: bool| Ok(Constant::Bool(value));
        match op {
            BinOp::Eq | BinOp::Ne => holds(self.equal(a, b) == (op == BinOp::Eq)),
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => match self.ordering(a, b) {
                None => holds(false),
                Some(order) => holds(match op {
                    BinOp::Lt => order.is_lt(),
                    BinOp::Le => order.is_le(),
                    BinOp::Gt => order.is_gt(),
                    _ => order.is_ge(),
                }),
            },
            _ => Err(ConstFault::refused(expr, ConstantForm::OperatorOtherThanComparisonOnCompositeValue)),
        }
    }
}
