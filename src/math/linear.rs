use crate::circuit::netlist::CircuitVariable;
use crate::math::Symbol;
use crate::math::num::Field;
use ndarray::{Array1, Array2, ArrayRef, ArrayView1};
use std::collections::HashMap;

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

pub trait SymbolicMatrix2 {
    fn size(&self) -> usize;
    fn new<A: AsIndex, E: Field>(
        size: usize,
        stamp: Vec<Stamp2<A, E>>,
    ) -> crate::result::Result<Self>
    where
        Self: Sized;
}

pub trait SparseLinearSystem2<E: Field> {
    type SymbolicType: SymbolicMatrix2;

    fn new(size: usize) -> Self;
    fn apply_stamps<A: AsIndex>(&mut self, stamps: Vec<Stamp2<A, E>>);
    fn solve_with_backend(&self, symbolic: &Self::SymbolicType)
    -> crate::result::Result<Array1<E>>;
    fn solve(&self) -> crate::result::Result<Array1<E>>;
}

#[derive(Debug, Clone)]
pub enum Stamp<S: Symbol, E: Field> {
    Matrix(S, S, E),
    Rhs(S, E),
}

impl<E: Field> Stamp<CircuitVariable, E> {
    pub fn has_ground_node(&self) -> bool {
        match self {
            Stamp::Matrix(a, b, _) => a.is_ground() || b.is_ground(),
            Stamp::Rhs(a, _) => a.is_ground(),
        }
    }
}

pub trait SparseLinearSystem<S: Symbol, E: Field> {
    type SymbolicType: SymbolicMatrix<S>;

    fn new(size: usize) -> Self;
    fn apply_stamps(&mut self, symbolic: &Self::SymbolicType, stamps: Vec<Stamp<S, E>>);
    fn solve_with_backend(&self, symbolic: &Self::SymbolicType)
    -> crate::result::Result<Array1<E>>;
    fn solve(&self) -> crate::result::Result<Array1<E>>;
}

pub trait SymbolicMatrix<S: Symbol> {
    fn size(&self) -> usize;

    fn mapping(&self) -> &HashMap<S, usize>;
}

pub trait DenseLinearSystem<E: Field> {
    fn set_matrix(&mut self, matrix: &Array2<E>);
    fn set_rhs(&mut self, rhs: &Array1<E>);
    fn solve(&self) -> crate::result::Result<Array1<E>>;
}
