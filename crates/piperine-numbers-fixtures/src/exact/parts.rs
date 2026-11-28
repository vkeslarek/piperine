//! The other integer parts of `ExactRational`: the ceiling and rounding half away from zero.

use super::ExactRational;
use num_bigint::{BigInt, BigUint};

impl ExactRational {
    /// The least integer not below the value.
    pub fn ceiling(&self) -> BigInt {
        -self.negated().floor()
    }

    /// The nearest integer, a tie going away from zero.
    pub fn round_half_away(&self) -> BigInt {
        let half = ExactRational { numerator: BigInt::from(1), denominator: BigUint::from(2u32) };
        if self.is_negative() { self.minus(&half).ceiling() } else { self.plus(&half).floor() }
    }
}
