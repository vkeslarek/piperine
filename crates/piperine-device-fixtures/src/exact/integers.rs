//! Integer parts of `ExactRational`: the floor and the Euclidean remainder.

use super::ExactRational;
use num_bigint::{BigInt, BigUint};

impl ExactRational {
    /// The greatest integer not above the value.
    pub fn floor(&self) -> BigInt {
        let denominator = BigInt::from(self.denominator.clone());
        let quotient = &self.numerator / &denominator;
        if self.is_negative() && &quotient * &denominator != self.numerator { quotient - 1 } else { quotient }
    }

    /// The Euclidean remainder `r` of `self` by `divisor`, `0 <= r < |divisor|` (`divisor` nonzero).
    pub fn remainder_euclid(&self, divisor: &Self) -> Self {
        let magnitude = ExactRational { numerator: BigInt::from(divisor.magnitude()), denominator: divisor.denominator.clone() };
        let quotient = self.divided_by(&magnitude).map(|ratio| ratio.floor()).unwrap_or_default();
        self.minus(&magnitude.times(&ExactRational { numerator: quotient, denominator: BigUint::from(1u32) }))
    }
}
