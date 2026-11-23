//! The correctly rounded transcendental methods of `Float128` (H.2): each evaluates an interval
//! that contains the exact value and lets [`ZivRounding`] raise the precision until both bounds
//! round to one binary128 pattern, so the result is the correctly rounded one. Special values
//! follow IEEE 754-2019 section 9.2 and H.2 rule 7 (a result too large is an infinity, a result
//! too small underflows to a subnormal or zero).

use super::constants::ReferenceConstants;
use super::elementary::ElementaryFunctions;
use super::limbs::{Limbs, ToLimbs};
use super::precision::{Interval, MultiPrecision};
use super::quad::Float128;
use super::ziv::ZivRounding;

/// Shared plumbing of the transcendental methods.
struct Evaluation;

impl Evaluation {
    /// Bits kept beyond the Ziv precision so each interval is far narrower than it.
    const GUARD_BITS: u32 = 48;

    /// The exact point `x` as an interval carried at `precision` plus the guard bits.
    fn point(x: Float128, precision: u32) -> Option<Interval> {
        Some(Interval::point(MultiPrecision::from_quad_bits(x.to_bits())?, precision + Self::GUARD_BITS))
    }

    /// Runs the Ziv loop; a failure of the loop (never reached) is NaN.
    fn rounded<F>(exact: Option<Float128>, evaluation: F) -> Float128
    where
        F: FnMut(u32) -> Option<Interval>,
    {
        ZivRounding::round(exact, evaluation).unwrap_or(Float128::NAN)
    }

    /// The integer `value` as a binary128.
    fn integer(negative: bool, value: u64) -> Float128 {
        Float128::from_bits(super::format::BinaryFormat::QUAD.round(negative, &value.to_limbs(), 0, false))
    }

    /// `log_base(x)` as `ln x / ln base` for a finite positive `x`, with the exact case supplied.
    fn logarithm(x: Float128, exact: Option<Float128>, divisor: fn(u32) -> Interval) -> Float128 {
        Self::rounded(exact, |precision| {
            let argument = Self::point(x, precision)?;
            ElementaryFunctions::ln(&argument)?.divide(&divisor(precision + Self::GUARD_BITS))
        })
    }

    /// `Some(k)` when `x` is `2^k` exactly.
    fn power_of_two(x: Float128) -> Option<i64> {
        let (negative, significand, exponent) = super::format::BinaryFormat::QUAD.decode(x.to_bits())?;
        (!negative && significand != 0 && significand.is_power_of_two()).then(|| exponent + i64::from(significand.trailing_zeros()))
    }

    /// `Some(k)` when `x` is `10^k` exactly (`k` from 0 to 48: `10^k = 2^k 5^k` and `5^k` fits 113 bits).
    fn power_of_ten(x: Float128) -> Option<u64> {
        let (negative, significand, exponent) = super::format::BinaryFormat::QUAD.decode(x.to_bits())?;
        if negative || significand == 0 || exponent + i64::from(significand.trailing_zeros()) < 0 {
            return None;
        }
        let integer = if exponent >= 0 { significand.to_limbs().shifted_left(exponent as u64) } else { significand.to_limbs().shifted_right((-exponent) as u64) };
        let mut power = 1u64.to_limbs();
        for k in 0..=48u64 {
            if power.compare_magnitude(&integer) == core::cmp::Ordering::Equal {
                return Some(k);
            }
            power = power.times_small_plus(10, 0);
        }
        None
    }
}

impl Float128 {
    /// `e^x`, correctly rounded.
    pub fn exp(self) -> Float128 {
        if self.is_nan() {
            return Float128::NAN;
        }
        if self.is_infinite() {
            return if self.is_sign_negative() { Float128::ZERO } else { Float128::INFINITY };
        }
        if self.is_zero() {
            return Float128::ONE;
        }
        let eleven_thousand = Evaluation::integer(false, 11_357);
        if self >= eleven_thousand {
            return Float128::INFINITY;
        }
        if self <= -Evaluation::integer(false, 11_434) {
            return Float128::ZERO;
        }
        Evaluation::rounded(None, |precision| ElementaryFunctions::exp(&Evaluation::point(self, precision)?))
    }

    /// The natural logarithm, correctly rounded (NaN below zero, `-inf` at zero).
    pub fn ln(self) -> Float128 {
        match self.logarithm_domain() {
            Some(result) => result,
            None if self == Float128::ONE => Float128::ZERO,
            None => Evaluation::rounded(None, |precision| ElementaryFunctions::ln(&Evaluation::point(self, precision)?)),
        }
    }

    /// The special values of every logarithm: NaN, a negative, zero and infinity.
    fn logarithm_domain(self) -> Option<Float128> {
        if self.is_nan() || (self.is_sign_negative() && !self.is_zero()) {
            return Some(Float128::NAN);
        }
        if self.is_zero() {
            return Some(Float128::NEGATIVE_INFINITY);
        }
        self.is_infinite().then_some(Float128::INFINITY)
    }

    /// The base-2 logarithm, correctly rounded.
    pub fn log2(self) -> Float128 {
        if let Some(result) = self.logarithm_domain() {
            return result;
        }
        let exact = Evaluation::power_of_two(self).map(|k| Evaluation::integer(k < 0, k.unsigned_abs()));
        Evaluation::logarithm(self, exact, ReferenceConstants::ln_two)
    }

    /// The base-10 logarithm, correctly rounded.
    pub fn log10(self) -> Float128 {
        if let Some(result) = self.logarithm_domain() {
            return result;
        }
        let exact = Evaluation::power_of_ten(self).map(|k| Evaluation::integer(false, k));
        Evaluation::logarithm(self, exact, ReferenceConstants::ln_ten)
    }
}
