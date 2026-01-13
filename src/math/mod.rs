use crate::circuit::netlist::CircuitReference;
use crate::math::num::Field;
use std::hash::Hash;

pub mod constant;
pub mod deriv;
pub mod faer;
pub mod linear;
pub mod newton_raphson;
pub mod num;
pub mod param;
pub mod rand;
pub mod unit;

pub trait Symbol: Clone + Eq + Hash {}

pub struct InitialValue<S: Symbol, E: Field> {
    pub reference: S,
    pub value: E,
}

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
