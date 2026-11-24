//! Linear-system vocabulary: the `Stamp` (matrix/rhs entry) currency
//! devices pay, and the `LinearSystem`/`SymbolicLinearSystem` contracts
//! sparse backends implement.
use crate::math::num::Scalar;
use ndarray::Array1;

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

pub trait LinearSystem<E: Scalar> {
    fn new(size: usize) -> Self;
    /// Adds every stamp of `stamps`, which the caller keeps (and can refill) afterwards.
    fn apply_stamps_from<A: AsIndex>(&mut self, stamps: &[Stamp<A, E>]);
    /// Adds every stamp of `stamps`.
    fn apply_stamps<A: AsIndex>(&mut self, stamps: Vec<Stamp<A, E>>) {
        self.apply_stamps_from(&stamps);
    }
    /// Clear stamps + RHS in-place for reuse across Newton iterations.
    /// Call instead of `new()` to avoid per-iteration heap allocation.
    fn reset(&mut self);
    /// The matrix entries stamped since the last reset, as `(row, column,
    /// value)`: what a singularity diagnosis reads after a failed solve.
    fn matrix_entries(&self) -> Vec<(usize, usize, E)>;
}

pub trait SymbolicMatrix {
    fn size(&self) -> usize;
    fn new<A: AsIndex, E: Scalar>(
        size: usize,
        stamp: Vec<Stamp<A, E>>,
    ) -> crate::error::Result<Self>
    where
        Self: Sized;
}

pub trait SymbolicLinearSystem<E: Scalar>: LinearSystem<E> {
    type SymbolicType: SymbolicMatrix;

    fn solve_with_backend(&self, symbolic: &Self::SymbolicType)
    -> crate::error::Result<Array1<E>>;
}
