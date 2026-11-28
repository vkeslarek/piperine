//! The correctly rounded square root of an `ExactRational`.

use super::ExactRational;
use piperine_numbers::format::BinaryFormat;

impl ExactRational {
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
