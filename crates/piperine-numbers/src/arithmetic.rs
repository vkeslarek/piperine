//! `IeeeArithmetic`: exact IEEE 754 operations on the bit patterns of one `BinaryFormat`.
//!
//! Every result is computed exactly with limb arithmetic and rounded once by
//! [`BinaryFormat::round`], so it is the correctly rounded value for any format up to 128 bits.
//! `Float16` and `Float128` delegate their exact methods here.

use super::format::{BinaryFormat, FloatClass};
use super::limbs::{Limbs, ToLimbs};
use core::cmp::Ordering;

/// A finite nonzero value `(-1)^negative * significand * 2^exponent`.
#[derive(Clone, Copy)]
struct Finite {
    /// The sign.
    negative: bool,
    /// The integer significand (nonzero).
    significand: u128,
    /// The binary exponent of the significand's least bit.
    exponent: i64,
}

impl Finite {
    /// The exponent of the leading bit: `floor(log2 |value|)`.
    fn top(&self) -> i64 {
        self.exponent + 127 - self.significand.leading_zeros() as i64
    }
}

/// Exact operations on the bit patterns of one format.
#[derive(Clone, Copy)]
pub struct IeeeArithmetic {
    /// The format whose patterns these are.
    pub format: BinaryFormat,
}

impl IeeeArithmetic {
    /// Operations of `format`.
    pub fn new(format: BinaryFormat) -> Self {
        IeeeArithmetic { format }
    }

    /// The canonical NaN pattern.
    pub fn nan(&self) -> u128 {
        self.format.quiet_nan()
    }

    /// The pattern of a zero of the given sign.
    pub fn zero(&self, negative: bool) -> u128 {
        if negative { self.format.sign_bit() } else { 0 }
    }

    /// Whether the pattern is a NaN.
    pub fn is_nan(&self, pattern: u128) -> bool {
        self.format.classify(pattern) == FloatClass::NaN
    }

    /// Whether the pattern is an infinity.
    pub fn is_infinite(&self, pattern: u128) -> bool {
        self.format.classify(pattern) == FloatClass::Infinite
    }

    /// Whether the pattern is neither infinite nor NaN.
    pub fn is_finite(&self, pattern: u128) -> bool {
        !matches!(self.format.classify(pattern), FloatClass::Infinite | FloatClass::NaN)
    }

    /// Whether the pattern is a zero.
    pub fn is_zero(&self, pattern: u128) -> bool {
        self.format.classify(pattern) == FloatClass::Zero
    }

    /// The finite nonzero value of a pattern, `None` for zero, infinity and NaN.
    fn finite(&self, pattern: u128) -> Option<Finite> {
        let (negative, significand, exponent) = self.format.decode(pattern)?;
        (significand != 0).then_some(Finite { negative, significand, exponent })
    }

    /// `-x`.
    pub fn negate(&self, pattern: u128) -> u128 {
        pattern ^ self.format.sign_bit()
    }

    /// `|x|`.
    pub fn absolute(&self, pattern: u128) -> u128 {
        pattern & !self.format.sign_bit()
    }

    /// IEEE 754 comparison: `None` when either side is NaN, and zeros of either sign are equal.
    pub fn compare(&self, left: u128, right: u128) -> Option<Ordering> {
        if self.is_nan(left) || self.is_nan(right) {
            return None;
        }
        if self.is_zero(left) && self.is_zero(right) {
            return Some(Ordering::Equal);
        }
        let (left_negative, right_negative) = (self.format.is_negative(left), self.format.is_negative(right));
        let magnitudes = self.absolute(left).cmp(&self.absolute(right));
        Some(match (left_negative, right_negative) {
            (false, true) => Ordering::Greater,
            (true, false) => Ordering::Less,
            (false, false) => magnitudes,
            (true, true) => magnitudes.reverse(),
        })
    }

    /// `left + right`, correctly rounded.
    pub fn add(&self, left: u128, right: u128) -> u128 {
        if self.is_nan(left) || self.is_nan(right) {
            return self.nan();
        }
        match (self.is_infinite(left), self.is_infinite(right)) {
            (true, true) => return if self.format.is_negative(left) == self.format.is_negative(right) { left } else { self.nan() },
            (true, false) => return left,
            (false, true) => return right,
            (false, false) => {}
        }
        match (self.finite(left), self.finite(right)) {
            (None, None) => self.zero(self.format.is_negative(left) && self.format.is_negative(right)),
            (Some(_), None) => left,
            (None, Some(_)) => right,
            (Some(a), Some(b)) => self.add_finite(a, b),
        }
    }

    /// The exact sum of two finite nonzero values, rounded.
    fn add_finite(&self, first: Finite, second: Finite) -> u128 {
        let (larger, smaller) = if first.top() >= second.top() { (first, second) } else { (second, first) };
        if larger.top() - smaller.top() > self.format.precision as i64 + 3 {
            // The smaller operand is under a quarter of a unit in the last place of the larger one.
            return self.format.round(larger.negative, &larger.significand.to_limbs(), larger.exponent, false);
        }
        let exponent = larger.exponent.min(smaller.exponent);
        let wide = larger.significand.to_limbs().shifted_left((larger.exponent - exponent) as u64);
        let narrow = smaller.significand.to_limbs().shifted_left((smaller.exponent - exponent) as u64);
        if larger.negative == smaller.negative {
            return self.format.round(larger.negative, &wide.plus(&narrow), exponent, false);
        }
        match wide.minus(&narrow) {
            Some(difference) if difference.is_empty() => 0,
            Some(difference) => self.format.round(larger.negative, &difference, exponent, false),
            None => self.format.round(smaller.negative, &narrow.minus(&wide).unwrap_or_default(), exponent, false),
        }
    }

    /// `left - right`, correctly rounded.
    pub fn subtract(&self, left: u128, right: u128) -> u128 {
        self.add(left, self.negate(right))
    }

    /// `left * right`, correctly rounded.
    pub fn multiply(&self, left: u128, right: u128) -> u128 {
        let negative = self.format.is_negative(left) != self.format.is_negative(right);
        if self.is_nan(left) || self.is_nan(right) {
            return self.nan();
        }
        let infinite = self.is_infinite(left) || self.is_infinite(right);
        let zero = self.is_zero(left) || self.is_zero(right);
        if infinite && zero {
            return self.nan();
        }
        if infinite {
            return self.format.infinity(negative);
        }
        match (self.finite(left), self.finite(right)) {
            (Some(a), Some(b)) => self.format.round(negative, &a.significand.to_limbs().times(&b.significand.to_limbs()), a.exponent + b.exponent, false),
            _ => self.zero(negative),
        }
    }

    /// `left / right`, correctly rounded.
    pub fn divide(&self, left: u128, right: u128) -> u128 {
        let negative = self.format.is_negative(left) != self.format.is_negative(right);
        if self.is_nan(left) || self.is_nan(right) || (self.is_infinite(left) && self.is_infinite(right)) || (self.is_zero(left) && self.is_zero(right)) {
            return self.nan();
        }
        if self.is_infinite(left) || self.is_zero(right) {
            return self.format.infinity(negative);
        }
        if self.is_infinite(right) || self.is_zero(left) {
            return self.zero(negative);
        }
        let (Some(a), Some(b)) = (self.finite(left), self.finite(right)) else { return self.nan() };
        let shift = 2 * self.format.precision as u64 + 4;
        let numerator = a.significand.to_limbs().shifted_left(shift);
        let (quotient, remainder) = numerator.divide_with_remainder(&b.significand.to_limbs()).unwrap_or_default();
        self.format.round(negative, &quotient, a.exponent - b.exponent - shift as i64, !remainder.is_empty())
    }

    /// The square root, correctly rounded (`-0` for `-0`, NaN below zero).
    pub fn square_root(&self, pattern: u128) -> u128 {
        if self.is_nan(pattern) || self.is_zero(pattern) {
            return pattern;
        }
        if self.format.is_negative(pattern) {
            return self.nan();
        }
        let Some(value) = self.finite(pattern) else { return pattern };
        let bits = 128 - value.significand.leading_zeros() as i64;
        let wanted = 2 * (self.format.precision as i64 + 3);
        let mut shift = (wanted - bits).max(0);
        if (value.exponent - shift) % 2 != 0 {
            shift += 1;
        }
        let (root, exact) = value.significand.to_limbs().shifted_left(shift as u64).integer_square_root();
        self.format.round(false, &root, (value.exponent - shift) / 2, !exact)
    }

    /// `sqrt(x^2 + y^2)` without overflow in the squares, correctly rounded.
    pub fn hypot(&self, left: u128, right: u128) -> u128 {
        if self.is_infinite(left) || self.is_infinite(right) {
            return self.format.infinity(false);
        }
        if self.is_nan(left) || self.is_nan(right) {
            return self.nan();
        }
        let (Some(a), Some(b)) = (self.finite(left), self.finite(right)) else {
            return self.absolute(if self.is_zero(left) { right } else { left });
        };
        let (larger, smaller) = if a.top() >= b.top() { (a, b) } else { (b, a) };
        if larger.top() - smaller.top() > self.format.precision as i64 + 5 {
            return self.format.round(false, &larger.significand.to_limbs(), larger.exponent, false);
        }
        let exponent = 2 * larger.exponent.min(smaller.exponent);
        let square = |value: &Finite| value.significand.to_limbs().times(&value.significand.to_limbs()).shifted_left((2 * value.exponent - exponent) as u64);
        let sum = square(&larger).plus(&square(&smaller));
        let bits = sum.bit_length() as i64;
        let mut shift = (2 * (self.format.precision as i64 + 3) - bits).max(0);
        if (exponent - shift) % 2 != 0 {
            shift += 1;
        }
        let (root, exact) = sum.shifted_left(shift as u64).integer_square_root();
        self.format.round(false, &root, (exponent - shift) / 2, !exact)
    }

    /// The Euclidean remainder `r = x - d*q` with `0 <= r < |d|` (H.2 rule 9), rounded once.
    /// A zero `d`, an infinite or NaN `x` gives NaN; an infinite `d` leaves a non-negative `x`
    /// and gives `+inf` for a negative one, as Rust's `rem_euclid` does.
    pub fn remainder_euclid(&self, left: u128, right: u128) -> u128 {
        if self.is_nan(left) || self.is_nan(right) || self.is_infinite(left) || self.is_zero(right) {
            return self.nan();
        }
        if self.is_infinite(right) {
            return if self.format.is_negative(left) && !self.is_zero(left) { self.format.infinity(false) } else { left };
        }
        let (Some(x), Some(d)) = (self.finite(left), self.finite(right)) else { return left };
        let exponent = x.exponent.min(d.exponent);
        let dividend = x.significand.to_limbs().shifted_left((x.exponent - exponent) as u64);
        let divisor = d.significand.to_limbs().shifted_left((d.exponent - exponent) as u64);
        let remainder = dividend.divide_with_remainder(&divisor).map(|(_, remainder)| remainder).unwrap_or_default();
        if remainder.is_empty() {
            return self.zero(x.negative);
        }
        let magnitude = if x.negative { divisor.minus(&remainder).unwrap_or_default() } else { remainder };
        self.format.round(false, &magnitude, exponent, false)
    }

    /// How the integer part of a value is taken.
    fn integer_part(&self, pattern: u128, direction: IntegerDirection) -> u128 {
        let Some(value) = self.finite(pattern) else { return pattern };
        if value.exponent >= 0 {
            return pattern;
        }
        let drop = (-value.exponent) as u64;
        let significand = value.significand.to_limbs();
        let mut whole = significand.shifted_right(drop);
        let fraction = significand.any_bit_below(drop);
        let half = significand.bit(drop - 1);
        let round_up = match direction {
            IntegerDirection::Truncate => false,
            IntegerDirection::Floor => value.negative && fraction,
            IntegerDirection::Ceiling => !value.negative && fraction,
            IntegerDirection::NearestAway => half,
        };
        if round_up {
            whole = whole.plus(&1u64.to_limbs());
        }
        self.format.round(value.negative, &whole, 0, false)
    }

    /// The greatest integer not above the value.
    pub fn floor(&self, pattern: u128) -> u128 {
        self.integer_part(pattern, IntegerDirection::Floor)
    }

    /// The least integer not below the value.
    pub fn ceiling(&self, pattern: u128) -> u128 {
        self.integer_part(pattern, IntegerDirection::Ceiling)
    }

    /// The integer part toward zero.
    pub fn truncate(&self, pattern: u128) -> u128 {
        self.integer_part(pattern, IntegerDirection::Truncate)
    }

    /// The nearest integer, a tie going away from zero.
    pub fn round_half_away(&self, pattern: u128) -> u128 {
        self.integer_part(pattern, IntegerDirection::NearestAway)
    }

    /// `x - trunc(x)`: exact, `+0` for an integer, NaN for an infinity.
    pub fn fraction(&self, pattern: u128) -> u128 {
        self.subtract(pattern, self.truncate(pattern))
    }

    /// `1`, `-1` by the sign bit (zeros and infinities included), NaN for NaN.
    pub fn signum(&self, pattern: u128) -> u128 {
        if self.is_nan(pattern) {
            return self.nan();
        }
        self.format.round(self.format.is_negative(pattern), &1u64.to_limbs(), 0, false)
    }

    /// The lesser value; a NaN operand yields the other; `-0` is below `+0` (H.2 rule 3).
    pub fn minimum(&self, left: u128, right: u128) -> u128 {
        self.extreme(left, right, Ordering::Less)
    }

    /// The greater value; a NaN operand yields the other; `+0` is above `-0` (H.2 rule 3).
    pub fn maximum(&self, left: u128, right: u128) -> u128 {
        self.extreme(left, right, Ordering::Greater)
    }

    /// The operand that wins `wanted` (`Less` for the minimum), the other for a NaN.
    fn extreme(&self, left: u128, right: u128, wanted: Ordering) -> u128 {
        match (self.is_nan(left), self.is_nan(right)) {
            (true, true) => return self.nan(),
            (true, false) => return right,
            (false, true) => return left,
            (false, false) => {}
        }
        match self.compare(left, right) {
            Some(Ordering::Equal) => {
                let negative_wins = wanted == Ordering::Less;
                if self.format.is_negative(left) == negative_wins { left } else { right }
            }
            Some(order) if order == wanted => left,
            _ => right,
        }
    }

    /// `clamp(lo, hi)` (H.2 rule 4): `None` when `lo > hi` or either bound is NaN; NaN stays NaN.
    pub fn clamp(&self, pattern: u128, lowest: u128, highest: u128) -> Option<u128> {
        if self.compare(lowest, highest)? == Ordering::Greater {
            return None;
        }
        if self.is_nan(pattern) {
            return Some(pattern);
        }
        Some(match self.compare(pattern, lowest) {
            Some(Ordering::Less) => lowest,
            _ if self.compare(pattern, highest) == Some(Ordering::Greater) => highest,
            _ => pattern,
        })
    }
}

/// Which integer an integer-part operation selects.
#[derive(Clone, Copy)]
enum IntegerDirection {
    /// Toward zero.
    Truncate,
    /// Toward negative infinity.
    Floor,
    /// Toward positive infinity.
    Ceiling,
    /// To nearest, ties away from zero.
    NearestAway,
}
