//! `ExactOracle`: the IEEE 754 operations of one format computed from `ExactRational` for finite
//! operands and from the standard's special-value tables otherwise. The specification is H.2 and
//! IEEE 754-2019 clauses 5 and 6; the code shares nothing with `IeeeArithmetic`.

use super::exact::ExactRational;
use num_bigint::BigInt;
use piperine_numbers::format::BinaryFormat;
use std::cmp::Ordering;

/// The reference operations on bit patterns of `format`.
pub struct ExactOracle {
    /// The format of every pattern.
    pub format: BinaryFormat,
}

impl ExactOracle {
    /// An oracle for `format`.
    pub fn new(format: BinaryFormat) -> Self {
        ExactOracle { format }
    }

    /// The canonical NaN.
    pub fn nan(&self) -> u128 {
        self.format.quiet_nan()
    }

    /// Whether the pattern is a NaN.
    pub fn is_nan(&self, pattern: u128) -> bool {
        ExactRational::from_pattern(self.format, pattern).is_none() && !self.is_infinite(pattern)
    }

    /// Whether the pattern is an infinity.
    pub fn is_infinite(&self, pattern: u128) -> bool {
        self.format.infinity(false) == pattern & !self.format.sign_bit()
    }

    /// The sign bit as a boolean.
    pub fn negative(&self, pattern: u128) -> bool {
        pattern & self.format.sign_bit() != 0
    }

    /// The pattern of a zero of the given sign.
    pub fn zero(&self, negative: bool) -> u128 {
        if negative { self.format.sign_bit() } else { 0 }
    }

    /// The exact value of a finite pattern.
    pub fn value(&self, pattern: u128) -> Option<ExactRational> {
        ExactRational::from_pattern(self.format, pattern)
    }

    /// The pattern of an exact result, with a given sign for an exact zero.
    pub fn result(&self, exact: &ExactRational, zero_negative: bool) -> u128 {
        if exact.is_zero() { self.zero(zero_negative) } else { exact.nearest(self.format) }
    }

    /// `left + right`.
    pub fn add(&self, left: u128, right: u128) -> u128 {
        if self.is_nan(left) || self.is_nan(right) {
            return self.nan();
        }
        match (self.is_infinite(left), self.is_infinite(right)) {
            (true, true) => return if self.negative(left) == self.negative(right) { left } else { self.nan() },
            (true, false) => return left,
            (false, true) => return right,
            (false, false) => {}
        }
        let (a, b) = (self.value(left).unwrap_or(ExactRational::integer(&BigInt::from(0))), self.value(right).unwrap_or(ExactRational::integer(&BigInt::from(0))));
        let both_negative_zero = a.is_zero() && b.is_zero() && self.negative(left) && self.negative(right);
        self.result(&a.plus(&b), both_negative_zero)
    }

    /// `left - right`.
    pub fn subtract(&self, left: u128, right: u128) -> u128 {
        self.add(left, right ^ self.format.sign_bit())
    }

    /// `left * right`.
    pub fn multiply(&self, left: u128, right: u128) -> u128 {
        let negative = self.negative(left) != self.negative(right);
        let (a, b) = (self.value(left), self.value(right));
        if self.is_nan(left) || self.is_nan(right) {
            return self.nan();
        }
        match (a, b) {
            (Some(a), Some(b)) => self.result(&a.times(&b), negative),
            (None, None) => self.format.infinity(negative),
            (None, Some(b)) | (Some(b), None) => if b.is_zero() { self.nan() } else { self.format.infinity(negative) },
        }
    }

    /// `left / right`.
    pub fn divide(&self, left: u128, right: u128) -> u128 {
        let negative = self.negative(left) != self.negative(right);
        if self.is_nan(left) || self.is_nan(right) {
            return self.nan();
        }
        match (self.value(left), self.value(right)) {
            (Some(a), Some(b)) => match a.divided_by(&b) {
                Some(quotient) => self.result(&quotient, negative),
                None if a.is_zero() => self.nan(),
                None => self.format.infinity(negative),
            },
            (None, None) => self.nan(),
            (None, Some(_)) => self.format.infinity(negative),
            (Some(_), None) => self.zero(negative),
        }
    }

    /// The square root.
    pub fn square_root(&self, pattern: u128) -> u128 {
        if self.is_nan(pattern) {
            return pattern;
        }
        match self.value(pattern) {
            None => if self.negative(pattern) { self.nan() } else { pattern },
            Some(value) if value.is_zero() => pattern,
            Some(value) if value.is_negative() => self.nan(),
            Some(value) => value.square_root_nearest(self.format),
        }
    }

    /// `sqrt(left^2 + right^2)`.
    pub fn hypot(&self, left: u128, right: u128) -> u128 {
        if self.is_infinite(left) || self.is_infinite(right) {
            return self.format.infinity(false);
        }
        match (self.value(left), self.value(right)) {
            (Some(a), Some(b)) => a.times(&a).plus(&b.times(&b)).square_root_nearest(self.format),
            _ => self.nan(),
        }
    }

    /// The Euclidean remainder (H.2 rule 9) with the decisions of `IeeeArithmetic::remainder_euclid`.
    pub fn remainder_euclid(&self, left: u128, right: u128) -> u128 {
        if self.is_nan(left) || self.is_nan(right) || self.is_infinite(left) {
            return self.nan();
        }
        if self.is_infinite(right) {
            return if self.negative(left) && self.value(left).is_some_and(|x| !x.is_zero()) { self.format.infinity(false) } else { left };
        }
        let (Some(x), Some(d)) = (self.value(left), self.value(right)) else { return self.nan() };
        if d.is_zero() {
            return self.nan();
        }
        let remainder = x.remainder_euclid(&d);
        self.result(&remainder, self.negative(left))
    }

    /// The pattern of an integer-valued result keeping the sign of `source` for a zero.
    fn integral(&self, integer: &BigInt, source: u128) -> u128 {
        self.result(&ExactRational::integer(integer), self.negative(source))
    }

    /// `floor`.
    pub fn floor(&self, pattern: u128) -> u128 {
        self.value(pattern).map_or(pattern, |value| self.integral(&value.floor(), pattern))
    }

    /// `ceil`.
    pub fn ceiling(&self, pattern: u128) -> u128 {
        self.value(pattern).map_or(pattern, |value| self.integral(&value.ceiling(), pattern))
    }

    /// `trunc`.
    pub fn truncate(&self, pattern: u128) -> u128 {
        self.value(pattern).map_or(pattern, |value| self.integral(&value.truncated(), pattern))
    }

    /// `round`, ties away from zero.
    pub fn round_half_away(&self, pattern: u128) -> u128 {
        self.value(pattern).map_or(pattern, |value| self.integral(&value.round_half_away(), pattern))
    }

    /// `fract` = `x - trunc(x)`: `+0` for an integer, NaN for infinity.
    pub fn fraction(&self, pattern: u128) -> u128 {
        if self.is_nan(pattern) || self.is_infinite(pattern) {
            return self.nan();
        }
        let value = self.value(pattern).unwrap_or(ExactRational::integer(&BigInt::from(0)));
        self.result(&value.minus(&ExactRational::integer(&value.truncated())), false)
    }

    /// `signum`.
    pub fn signum(&self, pattern: u128) -> u128 {
        if self.is_nan(pattern) {
            return self.nan();
        }
        self.result(&ExactRational::integer(&BigInt::from(if self.negative(pattern) { -1 } else { 1 })), false)
    }

    /// The exact order of two non-NaN patterns, zeros equal.
    fn order(&self, left: u128, right: u128) -> Ordering {
        let value = |pattern: u128| self.value(pattern);
        match (value(left), value(right)) {
            (Some(a), Some(b)) => a.compare(&b),
            (None, Some(_)) => if self.negative(left) { Ordering::Less } else { Ordering::Greater },
            (Some(_), None) => if self.negative(right) { Ordering::Greater } else { Ordering::Less },
            (None, None) => (!self.negative(left)).cmp(&!self.negative(right)),
        }
    }

    /// The lesser value (`-0` below `+0`); a NaN operand yields the other.
    pub fn minimum(&self, left: u128, right: u128) -> u128 {
        self.extreme(left, right, true)
    }

    /// The greater value (`+0` above `-0`); a NaN operand yields the other.
    pub fn maximum(&self, left: u128, right: u128) -> u128 {
        self.extreme(left, right, false)
    }

    /// The minimum or the maximum under H.2 rule 3.
    fn extreme(&self, left: u128, right: u128, lesser: bool) -> u128 {
        if self.is_nan(left) && self.is_nan(right) {
            return self.nan();
        }
        if self.is_nan(left) {
            return right;
        }
        if self.is_nan(right) {
            return left;
        }
        match self.order(left, right) {
            Ordering::Equal => if self.negative(left) == lesser { left } else { right },
            Ordering::Less => if lesser { left } else { right },
            Ordering::Greater => if lesser { right } else { left },
        }
    }

    /// `clamp(lo, hi)` (H.2 rule 4).
    pub fn clamp(&self, pattern: u128, lowest: u128, highest: u128) -> Option<u128> {
        if self.is_nan(lowest) || self.is_nan(highest) || self.order(lowest, highest) == Ordering::Greater {
            return None;
        }
        if self.is_nan(pattern) {
            return Some(pattern);
        }
        Some(if self.order(pattern, lowest) == Ordering::Less { lowest } else if self.order(pattern, highest) == Ordering::Greater { highest } else { pattern })
    }

    /// The order of two patterns as IEEE 754 comparison: `None` when unordered.
    pub fn compare(&self, left: u128, right: u128) -> Option<Ordering> {
        (!self.is_nan(left) && !self.is_nan(right)).then(|| self.order(left, right))
    }
}
