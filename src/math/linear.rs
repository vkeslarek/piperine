use crate::math::num::Field;
use ndarray::{Array1, Array2, ArrayRef, ArrayView1};

pub trait AsIndex {
    fn as_index(&self) -> Option<usize>;
}

pub trait AsIndexGetExt<A: AsIndex, E> {
    fn get(&self, value: &A) -> Option<&E>;
}

impl<A: AsIndex, E> AsIndexGetExt<A, E> for ArrayView1<'_, E> {
    fn get(&self, value: &A) -> Option<&E> {
        value.as_index().and_then(|idx| ArrayRef::get(self, idx))
    }
}

#[derive(Debug, Clone)]
pub enum Stamp2<A: AsIndex, E: Field> {
    Matrix(A, A, E),
    Rhs(A, E),
}

pub trait SymbolicMatrix {
    fn size(&self) -> usize;
    fn new<A: AsIndex, E: Field>(
        size: usize,
        stamp: Vec<Stamp2<A, E>>,
    ) -> crate::result::Result<Self>
    where
        Self: Sized;
}

pub trait SparseLinearSystem<E: Field> {
    type SymbolicType: SymbolicMatrix;

    fn new(size: usize) -> Self;
    fn apply_stamps<A: AsIndex>(&mut self, stamps: Vec<Stamp2<A, E>>);
    fn solve_with_backend(&self, symbolic: &Self::SymbolicType)
    -> crate::result::Result<Array1<E>>;
    fn solve(&self) -> crate::result::Result<Array1<E>>;
}

pub trait DenseLinearSystem<E: Field> {
    fn set_matrix(&mut self, matrix: &Array2<E>);
    fn set_rhs(&mut self, rhs: &Array1<E>);
    fn solve(&self) -> crate::result::Result<Array1<E>>;
}
