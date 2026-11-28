//! The values of `ExactRational` that conversions need: the exact value of a pattern, an integer,
//! the zero test and the integer part.

use super::exact::ExactRational;
use num_bigint::{BigInt, BigUint, Sign};
use piperine_numbers::format::BinaryFormat;

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

    /// Whether the value is zero.
    pub fn is_zero(&self) -> bool {
        self.numerator.sign() == Sign::NoSign
    }

    /// The integer part toward zero.
    pub fn truncated(&self) -> BigInt {
        &self.numerator / BigInt::from(self.denominator.clone())
    }
}
