//! `ExactRational`: an exact fraction over `num-bigint`, and the reference rounding of a
//! fraction to a binary interchange format. It shares no code with the library under test: the
//! neighbours of a value are found by exact division and compared with the midpoint as fractions.

use num_bigint::{BigInt, BigUint, Sign};
use piperine_numbers::format::BinaryFormat;
use std::cmp::Ordering;

/// A rational number `numerator / denominator` with a positive denominator, not reduced.
#[derive(Clone, Debug)]
pub struct ExactRational {
    /// The signed numerator.
    pub numerator: BigInt,
    /// The positive denominator.
    pub denominator: BigUint,
}

impl ExactRational {
    /// `(-1)^negative * significand * 2^exponent`.
    pub fn dyadic(negative: bool, significand: &BigUint, exponent: i64) -> Self {
        let sign = if negative { Sign::Minus } else { Sign::Plus };
        if exponent >= 0 {
            ExactRational { numerator: BigInt::from_biguint(sign, significand << (exponent as u64)), denominator: BigUint::from(1u32) }
        } else {
            ExactRational { numerator: BigInt::from_biguint(sign, significand.clone()), denominator: BigUint::from(1u32) << ((-exponent) as u64) }
        }
    }

    /// The magnitude of the numerator.
    pub fn magnitude(&self) -> BigUint {
        self.numerator.magnitude().clone()
    }

    /// Whether the value is negative.
    pub fn is_negative(&self) -> bool {
        self.numerator.sign() == Sign::Minus
    }

    /// `floor(log2 |self|)` of a nonzero value.
    pub fn binary_logarithm(&self) -> i64 {
        let numerator = self.magnitude();
        let mut exponent = numerator.bits() as i64 - self.denominator.bits() as i64;
        let (left, right) = if exponent >= 0 { (numerator.clone(), &self.denominator << (exponent as u64)) } else { (&numerator << ((-exponent) as u64), self.denominator.clone()) };
        if left < right {
            exponent -= 1;
        }
        exponent
    }

    /// The bits of the value of `format` nearest to `self`, ties to even, with gradual underflow
    /// and overflow to infinity. Zero gives `+0`.
    pub fn nearest(&self, format: BinaryFormat) -> u128 {
        let sign = if self.is_negative() { format.sign_bit() } else { 0 };
        if self.numerator.sign() == Sign::NoSign {
            return 0;
        }
        let precision = format.precision as i64;
        let minimum = format.minimum_exponent() as i64;
        let unit_exponent = self.binary_logarithm().max(minimum) - (precision - 1);
        let (mut numerator, mut denominator) = (self.magnitude(), self.denominator.clone());
        if unit_exponent >= 0 {
            denominator <<= unit_exponent as u64;
        } else {
            numerator <<= (-unit_exponent) as u64;
        }
        let mut quotient = &numerator / &denominator;
        let remainder = &numerator % &denominator;
        let rounded_up = match (remainder * 2u32).cmp(&denominator) {
            Ordering::Greater => true,
            Ordering::Equal => quotient.bit(0),
            Ordering::Less => false,
        };
        if rounded_up {
            quotient += 1u32;
        }
        Self::compose(format, sign, quotient, unit_exponent)
    }

    /// The pattern of `quotient * 2^unit_exponent` for a quotient already rounded to the format.
    fn compose(format: BinaryFormat, sign: u128, quotient: BigUint, unit_exponent: i64) -> u128 {
        let mut quotient = quotient;
        let mut unit_exponent = unit_exponent;
        if quotient == BigUint::from(1u32) << format.precision {
            quotient >>= 1u32;
            unit_exponent += 1;
        }
        let significand = Self::low_bits(&quotient);
        let implicit = 1u128 << format.fraction_bits();
        if significand < implicit {
            return sign | significand;
        }
        let field = unit_exponent + format.fraction_bits() as i64 + format.maximum_exponent as i64;
        if field >= 2 * format.maximum_exponent as i64 + 1 {
            return sign | (2 * format.maximum_exponent as u128 + 1) << format.fraction_bits();
        }
        sign | (field as u128) << format.fraction_bits() | (significand - implicit)
    }

    /// The low 128 bits of a big integer.
    fn low_bits(value: &BigUint) -> u128 {
        let digits = value.to_u64_digits();
        digits.first().copied().unwrap_or(0) as u128 | (digits.get(1).copied().unwrap_or(0) as u128) << 64
    }
}
