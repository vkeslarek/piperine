use num_complex::Complex;
use num_traits::{One, Zero};
use std::fmt::{Debug, Display, Formatter};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

pub trait Scalar:
    Copy
    + Clone
    + PartialEq
    + Zero
    + One
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
    + Neg<Output = Self>
    + AddAssign
    + SubAssign
    + MulAssign
    + DivAssign
{
}

impl Scalar for f64 {}

impl Scalar for Complex<f64> {}

#[derive(Debug, Copy, Clone)]
pub enum Expr {
    // TODO
}

#[derive(Debug, Clone)]
pub enum Dynamic<T: Scalar> {
    Literal(T),
    Expression(Expr),
}

impl<T: Scalar + Display> Display for Dynamic<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Dynamic::Literal(scalar) => scalar.fmt(f),
            Dynamic::Expression(expr) => expr.fmt(f),
        }
    }
}

impl<T: Scalar> From<T> for Dynamic<T> {
    fn from(val: T) -> Self {
        Dynamic::Literal(val)
    }
}

impl<T: Scalar> From<Expr> for Dynamic<T> {
    fn from(expr: Expr) -> Self {
        Dynamic::Expression(expr)
    }
}
