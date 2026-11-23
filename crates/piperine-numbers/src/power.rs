//! `ExactPower`: the exact value of `x^y` when it is a rational the loop of `ziv.rs` could never
//! decide (a representable value or a midpoint).
//!
//! `x = m * 2^a` with `m` odd, `y = p / 2^j` with `p` odd. `x^y` is rational only when `x` is a
//! `2^j`-th power: `m` is a perfect `2^j`-th power and `a` divisible by `2^j`. Then the result is
//! `t^p` for `t = x^(1/2^j)`, an odd integer power times a power of two; it has more than
//! `2 * 113 + 2` significant bits (so it is neither representable nor a midpoint, and the Ziv loop
//! ends) unless `p * bits(t) <= 230`, or `t` is a power of two. In those two cases the result is
//! computed exactly with limbs and rounded once. A negative exponent leaves an odd denominator
//! above 1, which is neither: only a power of two stays exact.

use super::format::BinaryFormat;
use super::limbs::{Limbs, ToLimbs};
use super::quad::Float128;

/// Exact rational powers of binary128 values.
pub struct ExactPower;

impl ExactPower {
    /// The most significant bits an exact result may have to be computed exactly (the loop is
    /// safe above `2 * 113 + 2` bits, and half that is ruled out by the bit count of the root).
    const EXACT_BITS: u64 = 230;
    /// The largest exponent of a power of two kept as an `i64` (beyond it the result overflows or underflows).
    const EXPONENT_CAP: i128 = 200_000;

    /// `|x|^y` when it is an exact rational result, `None` when the loop must decide. Both
    /// arguments are finite and nonzero; the sign of `x` is the caller's.
    pub fn of(x: Float128, y: Float128) -> Option<Float128> {
        let (_, x_significand, x_exponent) = BinaryFormat::QUAD.decode(x.to_bits())?;
        let (y_negative, y_significand, y_exponent) = BinaryFormat::QUAD.decode(y.to_bits())?;
        let zeros = x_significand.trailing_zeros();
        let mut odd = x_significand >> zeros;
        let mut shift = x_exponent + i64::from(zeros);
        let y_zeros = y_significand.trailing_zeros();
        let numerator = y_significand >> y_zeros;
        let mut roots = -(y_exponent + i64::from(y_zeros));
        while roots > 0 {
            let (root, exact) = odd.to_limbs().integer_square_root();
            if !exact || shift % 2 != 0 {
                return None;
            }
            odd = root.to_u128()?;
            shift /= 2;
            roots -= 1;
            if odd == 1 && shift == 0 {
                break;
            }
        }
        let multiplier = if roots < 0 { -roots } else { 0 };
        Self::integer_power(odd, shift, numerator, multiplier, y_negative)
    }

    /// `(odd * 2^shift)^(numerator * 2^multiplier)` (negated exponent when `negative`), exact.
    fn integer_power(odd: u128, shift: i64, numerator: u128, multiplier: i64, negative: bool) -> Option<Float128> {
        let exponent_bits = 128 - numerator.leading_zeros() as i64 + multiplier;
        if odd == 1 {
            // A power of two stays a power of two, whatever the exponent: 2^(shift * exponent).
            let scaled = if exponent_bits > 100 { i128::from(shift.signum()) * (i128::MAX / 4) } else { (shift as i128).saturating_mul((numerator as i128) << multiplier.min(60)) };
            let total = if negative { -scaled } else { scaled };
            let total = total.clamp(-Self::EXPONENT_CAP, Self::EXPONENT_CAP) as i64;
            return Some(Self::rounded(&1u64.to_limbs(), total));
        }
        if negative || exponent_bits > 8 {
            return None;
        }
        let exponent = numerator << multiplier;
        let bits = odd.to_limbs().bit_length();
        if exponent * u128::from(bits as u64) > u128::from(Self::EXACT_BITS) {
            return None;
        }
        let mut power = 1u64.to_limbs();
        for _ in 0..exponent {
            power = power.times(&odd.to_limbs());
        }
        Some(Self::rounded(&power, shift.saturating_mul(exponent as i64)))
    }

    /// The pattern of `magnitude * 2^exponent`, rounded once.
    fn rounded(magnitude: &[u64], exponent: i64) -> Float128 {
        Float128::from_bits(BinaryFormat::QUAD.round(false, magnitude, exponent, false))
    }
}
