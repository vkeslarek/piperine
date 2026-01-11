use faer::traits::ComplexField;
use num_complex::Complex;
use num_traits::{One, Zero};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

pub trait Field:
    Copy
    + Clone
    + PartialEq
    + Zero
    + One
    + ComplexField
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
    fn abs(self) -> f64;
}

pub trait ScalableByReal: Mul<f64, Output = Self> {}

impl Field for f64 {
    fn abs(self) -> f64 {
        self.abs()
    }
}
impl ScalableByReal for f64 {}

impl Field for Complex<f64> {
    fn abs(self) -> f64 {
        self.norm()
    }
}
impl ScalableByReal for Complex<f64> {}
