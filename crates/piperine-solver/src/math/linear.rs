//! Linear-system vocabulary: the `Stamp` (matrix/rhs entry) currency
//! devices pay, and the `LinearSystem`/`SymbolicLinearSystem` contracts
//! sparse backends implement.
use crate::math::num::Scalar;
use ndarray::Array1;

pub trait AsIndex {
    fn as_index(&self) -> Option<usize>;
}

#[derive(Debug, Clone)]
pub enum Stamp<A: AsIndex, E: Scalar> {
    Matrix(A, A, E),
    Rhs(A, E),
}

pub trait LinearSystem<E: Scalar> {
    fn new(size: usize) -> Self;
    fn apply_stamps<A: AsIndex>(&mut self, stamps: Vec<Stamp<A, E>>);
    /// Clear stamps + RHS in-place for reuse across Newton iterations.
    /// Call instead of `new()` to avoid per-iteration heap allocation.
    fn reset(&mut self);
}

pub trait SymbolicMatrix {
    fn size(&self) -> usize;
    fn new<A: AsIndex, E: Scalar>(
        size: usize,
        stamp: Vec<Stamp<A, E>>,
    ) -> crate::result::Result<Self>
    where
        Self: Sized;
}

pub trait SymbolicLinearSystem<E: Scalar>: LinearSystem<E> {
    type SymbolicType: SymbolicMatrix;

    fn solve_with_backend(&self, symbolic: &Self::SymbolicType)
    -> crate::result::Result<Array1<E>>;
}
