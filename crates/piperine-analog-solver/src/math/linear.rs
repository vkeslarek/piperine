//! Linear-system vocabulary: the `Stamp` (matrix/rhs entry) currency
//! devices pay, and the type of the linear system the Newton solver builds (`faer.rs`).
use crate::math::num::Scalar;

pub trait AsIndex {
    fn as_index(&self) -> Option<usize>;
}

impl AsIndex for usize {
    fn as_index(&self) -> Option<usize> {
        Some(*self)
    }
}

#[derive(Debug, Clone)]
pub enum Stamp<A: AsIndex, E: Scalar> {
    Matrix(A, A, E),
    Rhs(A, E),
}
