use crate::math::num::Field;
use ndarray::{Array1, ArrayRef, ArrayView1};

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
pub enum Stamp<A: AsIndex, E: Field> {
    Matrix(A, A, E),
    Rhs(A, E),
}

pub trait LinearSystem<E: Field> {
    fn new(size: usize) -> Self;
    fn apply_stamps<A: AsIndex>(&mut self, stamps: Vec<Stamp<A, E>>);
    fn solve(&self) -> crate::result::Result<Array1<E>>;
}

pub trait SymbolicMatrix {
    fn size(&self) -> usize;
    fn new<A: AsIndex, E: Field>(
        size: usize,
        stamp: Vec<Stamp<A, E>>,
    ) -> crate::result::Result<Self>
    where
        Self: Sized;
}

pub trait SymbolicLinearSystem<E: Field>: LinearSystem<E> {
    type SymbolicType: SymbolicMatrix;

    fn solve_with_backend(&self, symbolic: &Self::SymbolicType)
    -> crate::result::Result<Array1<E>>;
}

// This is ugly, I know =(
#[derive(Clone)]
pub struct NoSymbolic {
    pub size: usize,
}

impl SymbolicMatrix for NoSymbolic {
    fn size(&self) -> usize {
        self.size
    }
    fn new<A: AsIndex, E: Field>(size: usize, _: Vec<Stamp<A, E>>) -> crate::result::Result<Self> {
        Ok(Self { size })
    }
}
