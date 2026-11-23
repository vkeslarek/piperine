//! Derived comparisons of `Option`s and `T::default()` (H.8 rules 2 to 4, §1.5.23; feature
//! `composite-values`, CV-12).
//!
//! A struct, tuple, array or enum is one IR value, so its `==` and `<` are the IR's `Binary` on
//! two aggregates (the printer derives the order). An `Option` is not one value in value code, it
//! is the pair `(is, value)`, so a comparison of two of them is built from the halves: equal when
//! the flags agree and, for a `Some`, the payloads are equal; ordered with `None` lowest (§1.5.23)
//! and `Some` by payload. `T::default()` is `Expr::Default(T)`, the default of an `Option` its
//! `None`.

use piperine_ir::{BinaryOp, Expr, ExprId as LoweredExpr, Local, Origin, Stmt, StmtId as LoweredStmt, UnaryOp, ValueType};

use super::compound::Pair;
use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::pom::{BinOp, ExprId};

/// Which operand of a comparison a read names.
#[derive(Clone, Copy)]
enum Side {
    /// The left operand.
    Left,
    /// The right operand.
    Right,
}

impl Side {
    /// The other operand.
    fn other(self) -> Side {
        match self {
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        }
    }
}

/// The four values an `Option` comparison reads, each held in a local so it is evaluated once.
struct OptionOperands {
    /// The flag and the payload locals of the left operand.
    left: (u32, u32),
    /// The flag and the payload locals of the right operand.
    right: (u32, u32),
    /// The type of both payloads.
    payload: ValueType,
    /// Where the comparison is written.
    origin: Origin,
}

impl Elaboration<'_, '_> {
    /// `lhs op rhs` over two `Option`s with a comparison operator. `None` after a diagnostic.
    pub(super) fn lower_option_comparison(&mut self, scope: &mut Scope, op: BinOp, lhs: ExprId, rhs: ExprId, origin: Origin) -> Option<LoweredExpr> {
        let (left, right) = (self.lower_option(scope, lhs)?, self.lower_option(scope, rhs)?);
        let span = self.design.expr_node(lhs).span;
        let mut stmts = Vec::new();
        let operands = self.hold_operands(scope, &mut stmts, left, right, origin.clone());
        let result = match op {
            BinOp::Eq => self.options_equal(&operands),
            BinOp::Ne => {
                let equal = self.options_equal(&operands);
                self.negated(equal, &origin)
            }
            BinOp::Lt => self.option_below(&operands, Side::Left),
            BinOp::Gt => self.option_below(&operands, Side::Right),
            BinOp::Le => self.option_not_above(&operands, Side::Left),
            BinOp::Ge => self.option_not_above(&operands, Side::Right),
            _ => {
                self.unsupported(scope.file, span, "an operator other than a comparison on `Option`s");
                return None;
            }
        };
        Some(self.emit(Expr::Block { stmts, value: Some(result) }, ValueType::Bool, origin))
    }

    /// Binds the halves of both operands to locals, the binding statements added to `stmts`.
    fn hold_operands(&mut self, scope: &mut Scope, stmts: &mut Vec<LoweredStmt>, left: Pair, right: Pair, origin: Origin) -> OptionOperands {
        let payload = self.type_of(left.val);
        let mut hold = |this: &mut Self, value: LoweredExpr| {
            let local = scope.locals.len() as u32;
            scope.locals.push(Local { name: format!("option$cmp{local}"), ty: this.type_of(value), mutable: false });
            stmts.push(this.stmt(Stmt::Let { local, init: value }));
            local
        };
        let left = (hold(self, left.is), hold(self, left.val));
        let right = (hold(self, right.is), hold(self, right.val));
        OptionOperands { left, right, payload, origin }
    }

    /// A fresh read of the flag of the operand on `side`.
    fn flag_of(&mut self, operands: &OptionOperands, side: Side) -> LoweredExpr {
        let local = match side {
            Side::Left => operands.left.0,
            Side::Right => operands.right.0,
        };
        self.emit(Expr::Local(local), ValueType::Bool, operands.origin.clone())
    }

    /// A fresh read of the payload of the operand on `side`.
    fn payload_of(&mut self, operands: &OptionOperands, side: Side) -> LoweredExpr {
        let local = match side {
            Side::Left => operands.left.1,
            Side::Right => operands.right.1,
        };
        self.emit(Expr::Local(local), operands.payload.clone(), operands.origin.clone())
    }

    /// `lhs op rhs` of the known operands, a `bool`.
    fn joined(&mut self, op: BinaryOp, lhs: LoweredExpr, rhs: LoweredExpr, origin: &Origin) -> LoweredExpr {
        self.emit(Expr::Binary { op, lhs, rhs }, ValueType::Bool, origin.clone())
    }

    /// `!a` of a known `bool`.
    fn negated(&mut self, a: LoweredExpr, origin: &Origin) -> LoweredExpr {
        self.emit(Expr::Unary { op: UnaryOp::Not, expr: a }, ValueType::Bool, origin.clone())
    }

    /// `a == b` of two `Option`s: `a.is == b.is && (!a.is || a.value == b.value)` (H.8 rule 2).
    fn options_equal(&mut self, operands: &OptionOperands) -> LoweredExpr {
        let origin = &operands.origin;
        let (left_is, right_is) = (self.flag_of(operands, Side::Left), self.flag_of(operands, Side::Right));
        let same_flag = self.joined(BinaryOp::Eq, left_is, right_is, origin);
        let (left_value, right_value) = (self.payload_of(operands, Side::Left), self.payload_of(operands, Side::Right));
        let same_value = self.joined(BinaryOp::Eq, left_value, right_value, origin);
        let present = self.flag_of(operands, Side::Left);
        let absent = self.negated(present, origin);
        let payload_agrees = self.joined(BinaryOp::Or, absent, same_value, origin);
        self.joined(BinaryOp::And, same_flag, payload_agrees, origin)
    }

    /// `a < b` for `low` the left operand, `b < a` for the right: `None` is below every `Some`, two
    /// `Some`s order by payload (H.8 rule 3, §1.5.23).
    fn option_below(&mut self, operands: &OptionOperands, low: Side) -> LoweredExpr {
        let origin = &operands.origin;
        let high = low.other();
        let low_flag = self.flag_of(operands, low);
        let low_absent = self.negated(low_flag, origin);
        let high_present = self.flag_of(operands, high);
        let none_below_some = self.joined(BinaryOp::And, low_absent, high_present, origin);
        let (low_present, high_present) = (self.flag_of(operands, low), self.flag_of(operands, high));
        let both_present = self.joined(BinaryOp::And, low_present, high_present, origin);
        let (low_value, high_value) = (self.payload_of(operands, low), self.payload_of(operands, high));
        let ordered = self.joined(BinaryOp::Lt, low_value, high_value, origin);
        let by_payload = self.joined(BinaryOp::And, both_present, ordered, origin);
        self.joined(BinaryOp::Or, none_below_some, by_payload, origin)
    }

    /// `a <= b` for `low` the left operand, `b <= a` for the right: below, or equal.
    fn option_not_above(&mut self, operands: &OptionOperands, low: Side) -> LoweredExpr {
        let below = self.option_below(operands, low);
        let equal = self.options_equal(operands);
        self.joined(BinaryOp::Or, below, equal, &operands.origin)
    }
}
