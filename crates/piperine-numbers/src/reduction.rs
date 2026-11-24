//! `ArgumentReduction`: reducing an argument modulo pi/2 or modulo ln 2 with no loss of precision.
//!
//! For the circular functions a finite `f128` as large as `2^16384` is reduced with pi carried to
//! the bits the exponent needs (the idea of Payne and Hanek, with a multi-precision pi in place
//! of a table): the remainder keeps `precision` correct bits *after* the cancellation, because
//! the working precision is raised until the interval of the remainder is narrow relative to it.

use super::constants::ReferenceConstants;
use super::limbs::{Limbs, ToLimbs};
use super::precision::{Interval, MultiPrecision};
use super::quad::Float128;

/// An argument reduced modulo pi/2: `x = n * pi/2 + remainder` with `|remainder| <= pi/4`.
#[derive(Clone, Debug)]
pub struct QuarterTurns {
    /// `n mod 4`: the quadrant that decides which circular function the remainder feeds.
    pub quadrant: u8,
    /// The remainder as an interval, relatively narrow even when it is tiny.
    pub remainder: Interval,
}

/// An argument split as `x = k * ln 2 + remainder` with `|remainder| <= ln 2 / 2` (plus the width).
#[derive(Clone, Debug)]
pub struct LogarithmSteps {
    /// The integer `k`.
    pub steps: i64,
    /// The remainder as an interval.
    pub remainder: Interval,
}

/// Reductions of arguments by pi/2 and by ln 2.
pub struct ArgumentReduction;

impl ArgumentReduction {
    /// Extra bits kept beyond the requested precision to survive the cancellation of the closest
    /// binary128 approach to a multiple of pi/2 (about `2^-120` relative) with a safe margin.
    const CANCELLATION_GUARD: u32 = 160;
    /// The most bits a reduction may use before it gives up (a defect, never reached).
    const MAXIMUM_BITS: u32 = 1 << 20;

    /// `x mod pi/2` for a finite `f128`, the remainder carrying `precision` correct bits;
    /// `None` for infinity and NaN.
    pub fn quarter_turns(x: Float128, precision: u32) -> Option<QuarterTurns> {
        let value = MultiPrecision::from_quad_bits(x.to_bits())?;
        let top = value.top_exponent().unwrap_or(0);
        if value.is_zero() || top < -1 {
            // Under 1/2 < pi/4 nothing is subtracted: the remainder is the argument, exact.
            return Some(QuarterTurns { quadrant: 0, remainder: Interval::point(value, precision + 8) });
        }
        let mut working = precision + top.max(0) as u32 + Self::CANCELLATION_GUARD;
        while working <= Self::MAXIMUM_BITS {
            if let Some(reduced) = Self::attempt(&value, precision, working) {
                return Some(reduced);
            }
            working *= 2;
        }
        None
    }

    /// One reduction at `working` bits; `None` when the remainder is not relatively narrow enough.
    fn attempt(value: &MultiPrecision, precision: u32, working: u32) -> Option<QuarterTurns> {
        let half_pi = ReferenceConstants::pi(working).scaled(-1);
        let point = Interval::point(value.clone(), working);
        let quotient = point.divide(&half_pi)?;
        let nearest = quotient.lower().nearest_integer_magnitude();
        let negative = quotient.lower().is_negative();
        let turns = MultiPrecision::exact(negative, &nearest, 0);
        let remainder = point.subtract(&Interval::point(turns, working).multiply(&half_pi));
        let width = remainder.upper().subtract(remainder.lower(), 32, super::precision::Rounding::Up);
        let size_top = remainder.magnitude_top()?;
        let narrow = !remainder.contains_zero() && width.top_exponent().is_none_or(|width_top| width_top + (precision as i64) < size_top - 4);
        if !narrow {
            return None;
        }
        let low_bits = nearest.first().copied().unwrap_or(0) & 3;
        let quadrant = if negative { ((4 - low_bits) & 3) as u8 } else { low_bits as u8 };
        Some(QuarterTurns { quadrant, remainder: remainder.with_precision(precision + 32) })
    }

    /// `value = k * ln 2 + r` with `|r| <= ln 2 / 2`; `r` has an absolute error below
    /// `2^-precision`. `None` only when the value is too large for `k` to fit an `i64`.
    pub fn ln_two_steps(value: &Interval, precision: u32) -> Option<LogarithmSteps> {
        let top = value.lower().top_exponent().max(value.upper().top_exponent()).unwrap_or(0).max(0) as u32;
        if top > 60 {
            return None;
        }
        let working = precision + top + 64;
        let ln_two = ReferenceConstants::ln_two(working);
        let exact = value.with_precision(working);
        let quotient = exact.divide(&ln_two)?;
        let nearest = quotient.lower().nearest_integer_magnitude();
        let negative = quotient.lower().is_negative();
        let magnitude = nearest.to_u128()?;
        let steps = if negative { -(i64::try_from(magnitude).ok()?) } else { i64::try_from(magnitude).ok()? };
        let turns = Interval::point(MultiPrecision::exact(negative, &magnitude.to_limbs(), 0), working);
        let remainder = exact.subtract(&turns.multiply(&ln_two));
        Some(LogarithmSteps { steps, remainder })
    }
}
