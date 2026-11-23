//! `Float16`: IEEE 754 binary16 (`f16`) stored as its bits (CV-30).
//!
//! `+ - * /` and `sqrt` are computed in `f32` and rounded once to `f16`; that is exact, because a
//! 24-bit significand is at least `2 * 11 + 2` (Figueroa, "When is double rounding innocuous?").
//! The exact H.2 methods delegate to [`IeeeArithmetic`]. Comparisons are IEEE 754: NaN is
//! unordered and unequal, and `-0.0 == 0.0`.

use super::arithmetic::IeeeArithmetic;
use super::format::{BinaryFormat, FloatClass};
use super::limbs::ToLimbs;
use core::cmp::Ordering;
use core::ops::{Add, Div, Mul, Neg, Sub};

/// A binary16 value, held as its 16 bits.
#[derive(Clone, Copy, Debug)]
pub struct Float16(u16);

impl Float16 {
    /// The canonical quiet NaN.
    pub const NAN: Float16 = Float16(0x7E00);
    /// Positive zero.
    pub const ZERO: Float16 = Float16(0);
    /// Positive infinity.
    pub const INFINITY: Float16 = Float16(0x7C00);

    /// The value with the given bit pattern.
    pub const fn from_bits(bits: u16) -> Self {
        Float16(bits)
    }

    /// The bit pattern.
    pub const fn to_bits(self) -> u16 {
        self.0
    }

    /// The exact operations of the format.
    fn arithmetic() -> IeeeArithmetic {
        IeeeArithmetic::new(BinaryFormat::HALF)
    }

    /// The value as a pattern in another format, exact when that format is wider (`f32`, `f64`).
    fn widened(self, target: BinaryFormat) -> u128 {
        match BinaryFormat::HALF.decode(self.0 as u128) {
            Some((negative, significand, exponent)) => target.round(negative, &significand.to_limbs(), exponent, false),
            None => match BinaryFormat::HALF.classify(self.0 as u128) {
                FloatClass::Infinite => target.infinity(BinaryFormat::HALF.is_negative(self.0 as u128)),
                _ => target.quiet_nan(),
            },
        }
    }

    /// The same value as an `f32` (exact).
    pub fn to_f32(self) -> f32 {
        f32::from_bits(self.widened(BinaryFormat::SINGLE) as u32)
    }

    /// The same value as an `f64` (exact).
    pub fn to_f64(self) -> f64 {
        f64::from_bits(self.widened(BinaryFormat::DOUBLE) as u64)
    }

    /// The nearest `f16` to a pattern of another format, rounded once from that pattern.
    fn narrowed(source: BinaryFormat, pattern: u128) -> Self {
        let bits = match source.decode(pattern) {
            Some((negative, significand, exponent)) => BinaryFormat::HALF.round(negative, &significand.to_limbs(), exponent, false),
            None => match source.classify(pattern) {
                FloatClass::Infinite => BinaryFormat::HALF.infinity(source.is_negative(pattern)),
                _ => BinaryFormat::HALF.quiet_nan(),
            },
        };
        Float16(bits as u16)
    }

    /// The `f16` nearest to an `f32`, ties to even.
    pub fn from_f32(value: f32) -> Self {
        Self::narrowed(BinaryFormat::SINGLE, value.to_bits() as u128)
    }

    /// The `f16` nearest to an `f64`, ties to even, rounded once (never through `f32`).
    pub fn from_f64(value: f64) -> Self {
        Self::narrowed(BinaryFormat::DOUBLE, value.to_bits() as u128)
    }

    /// The square root, correctly rounded.
    pub fn sqrt(self) -> Self {
        Self::from_f32(self.to_f32().sqrt())
    }

    /// Whether the value is NaN.
    pub fn is_nan(self) -> bool {
        Self::arithmetic().is_nan(self.0 as u128)
    }

    /// Whether the value is neither infinite nor NaN.
    pub fn is_finite(self) -> bool {
        Self::arithmetic().is_finite(self.0 as u128)
    }

    /// Whether the value is an infinity.
    pub fn is_infinite(self) -> bool {
        Self::arithmetic().is_infinite(self.0 as u128)
    }

    /// The pattern of a result of the exact operations.
    fn from_pattern(pattern: u128) -> Self {
        Float16(pattern as u16)
    }

    /// `|x|`.
    pub fn abs(self) -> Self {
        Self::from_pattern(Self::arithmetic().absolute(self.0 as u128))
    }

    /// The greatest integer not above the value.
    pub fn floor(self) -> Self {
        Self::from_pattern(Self::arithmetic().floor(self.0 as u128))
    }

    /// The least integer not below the value.
    pub fn ceil(self) -> Self {
        Self::from_pattern(Self::arithmetic().ceiling(self.0 as u128))
    }

    /// The nearest integer, a tie going away from zero.
    pub fn round(self) -> Self {
        Self::from_pattern(Self::arithmetic().round_half_away(self.0 as u128))
    }

    /// The integer part toward zero.
    pub fn trunc(self) -> Self {
        Self::from_pattern(Self::arithmetic().truncate(self.0 as u128))
    }

    /// `x - trunc(x)`.
    pub fn fract(self) -> Self {
        Self::from_pattern(Self::arithmetic().fraction(self.0 as u128))
    }

    /// `1` or `-1` by the sign, NaN for NaN.
    pub fn signum(self) -> Self {
        Self::from_pattern(Self::arithmetic().signum(self.0 as u128))
    }

    /// The lesser value; one NaN operand yields the other (H.2 rule 3).
    pub fn min(self, other: Self) -> Self {
        Self::from_pattern(Self::arithmetic().minimum(self.0 as u128, other.0 as u128))
    }

    /// The greater value; one NaN operand yields the other (H.2 rule 3).
    pub fn max(self, other: Self) -> Self {
        Self::from_pattern(Self::arithmetic().maximum(self.0 as u128, other.0 as u128))
    }

    /// `clamp(lo, hi)`, `None` when `lo > hi` or a bound is NaN (H.2 rule 4).
    pub fn clamp_checked(self, lowest: Self, highest: Self) -> Option<Self> {
        Self::arithmetic().clamp(self.0 as u128, lowest.0 as u128, highest.0 as u128).map(Self::from_pattern)
    }

    /// The Euclidean remainder rounded once (H.2 rule 9); NaN for a zero divisor.
    pub fn rem_euclid(self, divisor: Self) -> Self {
        Self::from_pattern(Self::arithmetic().remainder_euclid(self.0 as u128, divisor.0 as u128))
    }

    /// `sqrt(x^2 + y^2)` without overflow in the squares, correctly rounded.
    pub fn hypot(self, other: Self) -> Self {
        Self::from_pattern(Self::arithmetic().hypot(self.0 as u128, other.0 as u128))
    }
}

impl PartialEq for Float16 {
    fn eq(&self, other: &Self) -> bool {
        Self::arithmetic().compare(self.0 as u128, other.0 as u128) == Some(Ordering::Equal)
    }
}

impl PartialOrd for Float16 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Self::arithmetic().compare(self.0 as u128, other.0 as u128)
    }
}

impl Add for Float16 {
    type Output = Float16;
    fn add(self, other: Float16) -> Float16 {
        Float16::from_f32(self.to_f32() + other.to_f32())
    }
}

impl Sub for Float16 {
    type Output = Float16;
    fn sub(self, other: Float16) -> Float16 {
        Float16::from_f32(self.to_f32() - other.to_f32())
    }
}

impl Mul for Float16 {
    type Output = Float16;
    fn mul(self, other: Float16) -> Float16 {
        Float16::from_f32(self.to_f32() * other.to_f32())
    }
}

impl Div for Float16 {
    type Output = Float16;
    fn div(self, other: Float16) -> Float16 {
        Float16::from_f32(self.to_f32() / other.to_f32())
    }
}

impl Neg for Float16 {
    type Output = Float16;
    fn neg(self) -> Float16 {
        Float16(self.0 ^ 0x8000)
    }
}
