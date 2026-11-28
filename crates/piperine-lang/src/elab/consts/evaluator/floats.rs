//! The floats of the const evaluator that are not `f64` (composite-values phase D, CV-29 to CV-31,
//! CV-36): `f32` (Rust's own `f32`, as the generated code computes it), `f16` and `f128`
//! (`piperine-numbers`, the library the generated code embeds). One trait states the operations
//! Appendix H.2 gives a float, so the evaluator's dispatch over the three formats is one generic
//! function and each implementation is a list of one-line calls to the type's own methods.

use std::ops::{Add, Div, Mul, Neg, Sub};

use piperine_numbers::half::Float16;
use piperine_numbers::quad::Float128;

use crate::types::Method;

/// An IEEE 754 binary float the evaluator folds: `f32`, `f16` or `f128`.
pub(in crate::elab) trait Ieee754: Copy + Add<Output = Self> + Sub<Output = Self> + Mul<Output = Self> + Div<Output = Self> + Neg<Output = Self> + PartialOrd {
    /// The value of a bit pattern of the format.
    fn from_pattern(pattern: u128) -> Self;
    /// The bit pattern.
    fn pattern(self) -> u128;
    /// A row of H.2 with no argument that gives a float; `None` for any other row.
    fn unary(self, method: Method) -> Option<Self>;
    /// A row of H.2 with one argument of the type that gives a float; `None` for any other row.
    fn binary(self, method: Method, other: Self) -> Option<Self>;
    /// `self.powi(exponent)`.
    fn power_integer(self, exponent: i32) -> Self;
    /// `self.clamp(lowest, highest)`; `None` for crossed or NaN bounds (a panic, §1 rule 11).
    fn clamp(self, lowest: Self, highest: Self) -> Option<Self>;
    /// Whether the value is NaN.
    fn is_nan(self) -> bool;
    /// Whether the value is finite.
    fn is_finite(self) -> bool;
    /// Whether the value is infinite.
    fn is_infinite(self) -> bool;
}

impl Ieee754 for f32 {
    fn from_pattern(pattern: u128) -> f32 {
        f32::from_bits(pattern as u32)
    }
    fn pattern(self) -> u128 {
        u128::from(self.to_bits())
    }
    fn unary(self, method: Method) -> Option<f32> {
        Some(match method {
            Method::Abs => self.abs(),
            Method::Sqrt => self.sqrt(),
            Method::Exp => self.exp(),
            Method::Ln => self.ln(),
            Method::Log10 => self.log10(),
            Method::Log2 => self.log2(),
            Method::Sin => self.sin(),
            Method::Cos => self.cos(),
            Method::Tan => self.tan(),
            Method::Asin => self.asin(),
            Method::Acos => self.acos(),
            Method::Atan => self.atan(),
            Method::Sinh => self.sinh(),
            Method::Cosh => self.cosh(),
            Method::Tanh => self.tanh(),
            Method::Floor => self.floor(),
            Method::Ceil => self.ceil(),
            Method::Round => self.round(),
            Method::Trunc => self.trunc(),
            Method::Fract => self.fract(),
            Method::Signum => self.signum(),
            _ => return None,
        })
    }
    fn binary(self, method: Method, other: f32) -> Option<f32> {
        Some(match method {
            Method::Powf => self.powf(other),
            Method::Atan2 => self.atan2(other),
            Method::Min => self.min(other),
            Method::Max => self.max(other),
            Method::RemEuclid => self.rem_euclid(other),
            Method::Hypot => self.hypot(other),
            _ => return None,
        })
    }
    fn power_integer(self, exponent: i32) -> f32 {
        self.powi(exponent)
    }
    fn clamp(self, lowest: f32, highest: f32) -> Option<f32> {
        (!(lowest.is_nan() || highest.is_nan() || lowest > highest)).then(|| f32::clamp(self, lowest, highest))
    }
    fn is_nan(self) -> bool {
        f32::is_nan(self)
    }
    fn is_finite(self) -> bool {
        f32::is_finite(self)
    }
    fn is_infinite(self) -> bool {
        f32::is_infinite(self)
    }
}

impl Ieee754 for Float16 {
    fn from_pattern(pattern: u128) -> Float16 {
        Float16::from_bits(pattern as u16)
    }
    fn pattern(self) -> u128 {
        u128::from(self.to_bits())
    }
    fn unary(self, method: Method) -> Option<Float16> {
        Some(match method {
            Method::Abs => self.abs(),
            Method::Sqrt => self.sqrt(),
            Method::Exp => self.exp(),
            Method::Ln => self.ln(),
            Method::Log10 => self.log10(),
            Method::Log2 => self.log2(),
            Method::Sin => self.sin(),
            Method::Cos => self.cos(),
            Method::Tan => self.tan(),
            Method::Asin => self.asin(),
            Method::Acos => self.acos(),
            Method::Atan => self.atan(),
            Method::Sinh => self.sinh(),
            Method::Cosh => self.cosh(),
            Method::Tanh => self.tanh(),
            Method::Floor => self.floor(),
            Method::Ceil => self.ceil(),
            Method::Round => self.round(),
            Method::Trunc => self.trunc(),
            Method::Fract => self.fract(),
            Method::Signum => self.signum(),
            _ => return None,
        })
    }
    fn binary(self, method: Method, other: Float16) -> Option<Float16> {
        Some(match method {
            Method::Powf => self.powf(other),
            Method::Atan2 => self.atan2(other),
            Method::Min => self.min(other),
            Method::Max => self.max(other),
            Method::RemEuclid => self.rem_euclid(other),
            Method::Hypot => self.hypot(other),
            _ => return None,
        })
    }
    fn power_integer(self, exponent: i32) -> Float16 {
        self.powi(exponent)
    }
    fn clamp(self, lowest: Float16, highest: Float16) -> Option<Float16> {
        self.clamp_checked(lowest, highest)
    }
    fn is_nan(self) -> bool {
        Float16::is_nan(self)
    }
    fn is_finite(self) -> bool {
        Float16::is_finite(self)
    }
    fn is_infinite(self) -> bool {
        Float16::is_infinite(self)
    }
}

impl Ieee754 for Float128 {
    fn from_pattern(pattern: u128) -> Float128 {
        Float128::from_bits(pattern)
    }
    fn pattern(self) -> u128 {
        self.to_bits()
    }
    fn unary(self, method: Method) -> Option<Float128> {
        Some(match method {
            Method::Abs => self.abs(),
            Method::Sqrt => self.sqrt(),
            Method::Exp => self.exp(),
            Method::Ln => self.ln(),
            Method::Log10 => self.log10(),
            Method::Log2 => self.log2(),
            Method::Sin => self.sin(),
            Method::Cos => self.cos(),
            Method::Tan => self.tan(),
            Method::Asin => self.asin(),
            Method::Acos => self.acos(),
            Method::Atan => self.atan(),
            Method::Sinh => self.sinh(),
            Method::Cosh => self.cosh(),
            Method::Tanh => self.tanh(),
            Method::Floor => self.floor(),
            Method::Ceil => self.ceil(),
            Method::Round => self.round(),
            Method::Trunc => self.trunc(),
            Method::Fract => self.fract(),
            Method::Signum => self.signum(),
            _ => return None,
        })
    }
    fn binary(self, method: Method, other: Float128) -> Option<Float128> {
        Some(match method {
            Method::Powf => self.powf(other),
            Method::Atan2 => self.atan2(other),
            Method::Min => self.min(other),
            Method::Max => self.max(other),
            Method::RemEuclid => self.rem_euclid(other),
            Method::Hypot => self.hypot(other),
            _ => return None,
        })
    }
    fn power_integer(self, exponent: i32) -> Float128 {
        self.powi(exponent)
    }
    fn clamp(self, lowest: Float128, highest: Float128) -> Option<Float128> {
        self.clamp_checked(lowest, highest)
    }
    fn is_nan(self) -> bool {
        Float128::is_nan(self)
    }
    fn is_finite(self) -> bool {
        Float128::is_finite(self)
    }
    fn is_infinite(self) -> bool {
        Float128::is_infinite(self)
    }
}
