//! The closed method catalogue of the M1 subset: the rows of Appendix H.2 the models call.
//! A table of rows, one line each to extend; a method outside it is `P0001`.

use crate::pom::BuiltinId;

use super::{Method, Ty};

/// The Appendix H.2 rows of `f64` the subset types. Each gives `Self`; the second column is the
/// number of arguments, each of the receiver's type.
const F64_METHODS: &[(&str, Method, usize)] = &[
    ("exp", Method::Exp, 0),
    ("ln", Method::Ln, 0),
    ("sqrt", Method::Sqrt, 0),
    ("abs", Method::Abs, 0),
    ("floor", Method::Floor, 0),
    ("min", Method::Min, 1),
    ("max", Method::Max, 1),
    ("clamp", Method::Clamp, 2),
];

/// The Appendix H rows of `UInt<N>` and `Int<N>` the subset types, as `F64_METHODS`.
const INTEGER_METHODS: &[(&str, Method, usize)] = &[("min", Method::Min, 1), ("max", Method::Max, 1), ("clamp", Method::Clamp, 2)];

pub(super) struct Catalogue;

impl Catalogue {
    /// The row named `name` for a receiver of type `recv`: the row, its argument types and its
    /// result.
    pub(super) fn method(recv: &Ty, name: &str) -> Option<(Method, Vec<Ty>, Ty)> {
        let rows = match recv {
            Ty::F64 => F64_METHODS,
            Ty::UInt(_) | Ty::Int(_) => INTEGER_METHODS,
            Ty::Option(inner) => return Self::option(inner, name),
            Ty::Bool | Ty::Unit | Ty::Node(_) | Ty::AnyNode | Ty::Fn(..) | Ty::Module(_) | Ty::Enum(_) | Ty::Tuple(_) => return None,
        };
        rows.iter().find(|(row, _, _)| *row == name).map(|(_, method, arity)| (*method, vec![recv.clone(); *arity], recv.clone()))
    }

    /// The rows of `Option<T>` (Appendix H): `is_some`, `is_none`, `unwrap_or(T) -> T`.
    fn option(inner: &Ty, name: &str) -> Option<(Method, Vec<Ty>, Ty)> {
        match name {
            "is_some" => Some((Method::IsSome, vec![], Ty::Bool)),
            "is_none" => Some((Method::IsNone, vec![], Ty::Bool)),
            "unwrap_or" => Some((Method::UnwrapOr, vec![inner.clone()], inner.clone())),
            _ => None,
        }
    }
}

/// One parameter of a core primitive of the subset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CoreParam {
    F64,
    /// `fn(f64, f64) -> f64`: the input function of `limit`.
    Limiter,
}

impl CoreParam {
    pub(super) fn ty(self) -> Ty {
        match self {
            CoreParam::F64 => Ty::F64,
            CoreParam::Limiter => Ty::Fn(vec![Ty::F64, Ty::F64], Box::new(Ty::F64)),
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
}

/// The §14 rows the subset types, by their spelling in `pom/builtin.rs`. A built-in with no row
/// here (`delay`, `noise`, `ddx`, `cross`, `at`, `now`, `freq`, `analysis`, `simparam`, ...) is
/// `P0001`, which is how it stays refused without a line of code each.
const CORE_ROWS: &[(&str, CoreSig)] = &[
    ("ddt", CoreSig { required: &[CoreParam::F64], optional: &[CoreParam::F64], allowed: Allowed::Instances }),
    ("limit", CoreSig { required: &[CoreParam::F64, CoreParam::Limiter], optional: &[CoreParam::F64], allowed: Allowed::Instances }),
    ("ac_stim", CoreSig { required: &[CoreParam::F64], optional: &[CoreParam::F64], allowed: Allowed::Analog }),
    ("temperature", CoreSig { required: &[], optional: &[], allowed: Allowed::Everywhere }),
    ("gmin", CoreSig { required: &[], optional: &[], allowed: Allowed::Everywhere }),
    ("source_scale", CoreSig { required: &[], optional: &[], allowed: Allowed::Everywhere }),
];

pub(super) struct Core;

impl Core {
    /// The row of a core primitive of the subset, if it has one.
    pub(super) fn signature(id: BuiltinId) -> Option<CoreSig> {
        let name = id.info().name;
        CORE_ROWS.iter().find(|(row, _)| *row == name).map(|(_, sig)| *sig)
    }
}
