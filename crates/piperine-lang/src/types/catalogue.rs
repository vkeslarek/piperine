//! The closed method catalogue of the M1 subset: the rows of Appendix H.2 the models call.
//! A table of rows, one line each to extend; a method outside it is `P0001`.

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
