//! `ExactRational`: an exact fraction over `num-bigint`, and the reference rounding of a
//! fraction to a binary interchange format. It shares no code with the library under test: the
//! neighbours of a value are found by exact division and compared with the midpoint as fractions.

mod algebra;
mod integers;
mod nearest;
mod parts;
mod root;
mod scaled;
mod text;
mod values;

use num_bigint::{BigInt, BigUint, Sign};

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
}
