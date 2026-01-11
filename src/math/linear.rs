use crate::circuit::netlist::CircuitReference;
use crate::math::num::Field;
use ndarray::Array1;
use std::collections::HashMap;
use std::hash::Hash;

pub trait Symbol: Clone + Eq + Hash {}

#[derive(Debug, Clone)]
pub enum Stamp<S: Symbol, E: Field> {
    Matrix(S, S, E),
    Rhs(S, E),
}

impl<E: Field> Stamp<CircuitReference, E> {
    pub fn has_ground_node(&self) -> bool {
        match self {
            Stamp::Matrix(a, b, _) => a.is_ground() || b.is_ground(),
            Stamp::Rhs(a, _) => a.is_ground(),
        }
    }
}

pub trait LinearSystem<S: Symbol, E: Field> {
    type SymbolicType: SymbolicMatrix<S>;

    fn new(size: usize) -> Self;
    fn apply_stamps(&mut self, symbolic: &Self::SymbolicType, stamps: Vec<Stamp<S, E>>);
    fn solve_with_backend(self, symbolic: &Self::SymbolicType) -> crate::result::Result<Array1<E>>;
}

pub trait SymbolicMatrix<S: Symbol> {
    fn size(&self) -> usize;

    fn mapping(&self) -> &HashMap<S, usize>;
}
