//! Decimal and scaled views of `ExactRational`, for the targets that read text or feed the
//! library's `round` an integer with a sticky bit.

use super::exact::ExactRational;
use num_bigint::{BigInt, BigUint, Sign};
use piperine_numbers::format::BinaryFormat;

impl ExactRational {
    /// `(-1)^negative * digits * 10^power`.
    pub fn decimal(negative: bool, digits: &BigUint, power: i64) -> Self {
        let sign = if negative { Sign::Minus } else { Sign::Plus };
        let ten_power = BigUint::from(10u32).pow(power.unsigned_abs() as u32);
        if power >= 0 {
            ExactRational { numerator: BigInt::from_biguint(sign, digits * ten_power), denominator: BigUint::from(1u32) }
        } else {
            ExactRational { numerator: BigInt::from_biguint(sign, digits.clone()), denominator: ten_power }
        }
    }

    /// `floor(|self| * 2^shift)` as limbs and whether a remainder was dropped.
    pub fn scaled(&self, shift: u32) -> (Vec<u64>, bool) {
        let scaled = self.magnitude() << shift;
        let quotient = &scaled / &self.denominator;
        let remainder = &scaled % &self.denominator;
        (quotient.to_u64_digits(), remainder != BigUint::from(0u32))
    }

    /// The exact value of a finite pattern (zero included), `None` for infinity and NaN.
    pub fn from_pattern_for_text(format: BinaryFormat, pattern: u128) -> Option<Self> {
        let fraction_bits = format.fraction_bits();
        let field_mask = (1u128 << (format.bits - format.precision)) - 1;
        let field = (pattern >> fraction_bits) & field_mask;
        if field == field_mask {
            return None;
        }
        let fraction = pattern & ((1u128 << fraction_bits) - 1);
        let negative = pattern >> (format.bits - 1) == 1;
        let (significand, exponent) = if field == 0 { (fraction, format.minimum_exponent() as i64 - fraction_bits as i64) } else { (fraction | 1u128 << fraction_bits, field as i64 - format.maximum_exponent as i64 - fraction_bits as i64) };
        Some(Self::dyadic(negative, &BigUint::from(significand), exponent))
    }

    /// Whether the value is zero.
    pub fn is_zero(&self) -> bool {
        self.numerator.sign() == Sign::NoSign
    }

    /// `floor(|self| / 10^power)` (a negative power multiplies).
    pub fn magnitude_over_power_of_ten(&self, power: i64) -> BigUint {
        let ten = BigUint::from(10u32).pow(power.unsigned_abs() as u32);
        if power >= 0 { self.magnitude() / (&self.denominator * ten) } else { self.magnitude() * ten / &self.denominator }
    }
}
