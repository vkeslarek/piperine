//! `Float128`: IEEE 754 binary128 (`f128`) stored as its `u128` bits (CV-31).
//!
//! Every operation is exact limb arithmetic rounded once by [`BinaryFormat::QUAD`], so it is the
//! correctly rounded result, which is stronger than H.2 rule 6's one-ulp bound. The
//! transcendental methods are in `transcendental.rs`. Comparisons are IEEE 754: NaN is unordered
//! and unequal, and `-0.0 == 0.0`.

use super::arithmetic::IeeeArithmetic;
use super::format::BinaryFormat;
use core::cmp::Ordering;
use core::ops::{Add, Div, Mul, Neg, Sub};

/// A binary128 value, held as its 128 bits.
#[derive(Clone, Copy, Debug)]
pub struct Float128(u128);

impl Float128 {
    /// The canonical quiet NaN.
    pub const NAN: Float128 = Float128(0x7FFF_8000_0000_0000_0000_0000_0000_0000);
    /// Positive infinity.
    pub const INFINITY: Float128 = Float128(0x7FFF_0000_0000_0000_0000_0000_0000_0000);
    /// Negative infinity.
    pub const NEGATIVE_INFINITY: Float128 = Float128(0xFFFF_0000_0000_0000_0000_0000_0000_0000);
    /// Positive zero.
    pub const ZERO: Float128 = Float128(0);
    /// Negative zero.
    pub const NEGATIVE_ZERO: Float128 = Float128(1u128 << 127);
    /// One.
    pub const ONE: Float128 = Float128(0x3FFF_0000_0000_0000_0000_0000_0000_0000);

    /// The value with the given bit pattern.
    pub const fn from_bits(bits: u128) -> Self {
        Float128(bits)
    }

    /// The bit pattern.
    pub const fn to_bits(self) -> u128 {
        self.0
    }

    /// The exact operations of the format.
    pub(crate) fn arithmetic() -> IeeeArithmetic {
        IeeeArithmetic::new(BinaryFormat::QUAD)
    }

    /// Whether the sign bit is set (NaN and zeros included).
    pub fn is_sign_negative(self) -> bool {
        BinaryFormat::QUAD.is_negative(self.0)
    }

    /// Whether the value is NaN.
    pub fn is_nan(self) -> bool {
        Self::arithmetic().is_nan(self.0)
    }

    /// Whether the value is neither infinite nor NaN.
    pub fn is_finite(self) -> bool {
        Self::arithmetic().is_finite(self.0)
    }

    /// Whether the value is an infinity.
    pub fn is_infinite(self) -> bool {
        Self::arithmetic().is_infinite(self.0)
    }

    /// Whether the value is a zero of either sign.
    pub fn is_zero(self) -> bool {
        Self::arithmetic().is_zero(self.0)
    }

    /// `|x|`.
    pub fn abs(self) -> Self {
        Float128(Self::arithmetic().absolute(self.0))
    }

    /// The square root, correctly rounded (NaN below zero, `-0` for `-0`).
    pub fn sqrt(self) -> Self {
        Float128(Self::arithmetic().square_root(self.0))
    }

    /// The greatest integer not above the value.
    pub fn floor(self) -> Self {
        Float128(Self::arithmetic().floor(self.0))
    }

    /// The least integer not below the value.
    pub fn ceil(self) -> Self {
        Float128(Self::arithmetic().ceiling(self.0))
    }

    /// The nearest integer, a tie going away from zero.
    pub fn round(self) -> Self {
        Float128(Self::arithmetic().round_half_away(self.0))
    }

    /// The integer part toward zero.
    pub fn trunc(self) -> Self {
        Float128(Self::arithmetic().truncate(self.0))
    }

    /// `x - trunc(x)`.
    pub fn fract(self) -> Self {
        Float128(Self::arithmetic().fraction(self.0))
    }

    /// `1` or `-1` by the sign, NaN for NaN.
    pub fn signum(self) -> Self {
        Float128(Self::arithmetic().signum(self.0))
    }

    /// The lesser value; one NaN operand yields the other (H.2 rule 3).
    pub fn min(self, other: Self) -> Self {
        Float128(Self::arithmetic().minimum(self.0, other.0))
    }

    /// The greater value; one NaN operand yields the other (H.2 rule 3).
    pub fn max(self, other: Self) -> Self {
        Float128(Self::arithmetic().maximum(self.0, other.0))
    }

    /// `clamp(lo, hi)`, `None` when `lo > hi` or a bound is NaN (H.2 rule 4).
    pub fn clamp_checked(self, lowest: Self, highest: Self) -> Option<Self> {
        Self::arithmetic().clamp(self.0, lowest.0, highest.0).map(Float128)
    }

    /// The Euclidean remainder rounded once (H.2 rule 9); NaN for a zero divisor.
    pub fn rem_euclid(self, divisor: Self) -> Self {
        Float128(Self::arithmetic().remainder_euclid(self.0, divisor.0))
    }

    /// `sqrt(x^2 + y^2)` without overflow in the squares, correctly rounded (H.2 rule 8).
    pub fn hypot(self, other: Self) -> Self {
        Float128(Self::arithmetic().hypot(self.0, other.0))
    }
}

impl PartialEq for Float128 {
    fn eq(&self, other: &Self) -> bool {
        Self::arithmetic().compare(self.0, other.0) == Some(Ordering::Equal)
    }
}

impl PartialOrd for Float128 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Self::arithmetic().compare(self.0, other.0)
    }
}

impl Add for Float128 {
    type Output = Float128;
    fn add(self, other: Float128) -> Float128 {
        Float128(Self::arithmetic().add(self.0, other.0))
    }
}

impl Sub for Float128 {
    type Output = Float128;
    fn sub(self, other: Float128) -> Float128 {
        Float128(Self::arithmetic().subtract(self.0, other.0))
    }
}

impl Mul for Float128 {
    type Output = Float128;
    fn mul(self, other: Float128) -> Float128 {
        Float128(Self::arithmetic().multiply(self.0, other.0))
    }
}

impl Div for Float128 {
    type Output = Float128;
    fn div(self, other: Float128) -> Float128 {
        Float128(Self::arithmetic().divide(self.0, other.0))
    }
}

impl Neg for Float128 {
    type Output = Float128;
    fn neg(self) -> Float128 {
        Float128(Self::arithmetic().negate(self.0))
    }
}
