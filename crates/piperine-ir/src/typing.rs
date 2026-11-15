//! The typing rules of the digital value forms (ir-rust phase B): the type an operator, a cast or
//! a method gives its operands' types, or `None` when the operands do not fit it. `Ir::check`
//! names a form whose recorded type differs from this one, and the structural printer writes a
//! type only where it does.
//!
//! The M1 forms keep their own rules in `check.rs` and `print.rs` while every operand is `f64`
//! or `bool`: these rules apply once an operand is an integer, or for the operators M1 does not
//! have.

use crate::model::{BinaryOp, Method, UnaryOp, ValueType};

pub(crate) struct Typing;

impl Typing {
    /// The type of a value with its unknown flag removed.
    pub(crate) fn base(t: &ValueType) -> &ValueType {
        t
    }

    /// Whether a value of this type may be unknown.
    pub(crate) fn flagged(_t: &ValueType) -> bool {
        false
    }

    /// `base`, flagged when `flagged` holds and the base can carry a flag.
    pub(crate) fn flag(base: ValueType, _flagged: bool) -> ValueType {
        base
    }

    /// A type outside the M1 pair `f64`, `bool`: an operand that selects these rules.
    pub(crate) fn digital(t: &ValueType) -> bool {
        Typing::base(t).is_integer() || Typing::flagged(t)
    }

    /// `f64` or an integer type.
    fn numeric(t: &ValueType) -> bool {
        *t == ValueType::F64 || t.is_integer()
    }

    /// A type a carrier can hold: `bool`, `f64` or an integer type.
    pub(crate) fn scalar(t: &ValueType) -> bool {
        matches!(t, ValueType::Bool | ValueType::F64) || t.is_integer()
    }

    /// The operators M1 does not have: they are typed by these rules whatever their operands.
    pub(crate) fn digital_op(op: BinaryOp) -> bool {
        matches!(op, BinaryOp::Rem | BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor | BinaryOp::Shl | BinaryOp::Shr)
    }

    pub(crate) fn binary(op: BinaryOp, l: &ValueType, r: &ValueType) -> Option<ValueType> {
        let flagged = Typing::flagged(l) || Typing::flagged(r);
        let (lb, rb) = (Typing::base(l), Typing::base(r));
        let same = lb == rb;
        let base = match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div if same && Typing::numeric(lb) => lb.clone(),
            BinaryOp::Rem if same && lb.is_integer() => lb.clone(),
            BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor if same && (lb.is_integer() || *lb == ValueType::Bool) => lb.clone(),
            BinaryOp::Shl | BinaryOp::Shr if lb.is_integer() && rb.is_integer() => lb.clone(),
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge if same && Typing::numeric(lb) => ValueType::Bool,
            BinaryOp::Eq | BinaryOp::Ne if same && Typing::scalar(lb) => ValueType::Bool,
            // `&&` and `||` decide control: their operands are plain `bool`.
            BinaryOp::And | BinaryOp::Or if *l == ValueType::Bool && *r == ValueType::Bool => ValueType::Bool,
            _ => return None,
        };
        Some(Typing::flag(base, flagged))
    }

    pub(crate) fn unary(op: UnaryOp, t: &ValueType) -> Option<ValueType> {
        let base = Typing::base(t);
        let fits = match op {
            UnaryOp::Neg => Typing::numeric(base),
            UnaryOp::Not => base.is_integer() || *base == ValueType::Bool,
        };
        fits.then(|| Typing::flag(base.clone(), Typing::flagged(t)))
    }

    /// `t as to`: among integer types and `f64`, or from `bool` to an integer type.
    pub(crate) fn cast(t: &ValueType, to: &ValueType) -> Option<ValueType> {
        let from = Typing::base(t);
        let fits = Typing::numeric(to) && (Typing::numeric(from) || (*from == ValueType::Bool && to.is_integer()));
        fits.then(|| Typing::flag(to.clone(), Typing::flagged(t)))
    }

    pub(crate) fn method(method: Method, receiver: &ValueType, args: &[&ValueType]) -> Option<ValueType> {
        let base = Typing::base(receiver);
        let arity = match method {
            Method::Min | Method::Max => 1,
            Method::Clamp => 2,
            Method::Floor => 0,
        };
        let fits = args.len() == arity
            && match method {
                Method::Floor => *base == ValueType::F64,
                Method::Min | Method::Max | Method::Clamp => Typing::numeric(base) && args.iter().all(|a| Typing::base(a) == base),
            };
        let flagged = Typing::flagged(receiver) || args.iter().any(|a| Typing::flagged(a));
        fits.then(|| Typing::flag(base.clone(), flagged))
    }
}
