//! `MultiPrecision` and `Interval`: binary floats of any precision with directed rounding.
//!
//! The transcendentals evaluate a function as an interval whose bounds are rounded outward, so
//! the exact value is always inside; [`Interval::round_to_quad`] then decides whether both bounds
//! round to the same `f128` (the Ziv test of `ziv.rs`). A value is
//! `(-1)^negative * magnitude * 2^exponent` with an odd magnitude (zero has none).

use super::format::BinaryFormat;
use super::limbs::{Limbs, ToLimbs};
use core::cmp::Ordering;

/// The direction a result is rounded in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rounding {
    /// Toward negative infinity: the result is at most the exact value.
    Down,
    /// Toward positive infinity: the result is at least the exact value.
    Up,
}

impl Rounding {
    /// The opposite direction.
    pub fn opposite(self) -> Rounding {
        match self {
            Rounding::Down => Rounding::Up,
            Rounding::Up => Rounding::Down,
        }
    }
}

/// A binary floating-point number of unbounded precision and exponent range.
#[derive(Clone, Debug)]
pub struct MultiPrecision {
    /// Whether the value is below zero (a zero is never negative).
    negative: bool,
    /// The odd significand (empty for zero).
    magnitude: Vec<u64>,
    /// The binary exponent of the significand's least bit.
    exponent: i64,
}

impl MultiPrecision {
    /// Zero.
    pub fn zero() -> Self {
        MultiPrecision { negative: false, magnitude: Vec::new(), exponent: 0 }
    }

    /// The exact value `(-1)^negative * magnitude * 2^exponent`, normalized.
    pub fn exact(negative: bool, magnitude: &[u64], exponent: i64) -> Self {
        let trimmed = magnitude.trimmed();
        if trimmed.is_empty() {
            return Self::zero();
        }
        let zeros = trimmed.trailing_zero_bits();
        MultiPrecision { negative, magnitude: trimmed.shifted_right(zeros), exponent: exponent + zeros as i64 }
    }

    /// The exact value of a `u64` with a sign.
    pub fn from_integer(negative: bool, value: u64) -> Self {
        Self::exact(negative, &value.to_limbs(), 0)
    }

    /// The exact value of a finite `f128` pattern, `None` for infinity and NaN.
    pub fn from_quad_bits(bits: u128) -> Option<Self> {
        let (negative, significand, exponent) = BinaryFormat::QUAD.decode(bits)?;
        Some(Self::exact(negative, &significand.to_limbs(), exponent))
    }

    /// Whether the value is below zero.
    pub fn is_negative(&self) -> bool {
        self.negative
    }

    /// Whether the value is zero.
    pub fn is_zero(&self) -> bool {
        self.magnitude.is_empty()
    }

    /// The odd significand, least significant limb first (empty for zero).
    pub fn magnitude(&self) -> &[u64] {
        &self.magnitude
    }

    /// The binary exponent of the significand's least bit.
    pub fn exponent(&self) -> i64 {
        self.exponent
    }

    /// `floor(log2 |x|)`, `None` for zero.
    pub fn top_exponent(&self) -> Option<i64> {
        (!self.is_zero()).then(|| self.exponent + self.magnitude.bit_length() as i64 - 1)
    }

    /// `-x`.
    pub fn negated(&self) -> Self {
        MultiPrecision { negative: !self.negative && !self.is_zero(), magnitude: self.magnitude.clone(), exponent: self.exponent }
    }

    /// `|x|`.
    pub fn absolute(&self) -> Self {
        MultiPrecision { negative: false, magnitude: self.magnitude.clone(), exponent: self.exponent }
    }

    /// `x * 2^amount`, exact.
    pub fn scaled(&self, amount: i64) -> Self {
        MultiPrecision { negative: self.negative, magnitude: self.magnitude.clone(), exponent: self.exponent + if self.is_zero() { 0 } else { amount } }
    }

    /// The exact order of two values.
    pub fn compare(&self, other: &Self) -> Ordering {
        match (self.is_zero(), other.is_zero(), self.negative, other.negative) {
            (true, true, _, _) => Ordering::Equal,
            (true, false, _, negative) => if negative { Ordering::Greater } else { Ordering::Less },
            (false, true, negative, _) => if negative { Ordering::Less } else { Ordering::Greater },
            (false, false, true, false) => Ordering::Less,
            (false, false, false, true) => Ordering::Greater,
            (false, false, negative, _) => {
                let order = self.compare_magnitude(other);
                if negative { order.reverse() } else { order }
            }
        }
    }

    /// The order of the magnitudes of two nonzero values.
    fn compare_magnitude(&self, other: &Self) -> Ordering {
        let (left_top, right_top) = (self.top_exponent().unwrap_or(i64::MIN), other.top_exponent().unwrap_or(i64::MIN));
        if left_top != right_top {
            return left_top.cmp(&right_top);
        }
        let exponent = self.exponent.min(other.exponent);
        let left = self.magnitude.shifted_left((self.exponent - exponent) as u64);
        let right = other.magnitude.shifted_left((other.exponent - exponent) as u64);
        left.compare_magnitude(&right)
    }

    /// The value as the pattern nearest to it in binary128 (ties to even).
    pub fn to_quad_bits(&self) -> u128 {
        BinaryFormat::QUAD.round(self.negative, &self.magnitude, self.exponent, false)
    }

    /// The value `(-1)^negative * (magnitude + epsilon) * 2^exponent`, with `epsilon` in `(0, 1)`
    /// when `sticky` and zero otherwise, rounded to `precision` bits in the given direction. A
    /// sticky value needs a magnitude of more than `precision` bits: the unit of the magnitude
    /// must be finer than the rounding window, or the position of `epsilon` is lost.
    pub fn rounded(negative: bool, magnitude: &[u64], exponent: i64, sticky: bool, precision: u32, rounding: Rounding) -> Self {
        let mut magnitude = magnitude.trimmed();
        let mut exponent = exponent;
        if magnitude.is_empty() {
            return Self::zero();
        }
        let precision = precision.max(2) as u64;
        let length = magnitude.bit_length();
        let mut inexact = sticky;
        if length > precision {
            let drop = length - precision;
            inexact = inexact || magnitude.any_bit_below(drop);
            magnitude = magnitude.shifted_right(drop);
            exponent += drop as i64;
        }
        let away_from_zero = (rounding == Rounding::Up) != negative;
        if inexact && away_from_zero {
            magnitude = magnitude.plus(&1u64.to_limbs());
        }
        Self::exact(negative, &magnitude, exponent)
    }

    /// `self + other` rounded to `precision` bits in the given direction.
    pub fn add(&self, other: &Self, precision: u32, rounding: Rounding) -> Self {
        if self.is_zero() {
            return Self::rounded(other.negative, &other.magnitude, other.exponent, false, precision, rounding);
        }
        if other.is_zero() {
            return Self::rounded(self.negative, &self.magnitude, self.exponent, false, precision, rounding);
        }
        let (large, small) = if self.compare_magnitude(other) == Ordering::Less { (other, self) } else { (self, other) };
        // Far case: write the larger operand in units finer than the rounding window, so the smaller
        // one is under one such unit and only its sign matters (a sticky bit above or below).
        let padding = (precision as u64 + 3).saturating_sub(large.magnitude.bit_length()) + 1;
        if small.top_exponent().unwrap_or(i64::MIN) < large.exponent - padding as i64 {
            let padded = large.magnitude.shifted_left(padding);
            let magnitude = if large.negative == small.negative { padded } else { padded.minus(&1u64.to_limbs()).unwrap_or_default() };
            return Self::rounded(large.negative, &magnitude, large.exponent - padding as i64, true, precision, rounding);
        }
        let exponent = large.exponent.min(small.exponent);
        let wide = large.magnitude.shifted_left((large.exponent - exponent) as u64);
        let narrow = small.magnitude.shifted_left((small.exponent - exponent) as u64);
        if large.negative == small.negative {
            return Self::rounded(large.negative, &wide.plus(&narrow), exponent, false, precision, rounding);
        }
        match wide.minus(&narrow) {
            Some(difference) => Self::rounded(large.negative, &difference, exponent, false, precision, rounding),
            None => Self::rounded(small.negative, &narrow.minus(&wide).unwrap_or_default(), exponent, false, precision, rounding),
        }
    }

    /// `self - other` rounded to `precision` bits in the given direction.
    pub fn subtract(&self, other: &Self, precision: u32, rounding: Rounding) -> Self {
        self.add(&other.negated(), precision, rounding)
    }

    /// `self * other` rounded to `precision` bits in the given direction.
    pub fn multiply(&self, other: &Self, precision: u32, rounding: Rounding) -> Self {
        if self.is_zero() || other.is_zero() {
            return Self::zero();
        }
        Self::rounded(self.negative != other.negative, &self.magnitude.times(&other.magnitude), self.exponent + other.exponent, false, precision, rounding)
    }

    /// `self / other` rounded to `precision` bits in the given direction, `None` for a zero divisor.
    pub fn divide(&self, other: &Self, precision: u32, rounding: Rounding) -> Option<Self> {
        if other.is_zero() {
            return None;
        }
        if self.is_zero() {
            return Some(Self::zero());
        }
        let wanted = precision as u64 + 3 + other.magnitude.bit_length();
        let shift = wanted.saturating_sub(self.magnitude.bit_length());
        let (quotient, remainder) = self.magnitude.shifted_left(shift).divide_with_remainder(&other.magnitude)?;
        Some(Self::rounded(self.negative != other.negative, &quotient, self.exponent - other.exponent - shift as i64, !remainder.is_empty(), precision, rounding))
    }

    /// The square root rounded to `precision` bits in the given direction, `None` below zero.
    pub fn square_root(&self, precision: u32, rounding: Rounding) -> Option<Self> {
        if self.negative {
            return None;
        }
        if self.is_zero() {
            return Some(Self::zero());
        }
        let wanted = 2 * (precision as u64 + 3);
        let mut shift = wanted.saturating_sub(self.magnitude.bit_length());
        if (self.exponent - shift as i64) % 2 != 0 {
            shift += 1;
        }
        let (root, exact) = self.magnitude.shifted_left(shift).integer_square_root();
        Some(Self::rounded(false, &root, (self.exponent - shift as i64) / 2, !exact, precision, rounding))
    }
}

/// A closed interval `[lower, upper]` of [`MultiPrecision`] values that contains the exact value
/// of the quantity being evaluated; every operation rounds its bounds outward.
#[derive(Clone, Debug)]
pub struct Interval {
    /// The lower bound.
    lower: MultiPrecision,
    /// The upper bound.
    upper: MultiPrecision,
    /// The working precision of results, in bits.
    precision: u32,
}

impl Interval {
    /// The interval `[lower, upper]` carried at `precision` bits.
    pub fn new(lower: MultiPrecision, upper: MultiPrecision, precision: u32) -> Self {
        Interval { lower, upper, precision }
    }

    /// The degenerate interval holding one exact value.
    pub fn point(value: MultiPrecision, precision: u32) -> Self {
        Interval { lower: value.clone(), upper: value, precision }
    }

    /// The interval of an exact integer.
    pub fn integer(negative: bool, value: u64, precision: u32) -> Self {
        Self::point(MultiPrecision::from_integer(negative, value), precision)
    }

    /// The lower bound.
    pub fn lower(&self) -> &MultiPrecision {
        &self.lower
    }

    /// The upper bound.
    pub fn upper(&self) -> &MultiPrecision {
        &self.upper
    }

    /// The working precision in bits.
    pub fn precision(&self) -> u32 {
        self.precision
    }

    /// The same bounds carried at another working precision.
    pub fn with_precision(&self, precision: u32) -> Self {
        Interval { lower: self.lower.clone(), upper: self.upper.clone(), precision }
    }

    /// Whether the exact value `value` lies inside.
    pub fn contains(&self, value: &MultiPrecision) -> bool {
        self.lower.compare(value) != Ordering::Greater && self.upper.compare(value) != Ordering::Less
    }

    /// Whether zero lies inside.
    pub fn contains_zero(&self) -> bool {
        self.lower.compare(&MultiPrecision::zero()) != Ordering::Greater && self.upper.compare(&MultiPrecision::zero()) != Ordering::Less
    }

    /// Whether every value is below zero.
    pub fn is_negative(&self) -> bool {
        self.upper.is_negative()
    }

    /// Whether every value is above zero.
    pub fn is_positive(&self) -> bool {
        !self.lower.is_negative() && !self.lower.is_zero()
    }

    /// `-self`.
    pub fn negated(&self) -> Self {
        Interval { lower: self.upper.negated(), upper: self.lower.negated(), precision: self.precision }
    }

    /// `self * 2^amount`, exact.
    pub fn scaled(&self, amount: i64) -> Self {
        Interval { lower: self.lower.scaled(amount), upper: self.upper.scaled(amount), precision: self.precision }
    }

    /// `self + other`.
    pub fn add(&self, other: &Self) -> Self {
        Interval { lower: self.lower.add(&other.lower, self.precision, Rounding::Down), upper: self.upper.add(&other.upper, self.precision, Rounding::Up), precision: self.precision }
    }

    /// `self - other`.
    pub fn subtract(&self, other: &Self) -> Self {
        self.add(&other.negated())
    }

    /// `self * other`: the extremes of the four products, each bound rounded outward.
    pub fn multiply(&self, other: &Self) -> Self {
        let bounds = [(&self.lower, &other.lower), (&self.lower, &other.upper), (&self.upper, &other.lower), (&self.upper, &other.upper)];
        let lowest = bounds.iter().map(|(left, right)| left.multiply(right, self.precision, Rounding::Down)).reduce(|best, next| if next.compare(&best) == Ordering::Less { next } else { best });
        let highest = bounds.iter().map(|(left, right)| left.multiply(right, self.precision, Rounding::Up)).reduce(|best, next| if next.compare(&best) == Ordering::Greater { next } else { best });
        Interval { lower: lowest.unwrap_or_else(MultiPrecision::zero), upper: highest.unwrap_or_else(MultiPrecision::zero), precision: self.precision }
    }

    /// `self / other`, `None` when the divisor interval contains zero.
    pub fn divide(&self, other: &Self) -> Option<Self> {
        if other.contains_zero() {
            return None;
        }
        let mut lowest: Option<MultiPrecision> = None;
        let mut highest: Option<MultiPrecision> = None;
        for (left, right) in [(&self.lower, &other.lower), (&self.lower, &other.upper), (&self.upper, &other.lower), (&self.upper, &other.upper)] {
            let down = left.divide(right, self.precision, Rounding::Down)?;
            let up = left.divide(right, self.precision, Rounding::Up)?;
            lowest = Some(match lowest { Some(best) if best.compare(&down) != Ordering::Greater => best, _ => down });
            highest = Some(match highest { Some(best) if best.compare(&up) != Ordering::Less => best, _ => up });
        }
        Some(Interval { lower: lowest?, upper: highest?, precision: self.precision })
    }

    /// The square root, `None` when the interval reaches below zero.
    pub fn square_root(&self) -> Option<Self> {
        Some(Interval { lower: self.lower.square_root(self.precision, Rounding::Down)?, upper: self.upper.square_root(self.precision, Rounding::Up)?, precision: self.precision })
    }

    /// `|self|`.
    pub fn absolute(&self) -> Self {
        if self.is_positive() || self.lower.is_zero() {
            return self.clone();
        }
        if self.is_negative() {
            return self.negated();
        }
        let upper = if self.upper.compare(&self.lower.negated()) == Ordering::Less { self.lower.negated() } else { self.upper.clone() };
        Interval { lower: MultiPrecision::zero(), upper, precision: self.precision }
    }

    /// The bounds rounded to the nearest `f128` each: if they are equal, every value between
    /// them rounds to that pattern too (rounding to nearest is monotonic).
    pub fn round_to_quad(&self) -> (u128, u128) {
        (self.lower.to_quad_bits(), self.upper.to_quad_bits())
    }
}
