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
        match t {
            ValueType::Flagged(inner) => inner,
            other => other,
        }
    }

    /// Whether a value of this type may be unknown.
    pub(crate) fn flagged(t: &ValueType) -> bool {
        matches!(t, ValueType::Flagged(_))
    }

    /// `base`, flagged when `flagged` holds and the base can carry a flag (§3.3.2: one unknown
    /// operand makes the result unknown).
    pub(crate) fn flag(base: ValueType, flagged: bool) -> ValueType {
        if flagged && Typing::scalar(&base) { ValueType::Flagged(Box::new(base)) } else { base }
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

    /// `if cond { then } else { otherwise }` once a part is flagged: a `bool` condition, arms of
    /// one base type; the value is unknown when the condition or an arm may be (§3.3.3).
    pub(crate) fn if_(cond: &ValueType, then: &ValueType, otherwise: &ValueType) -> Option<ValueType> {
        let fits = *Typing::base(cond) == ValueType::Bool && Typing::base(then) == Typing::base(otherwise);
        let flagged = [cond, then, otherwise].iter().any(|t| Typing::flagged(t));
        fits.then(|| Typing::flag(Typing::base(then).clone(), flagged))
    }

    /// `value.known_or(default)`: a known `default` of the value's base type; the result is known.
    pub(crate) fn known_or(value: &ValueType, default: &ValueType) -> Option<ValueType> {
        let fits = Typing::scalar(default) && Typing::base(value) == default;
        fits.then(|| default.clone())
    }

    /// `value.is_known()` of any carried value, `value.high()` of a `bool`: a known `bool`.
    pub(crate) fn flag_read(high: bool, value: &ValueType) -> Option<ValueType> {
        let base = Typing::base(value);
        let fits = if high { *base == ValueType::Bool } else { Typing::scalar(base) };
        fits.then_some(ValueType::Bool)
    }

    pub(crate) fn method(method: Method, receiver: &ValueType, args: &[&ValueType]) -> Option<ValueType> {
        let base = Typing::base(receiver);
        let signed = matches!(base, ValueType::Int(_));
        let float = *base == ValueType::F64;
        let arity = match method {
            Method::Clamp => 2,
            Method::Min
            | Method::Max
            | Method::Atan2
            | Method::Powf
            | Method::Powi
            | Method::RemEuclid
            | Method::Hypot
            | Method::Pow
            | Method::SaturatingAdd
            | Method::SaturatingSub
            | Method::SaturatingMul
            | Method::AddOverflows
            | Method::SubOverflows
            | Method::MulOverflows => 1,
            _ => 0,
        };
        let same = args.iter().all(|a| Typing::base(a) == base);
        let fits = args.len() == arity
            && match method {
                Method::Min | Method::Max | Method::Clamp | Method::RemEuclid => Typing::numeric(base) && same,
                Method::Floor
                | Method::Sin
                | Method::Cos
                | Method::Tan
                | Method::Asin
                | Method::Acos
                | Method::Atan
                | Method::Sinh
                | Method::Cosh
                | Method::Tanh
                | Method::Log10
                | Method::Log2
                | Method::Ceil
                | Method::Round
                | Method::Trunc
                | Method::Fract
                | Method::IsNan
                | Method::IsFinite
                | Method::IsInfinite => float,
                Method::Atan2 | Method::Powf | Method::Hypot => float && same,
                Method::Powi => float && args.iter().all(|a| *Typing::base(a) == ValueType::Int(32)),
                Method::Signum => float || signed,
                Method::IntAbs => signed,
                Method::Pow => base.is_integer() && args.iter().all(|a| *Typing::base(a) == ValueType::UInt(32)),
                Method::SaturatingAdd | Method::SaturatingSub | Method::SaturatingMul | Method::AddOverflows | Method::SubOverflows | Method::MulOverflows => {
                    base.is_integer() && same
                }
            };
        let flagged = Typing::flagged(receiver) || args.iter().any(|a| Typing::flagged(a));
        let result = match method {
            Method::IsNan | Method::IsFinite | Method::IsInfinite | Method::AddOverflows | Method::SubOverflows | Method::MulOverflows => ValueType::Bool,
            _ => base.clone(),
        };
        fits.then(|| Typing::flag(result, flagged))
    }
}
