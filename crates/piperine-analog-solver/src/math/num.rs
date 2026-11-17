//! The `Scalar` trait: the numeric-field bounds (real and complex) every
//! math routine is generic over.
use faer::traits::ComplexField;
use num_complex::Complex;
use num_traits::{One, Zero};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

pub trait Scalar:
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
    fn is_finite(&self) -> bool;
    /// The real number a diagnostic sees in place of this entry: the value
    /// itself for a real scalar, the magnitude for a complex one.
    fn diagnostic_value(self) -> f64;
}

impl Scalar for f64 {
    fn abs(self) -> f64 {
        self.abs()
    }

    fn is_finite(&self) -> bool {
        f64::is_finite(*self)
    }

    fn diagnostic_value(self) -> f64 {
        self
    }
}

impl Scalar for Complex<f64> {
    fn abs(self) -> f64 {
        self.norm()
    }

    fn is_finite(&self) -> bool {
        Complex::is_finite(*self)
    }

    fn diagnostic_value(self) -> f64 {
        self.norm()
    }
}
