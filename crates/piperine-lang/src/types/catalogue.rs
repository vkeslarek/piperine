//! The closed method catalogue of the M1 subset: the rows of Appendix H.2 the models call.
//! A table of rows, one line each to extend; a method outside it is `P0001`.

use crate::pom::BuiltinId;

use super::{Method, Ty};

/// The Appendix H.2 rows of `f64` the subset types. Each is `fn name(self) -> Self`.
const F64_METHODS: &[(&str, Method)] = &[("exp", Method::Exp), ("ln", Method::Ln), ("sqrt", Method::Sqrt), ("abs", Method::Abs)];

pub(super) struct Catalogue;

impl Catalogue {
    /// The row named `name` for a receiver of type `recv`.
    pub(super) fn method(recv: &Ty, name: &str) -> Option<Method> {
        let rows = match recv {
            Ty::F64 => F64_METHODS,
            Ty::Bool | Ty::Unit | Ty::Node(_) | Ty::AnyNode | Ty::Fn(..) | Ty::Module(_) => return None,
        };
        rows.iter().find(|(row, _)| *row == name).map(|(_, method)| *method)
    }

    /// The type of the call: every row of the subset takes only `self` and gives `Self`.
    pub(super) fn result(_method: Method, recv: &Ty) -> Ty {
        recv.clone()
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
