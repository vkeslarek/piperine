use crate::math::num::Field;
use std::ops::{Add, Neg, Sub};

pub trait VectorZero {
    fn zero(dimension: usize) -> Self;
}

pub trait Vector<E: Field>: Clone + Sized + VectorZero + PartialEq + Add + Sub + Neg {
    fn scale(&self, factor: E) -> Self;
}
