//! Exact arithmetic on `ExactRational`: sums, products, quotients, integer parts, the Euclidean
//! remainder and the correctly rounded square root, for the targets that need an oracle for a
//! whole operation (`ExactOracle`).

use super::exact::ExactRational;
use num_bigint::{BigInt, BigUint, Sign};
use piperine_numbers::format::BinaryFormat;
use std::cmp::Ordering;

impl ExactRational {
    /// The exact value of a finite pattern of `format` (zero included), `None` for infinity and NaN.
    pub fn from_pattern(format: BinaryFormat, pattern: u128) -> Option<Self> {
        let fraction_bits = format.fraction_bits();
        let field = (pattern >> fraction_bits) & ((1u128 << (format.bits - format.precision)) - 1);
        let fraction = pattern & ((1u128 << fraction_bits) - 1);
        let negative = pattern >> (format.bits - 1) == 1;
        if field == (1u128 << (format.bits - format.precision)) - 1 {
            return None;
        }
        let minimum_unit = format.minimum_exponent() as i64 - fraction_bits as i64;
        let (significand, exponent) = if field == 0 { (fraction, minimum_unit) } else { (fraction | 1u128 << fraction_bits, field as i64 - format.maximum_exponent as i64 - fraction_bits as i64) };
        Some(Self::dyadic(negative, &BigUint::from(significand), exponent))
    }

    /// The integer `value`.
    pub fn integer(value: &BigInt) -> Self {
        ExactRational { numerator: value.clone(), denominator: BigUint::from(1u32) }
    }

    /// `self + other`.
    pub fn plus(&self, other: &Self) -> Self {
        let denominator = &self.denominator * &other.denominator;
        let numerator = &self.numerator * BigInt::from(other.denominator.clone()) + &other.numerator * BigInt::from(self.denominator.clone());
        ExactRational { numerator, denominator }
    }

    /// `-self`.
    pub fn negated(&self) -> Self {
        ExactRational { numerator: -self.numerator.clone(), denominator: self.denominator.clone() }
    }

    /// `self - other`.
    pub fn minus(&self, other: &Self) -> Self {
        self.plus(&other.negated())
    }

    /// `self * other`.
    pub fn times(&self, other: &Self) -> Self {
        ExactRational { numerator: &self.numerator * &other.numerator, denominator: &self.denominator * &other.denominator }
    }

    /// `self / other`, `None` for a zero divisor.
    pub fn divided_by(&self, other: &Self) -> Option<Self> {
        if other.numerator.sign() == Sign::NoSign {
            return None;
        }
        let sign = if other.is_negative() { BigInt::from(-1) } else { BigInt::from(1) };
        Some(ExactRational { numerator: &self.numerator * BigInt::from(other.denominator.clone()) * sign, denominator: self.denominator.clone() * other.magnitude() })
    }

    /// Whether the value is zero.
    pub fn is_zero(&self) -> bool {
        self.numerator.sign() == Sign::NoSign
    }

    /// The exact order of two rationals.
    pub fn compare(&self, other: &Self) -> Ordering {
        (&self.numerator * BigInt::from(other.denominator.clone())).cmp(&(&other.numerator * BigInt::from(self.denominator.clone())))
    }

    /// The greatest integer not above the value.
    pub fn floor(&self) -> BigInt {
        let denominator = BigInt::from(self.denominator.clone());
        let quotient = &self.numerator / &denominator;
        if self.is_negative() && &quotient * &denominator != self.numerator { quotient - 1 } else { quotient }
    }

    /// The least integer not below the value.
    pub fn ceiling(&self) -> BigInt {
        -self.negated().floor()
    }

    /// The integer part toward zero.
    pub fn truncated(&self) -> BigInt {
        &self.numerator / BigInt::from(self.denominator.clone())
    }

    /// The nearest integer, a tie going away from zero.
    pub fn round_half_away(&self) -> BigInt {
        let half = ExactRational { numerator: BigInt::from(1), denominator: BigUint::from(2u32) };
        if self.is_negative() { self.minus(&half).ceiling() } else { self.plus(&half).floor() }
    }

    /// The Euclidean remainder `r` of `self` by `divisor`, `0 <= r < |divisor|` (`divisor` nonzero).
    pub fn remainder_euclid(&self, divisor: &Self) -> Self {
        let magnitude = ExactRational { numerator: BigInt::from(divisor.magnitude()), denominator: divisor.denominator.clone() };
        let quotient = self.divided_by(&magnitude).map(|ratio| ratio.floor()).unwrap_or_default();
        self.minus(&magnitude.times(&Self::integer(&quotient)))
    }

    /// The bits of the nearest value of `format` to the square root of `self` (`self >= 0`).
    pub fn square_root_nearest(&self, format: BinaryFormat) -> u128 {
        if self.is_zero() {
            return 0;
        }
        let top = self.binary_logarithm();
        let shift = (format.precision as i64 + 4 - top.div_euclid(2)).max(0) as u64;
        let scaled_numerator = self.magnitude() << (2 * shift);
        let floor_quotient = &scaled_numerator / &self.denominator;
        let root = floor_quotient.sqrt();
        let exact = &root * &root == floor_quotient && &floor_quotient * &self.denominator == scaled_numerator;
        if exact {
            Self::dyadic(false, &root, -(shift as i64)).nearest(format)
        } else {
            Self::dyadic(false, &(root * 2u32 + 1u32), -(shift as i64) - 1).nearest(format)
        }
    }
}
