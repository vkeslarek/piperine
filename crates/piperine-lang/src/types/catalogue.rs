//! The closed method catalogue: the rows of Appendix H, one line each. A method outside a
//! complete receiver type's rows is `E0161` (rule 1); a receiver type without a catalogue is `P0001`.

use crate::pom::BuiltinId;

use super::{Method, Ty};

/// The type of one argument of a row of Appendix H.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Arg {
    /// The receiver's own type.
    SelfTy,
    /// `i32`: the exponent of `powi`.
    I32,
    /// `u32`: the exponent of `pow`.
    U32,
}

impl Arg {
    fn ty(self, recv: &Ty) -> Ty {
        match self {
            Arg::SelfTy => recv.clone(),
            Arg::I32 => Ty::Int(32),
            Arg::U32 => Ty::UInt(32),
        }
    }
}

/// The result type of a row of Appendix H.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Res {
    SelfTy,
    Bool,
    /// `Option<Self>`: the `checked_*` rows.
    OptionSelf,
}

impl Res {
    fn ty(self, recv: &Ty) -> Ty {
        match self {
            Res::SelfTy => recv.clone(),
            Res::Bool => Ty::Bool,
            Res::OptionSelf => Ty::Option(Box::new(recv.clone())),
        }
    }
}

/// One row of Appendix H: the method's name, what it is recorded as, its arguments and its result.
struct Row {
    name: &'static str,
    method: Method,
    params: &'static [Arg],
    result: Res,
    /// A row of the signed integer types only (`abs`, `signum`).
    signed_only: bool,
}

const fn row(name: &'static str, method: Method, params: &'static [Arg], result: Res) -> Row {
    Row { name, method, params, result, signed_only: false }
}

const fn signed(name: &'static str, method: Method, params: &'static [Arg], result: Res) -> Row {
    Row { name, method, params, result, signed_only: true }
}

const SELF: Arg = Arg::SelfTy;

/// The rows of Appendix H.2, `f64`: 32 of them.
const F64_METHODS: &[Row] = &[
    row("abs", Method::Abs, &[], Res::SelfTy),
    row("sqrt", Method::Sqrt, &[], Res::SelfTy),
    row("powf", Method::Powf, &[SELF], Res::SelfTy),
    row("powi", Method::Powi, &[Arg::I32], Res::SelfTy),
    row("exp", Method::Exp, &[], Res::SelfTy),
    row("ln", Method::Ln, &[], Res::SelfTy),
    row("log10", Method::Log10, &[], Res::SelfTy),
    row("log2", Method::Log2, &[], Res::SelfTy),
    row("sin", Method::Sin, &[], Res::SelfTy),
    row("cos", Method::Cos, &[], Res::SelfTy),
    row("tan", Method::Tan, &[], Res::SelfTy),
    row("asin", Method::Asin, &[], Res::SelfTy),
    row("acos", Method::Acos, &[], Res::SelfTy),
    row("atan", Method::Atan, &[], Res::SelfTy),
    row("atan2", Method::Atan2, &[SELF], Res::SelfTy),
    row("sinh", Method::Sinh, &[], Res::SelfTy),
    row("cosh", Method::Cosh, &[], Res::SelfTy),
    row("tanh", Method::Tanh, &[], Res::SelfTy),
    row("floor", Method::Floor, &[], Res::SelfTy),
    row("ceil", Method::Ceil, &[], Res::SelfTy),
    row("round", Method::Round, &[], Res::SelfTy),
    row("trunc", Method::Trunc, &[], Res::SelfTy),
    row("fract", Method::Fract, &[], Res::SelfTy),
    row("min", Method::Min, &[SELF], Res::SelfTy),
    row("max", Method::Max, &[SELF], Res::SelfTy),
    row("clamp", Method::Clamp, &[SELF, SELF], Res::SelfTy),
    row("signum", Method::Signum, &[], Res::SelfTy),
    row("rem_euclid", Method::RemEuclid, &[SELF], Res::SelfTy),
    row("hypot", Method::Hypot, &[SELF], Res::SelfTy),
    row("is_nan", Method::IsNan, &[], Res::Bool),
    row("is_finite", Method::IsFinite, &[], Res::Bool),
    row("is_infinite", Method::IsInfinite, &[], Res::Bool),
];

/// The rows of Appendix H.1, `UInt<N>` and `Int<N>`: 13 of them.
const INTEGER_METHODS: &[Row] = &[
    signed("abs", Method::IntAbs, &[], Res::SelfTy),
    row("min", Method::Min, &[SELF], Res::SelfTy),
    row("max", Method::Max, &[SELF], Res::SelfTy),
    row("clamp", Method::Clamp, &[SELF, SELF], Res::SelfTy),
    row("pow", Method::Pow, &[Arg::U32], Res::SelfTy),
    row("rem_euclid", Method::RemEuclid, &[SELF], Res::SelfTy),
    signed("signum", Method::Signum, &[], Res::SelfTy),
    row("checked_add", Method::CheckedAdd, &[SELF], Res::OptionSelf),
    row("checked_sub", Method::CheckedSub, &[SELF], Res::OptionSelf),
    row("checked_mul", Method::CheckedMul, &[SELF], Res::OptionSelf),
    row("saturating_add", Method::SaturatingAdd, &[SELF], Res::SelfTy),
    row("saturating_sub", Method::SaturatingSub, &[SELF], Res::SelfTy),
    row("saturating_mul", Method::SaturatingMul, &[SELF], Res::SelfTy),
];

/// What looking a method up on a receiver type found.
pub(super) enum Lookup {
    /// A row: its method, its argument types and its result.
    Row(Method, Vec<Ty>, Ty),
    /// A row of H.3 or H.4 whose arguments are typed from the receiver's payload and a closure's
    /// body (`Checker::payload_row`).
    Payload(Method),
    /// The receiver's catalogue is complete and has no such row: `E0161` (Appendix H rule 1).
    Absent,
    /// The receiver's type has no catalogue in the subset yet: `P0001`.
    Uncatalogued,
}

pub(super) struct Catalogue;

impl Catalogue {
    /// The row named `name` for a receiver of type `recv`.
    pub(super) fn method(recv: &Ty, name: &str) -> Lookup {
        let rows = match recv {
            Ty::F64 | Ty::F32 | Ty::F16 | Ty::F128 => F64_METHODS,
            Ty::UInt(_) | Ty::Int(_) | Ty::GenericInteger { .. } => INTEGER_METHODS,
            Ty::Option(inner) => return Self::option(inner, name),
            Ty::Array(element, _) => return Self::array(element, name),
            Ty::Range { element, iterable } => return Self::range(element, *iterable, name),
            Ty::Iterator { element, reversible } => return Self::iterator(element, *reversible, name),
            Ty::Bool => {
                return match name {
                    "then" => Lookup::Payload(Method::Then),
                    "then_some" => Lookup::Payload(Method::ThenSome),
                    _ => Lookup::Absent,
                };
            }
            Ty::Unit | Ty::Node(_) | Ty::AnyNode | Ty::Fn(..) | Ty::Module(_) | Ty::Enum(_) | Ty::Tuple(_) | Ty::Struct(_) | Ty::Carrier { .. } | Ty::Event | Ty::Str | Ty::Never | Ty::Param(_) => return Lookup::Uncatalogued,
        };
        match rows.iter().find(|r| r.name == name && (!r.signed_only || matches!(recv, Ty::Int(_) | Ty::GenericInteger { signed: true, .. }))) {
            Some(r) => Lookup::Row(r.method, r.params.iter().map(|a| a.ty(recv)).collect(), r.result.ty(recv)),
            None => Lookup::Absent,
        }
    }

    /// The rows of `[T; N]` (Appendix H.5): every receiver, argument and result is a value.
    fn array(element: &Ty, name: &str) -> Lookup {
        let usize_ty = Ty::UInt(64);
        let option = Ty::Option(Box::new(element.clone()));
        match name {
            "len" => Lookup::Row(Method::ArrayLen, vec![], usize_ty),
            "is_empty" => Lookup::Row(Method::ArrayIsEmpty, vec![], Ty::Bool),
            "iter" => Lookup::Row(Method::ArrayIter, vec![], Ty::Iterator { element: Box::new(element.clone()), reversible: true }),
            "contains" => Lookup::Payload(Method::ArrayContains),
            "first" => Lookup::Row(Method::ArrayFirst, vec![], option),
            "last" => Lookup::Row(Method::ArrayLast, vec![], option),
            "get" => Lookup::Row(Method::ArrayGet, vec![usize_ty], option),
            _ => Lookup::Absent,
        }
    }

    /// The rows of a range (Appendix H.6); an iterable range is also an iterator (H.7).
    fn range(element: &Ty, iterable: bool, name: &str) -> Lookup {
        match name {
            "contains" => Lookup::Row(Method::RangeContains, vec![element.clone()], Ty::Bool),
            "is_empty" => Lookup::Row(Method::RangeIsEmpty, vec![], Ty::Bool),
            _ if iterable => Self::iterator(element, true, name),
            _ => Lookup::Absent,
        }
    }

    /// The rows of an iterator of `element` (Appendix H.7); `rev` only when `reversible` (rule 3).
    fn iterator(element: &Ty, reversible: bool, name: &str) -> Lookup {
        let iterator = |item: Ty, reversible: bool| Ty::Iterator { element: Box::new(item), reversible };
        match name {
            "map" => Lookup::Payload(Method::IteratorMap),
            "filter" => Lookup::Payload(Method::IteratorFilter),
            "fold" => Lookup::Payload(Method::IteratorFold),
            "sum" => Lookup::Payload(Method::IteratorSum),
            "zip" => Lookup::Payload(Method::IteratorZip),
            "any" => Lookup::Payload(Method::IteratorAny),
            "all" => Lookup::Payload(Method::IteratorAll),
            "count" => Lookup::Row(Method::IteratorCount, vec![], Ty::UInt(64)),
            "enumerate" => Lookup::Row(Method::IteratorEnumerate, vec![], iterator(Ty::Tuple(vec![Ty::UInt(64), element.clone()]), reversible)),
            "rev" if reversible => Lookup::Row(Method::IteratorRev, vec![], iterator(element.clone(), true)),
            _ => Lookup::Absent,
        }
    }

    /// The rows of `Option<T>` (Appendix H.4).
    fn option(inner: &Ty, name: &str) -> Lookup {
        match name {
            "is_some" => Lookup::Row(Method::IsSome, vec![], Ty::Bool),
            "is_none" => Lookup::Row(Method::IsNone, vec![], Ty::Bool),
            "unwrap" => Lookup::Row(Method::Unwrap, vec![], inner.clone()),
            "unwrap_or" => Lookup::Row(Method::UnwrapOr, vec![inner.clone()], inner.clone()),
            "or" => Lookup::Row(Method::Or, vec![Ty::Option(Box::new(inner.clone()))], Ty::Option(Box::new(inner.clone()))),
            "expect" => Lookup::Payload(Method::Expect),
            "unwrap_or_else" => Lookup::Payload(Method::UnwrapOrElse),
            "map" => Lookup::Payload(Method::Map),
            "and_then" => Lookup::Payload(Method::AndThen),
            _ => Lookup::Absent,
        }
    }
}

/// One parameter of a core primitive of the subset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CoreParam {
    F64,
    /// `fn(f64, f64) -> f64`: the input function of `limit`.
    Limiter,
    /// `Option<f64>`: a tolerance of `cross`.
    OptionF64,
    /// A string literal or a `str` the elaborator reads as text: `simparam`'s name.
    Text,
    /// `Option<str>`: the name of a `noise` source, a literal or a `str` param passed on.
    OptionText,
    /// A potential or flow probe of a branch of this module: the variable of `ddx` (§14.2.2).
    /// Judged by name in `core_call` (`E0451`), typed as the `f64` the probe reads.
    Probe,
}

impl CoreParam {
    pub(super) fn ty(self) -> Ty {
        match self {
            CoreParam::F64 | CoreParam::Probe => Ty::F64,
            CoreParam::Text => Ty::Str,
            CoreParam::OptionText => Ty::Option(Box::new(Ty::Str)),
            CoreParam::Limiter => Ty::Fn(vec![Ty::F64, Ty::F64], Box::new(Ty::F64)),
            CoreParam::OptionF64 => Ty::Option(Box::new(Ty::F64)),
        }
    }
}

/// The type a call of a core primitive has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CoreResult {
    F64,
    Bool,
    Event,
    /// `simparam`: an `Option<f64>`.
    OptionF64,
    /// `bound_step`: a statement, no value.
    Unit,
}

impl CoreResult {
    pub(super) fn ty(self) -> Ty {
        match self {
            CoreResult::F64 => Ty::F64,
            CoreResult::Bool => Ty::Bool,
            CoreResult::Event => Ty::Event,
            CoreResult::OptionF64 => Ty::Option(Box::new(Ty::F64)),
            CoreResult::Unit => Ty::Unit,
        }
    }
}

/// Where a core primitive may be written (§14 table, column "Allowed in").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Allowed {
    /// An `analog` block only (`ac_stim`).
    Analog,
    /// Where instances go and an `analog` block reads them: a module body or an `analog` block,
    /// never a `fn`, a closure, a param default or a `where` (`ddt`, `limit`).
    Instances,
    /// Everywhere a value is evaluated during a run: never a const context (`E0323`).
    Everywhere,
}

/// The signature of a core primitive of the subset: its result is always `f64`.
#[derive(Debug, Clone, Copy)]
pub(super) struct CoreSig {
    pub(super) required: &'static [CoreParam],
    pub(super) optional: &'static [CoreParam],
    pub(super) allowed: Allowed,
    pub(super) result: CoreResult,
}

/// The §14 rows the subset types, by their spelling in `pom/builtin.rs`. A built-in with no row
/// here (a built-in the table does not list) is
/// `P0001`, which is how it stays refused without a line of code each.
const CORE_ROWS: &[(&str, CoreSig)] = &[
    ("ddt", CoreSig { required: &[CoreParam::F64], optional: &[CoreParam::F64], allowed: Allowed::Instances, result: CoreResult::F64 }),
    ("limit", CoreSig { required: &[CoreParam::F64, CoreParam::Limiter], optional: &[CoreParam::F64], allowed: Allowed::Instances, result: CoreResult::F64 }),
    ("ddx", CoreSig { required: &[CoreParam::F64, CoreParam::Probe], optional: &[], allowed: Allowed::Analog, result: CoreResult::F64 }),
    ("delay", CoreSig { required: &[CoreParam::F64, CoreParam::F64], optional: &[CoreParam::OptionF64], allowed: Allowed::Instances, result: CoreResult::F64 }),
    ("noise", CoreSig { required: &[CoreParam::F64], optional: &[CoreParam::OptionText], allowed: Allowed::Instances, result: CoreResult::F64 }),
    ("simparam", CoreSig { required: &[CoreParam::Text], optional: &[], allowed: Allowed::Everywhere, result: CoreResult::OptionF64 }),
    ("ac_stim", CoreSig { required: &[CoreParam::F64], optional: &[CoreParam::F64], allowed: Allowed::Analog, result: CoreResult::F64 }),
    ("temperature", CoreSig { required: &[], optional: &[], allowed: Allowed::Everywhere, result: CoreResult::F64 }),
    ("gmin", CoreSig { required: &[], optional: &[], allowed: Allowed::Everywhere, result: CoreResult::F64 }),
    ("source_scale", CoreSig { required: &[], optional: &[], allowed: Allowed::Everywhere, result: CoreResult::F64 }),
    ("now", CoreSig { required: &[], optional: &[], allowed: Allowed::Everywhere, result: CoreResult::F64 }),
    // `cross` and `at` are core modules with an `Event` result, placed in event positions only
    // (`core_call` judges the position, `Checker::event_arguments` the tolerances and the time).
    ("cross", CoreSig { required: &[CoreParam::F64], optional: &[CoreParam::OptionF64, CoreParam::OptionF64], allowed: Allowed::Everywhere, result: CoreResult::Event }),
    ("at", CoreSig { required: &[CoreParam::F64], optional: &[], allowed: Allowed::Everywhere, result: CoreResult::Event }),
    // `freq()` is continuous (§14.4.2, `continuity.rs`); `bound_step(dt)` is a statement of `analog` (§14.10).
    ("freq", CoreSig { required: &[], optional: &[], allowed: Allowed::Everywhere, result: CoreResult::F64 }),
    ("bound_step", CoreSig { required: &[CoreParam::F64], optional: &[], allowed: Allowed::Analog, result: CoreResult::Unit }),
    // `analysis(kind)`: its one argument is a kind of analysis, checked by name in `core_call`.
    ("analysis", CoreSig { required: &[], optional: &[], allowed: Allowed::Everywhere, result: CoreResult::Bool }),
];

pub(super) struct Core;

impl Core {
    /// The row of a core primitive of the subset, if it has one.
    pub(super) fn signature(id: BuiltinId) -> Option<CoreSig> {
        let name = id.info().name;
        CORE_ROWS.iter().find(|(row, _)| *row == name).map(|(_, sig)| *sig)
    }
}
