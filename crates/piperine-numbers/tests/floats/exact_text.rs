//! The decimal constructor of `ExactRational`.

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
}
