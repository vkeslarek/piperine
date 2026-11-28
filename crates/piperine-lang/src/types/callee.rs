//! What a call or a placement of the reachable set refers to: the row of a method, a built-in, a callee.

use crate::pom::{BuiltinId, DeclId};

use super::Intrinsic;

/// What a `Deferred` method call of the reachable set resolves to: a row of the Appendix H
/// catalogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Exp,
    Ln,
    Sqrt,
    Abs,
    /// `a.min(b)` on an integer or `f64`.
    Min,
    /// `a.max(b)` on an integer or `f64`.
    Max,
    /// `x.clamp(lo, hi)` on an integer or `f64`.
    Clamp,
    /// `x.floor()` on `f64`.
    Floor,
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Atan2,
    Sinh,
    Cosh,
    Tanh,
    Powf,
    /// `x.powi(e: i32)` on `f64`.
    Powi,
    Log10,
    Log2,
    Ceil,
    Round,
    Trunc,
    Fract,
    /// `x.signum()` on `f64` or a signed integer.
    Signum,
    /// `x.rem_euclid(d)` on `f64` or an integer.
    RemEuclid,
    Hypot,
    IsNan,
    IsFinite,
    IsInfinite,
    /// `x.abs()` on a signed integer.
    IntAbs,
    /// `x.pow(e: u32)` on an integer.
    Pow,
    SaturatingAdd,
    SaturatingSub,
    SaturatingMul,
    /// `a.checked_add(b)`: an `Option` of the wrapping sum.
    CheckedAdd,
    CheckedSub,
    CheckedMul,
    /// `o.is_some()` on an `Option`.
    IsSome,
    /// `o.is_none()` on an `Option`.
    IsNone,
    /// `o.unwrap_or(d)` on an `Option`.
    UnwrapOr,
    /// `o.unwrap()`: a fault on `None`.
    Unwrap,
    /// `o.expect("msg")`: a fault with `msg` on `None`.
    Expect,
    /// `o.unwrap_or_else(|| d)`.
    UnwrapOrElse,
    /// `o.map(|x| e)`.
    Map,
    /// `o.and_then(|x| p)`.
    AndThen,
    /// `a.or(b)` on `Option`s.
    Or,
    /// `c.then(|| v)` on a `bool`.
    Then,
    /// `c.then_some(v)` on a `bool`.
    ThenSome,
    /// `a.len()` on an array (H.5).
    ArrayLen,
    /// `a.is_empty()` on an array.
    ArrayIsEmpty,
    /// `a.iter()`: the elements of an array as an iterator.
    ArrayIter,
    /// `a.contains(x)` on an array of a `PartialEq` type.
    ArrayContains,
    /// `a.first()` on an array: `Some` of element 0.
    ArrayFirst,
    /// `a.last()` on an array: `Some` of the last element.
    ArrayLast,
    /// `a.get(i)` on an array: `None` past the end.
    ArrayGet,
    /// `r.contains(x)` on a range (H.6).
    RangeContains,
    /// `r.is_empty()` on a range.
    RangeIsEmpty,
    /// `it.map(f)` on an iterator (H.7).
    IteratorMap,
    /// `it.filter(f)`.
    IteratorFilter,
    /// `it.expressions().fold(init, f)`.
    IteratorFold,
    /// `it.sum()` of an integer or float element type.
    IteratorSum,
    /// `it.count()`.
    IteratorCount,
    /// `it.expressions().zip(other)`.
    IteratorZip,
    /// `it.enumerate()`.
    IteratorEnumerate,
    /// `it.rev()` on a reversible iterator.
    IteratorRev,
    /// `it.any(f)`.
    IteratorAny,
    /// `it.all(f)`.
    IteratorAll,
}

impl Method {
    /// Whether the row is one of H.5 to H.7 the elaborator lowers as a loop over a sequence and
    /// reads as a value (`first`, `last` and `get` are `Option`s, lowered on their pair).
    pub fn is_sequence_row(self) -> bool {
        matches!(
            self,
            Method::ArrayLen | Method::ArrayIsEmpty | Method::ArrayContains | Method::RangeContains | Method::RangeIsEmpty | Method::IteratorSum | Method::IteratorCount | Method::IteratorFold | Method::IteratorAny | Method::IteratorAll
        )
    }
}

/// A method the checker treats as part of the language because it resolved to the stdlib's own
/// item (part I §3.3.7): the edges of `Bit` and the history of a
/// carrier. A project's replacement is `P0001` at its use, never one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Builtin {
    /// `c.changed() -> Event`.
    Changed,
    /// `c.prev() -> T`: the value before the most recent change.
    Prev,
    /// `c.rise() -> Event` on a `Bit`.
    Rise,
    /// `c.fall() -> Event` on a `Bit`.
    Fall,
    /// `p.inflow() -> f64`: the flow entering the module at its port `p` (§6 rule 5).
    Inflow,
    /// `c.others() -> V` on a resolved carrier: the resolution of every drive but the module's own (§5.2.7).
    Others,
}

/// What a call or a placement of the reachable set refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Callee {
    Function(DeclId),
    Core(BuiltinId),
    Module(DeclId),
    /// A method of the project called on a value, by the function it resolves to (§3.7.8).
    Method(DeclId),
    /// A method a type parameter's bound gives, by the trait's function: the instance a call reaches is
    /// the implementation for the concrete type, found where the function is instantiated.
    Bound {
        /// The trait's function.
        item: DeclId,
    },
    /// A function of §22.1 typed by its contract at the concrete type (`merge`, `same`, `cast`,
    /// `logic`, `matches_bits`), until `generics` lets it compile from its stdlib source.
    Intrinsic(Intrinsic),
}
