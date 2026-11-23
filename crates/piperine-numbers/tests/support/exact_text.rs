//! Decimal and scaled views of `ExactRational`, for the targets that read text or feed the
//! library's `round` an integer with a sticky bit.

use super::exact::ExactRational;
use num_bigint::{BigInt, BigUint, Sign};

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
}
