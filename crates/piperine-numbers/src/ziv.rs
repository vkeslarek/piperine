//! `ZivRounding`: from an interval evaluation to one correctly rounded `f128`.
//!
//! Ziv's strategy: evaluate the function as an interval at a modest precision; if both bounds
//! round to the same `f128`, so does the exact value (rounding to nearest is monotonic) and the
//! answer is decided; otherwise raise the precision and evaluate again. A function first offers
//! its exact cases (a result that is representable or a midpoint, where the loop would never
//! end); the loop runs only when there is none.

use super::precision::Interval;
use super::quad::Float128;

/// The precision schedule and the rounding decision of the transcendental functions.
pub struct ZivRounding;

impl ZivRounding {
    /// The precision of the first evaluation, in bits.
    pub const FIRST_PRECISION: u32 = 160;
    /// The precision past which the loop stops: a defect, never reached by a function with its
    /// exact cases.
    pub const MAXIMUM_PRECISION: u32 = 1 << 20;

    /// The correctly rounded value of the quantity `evaluation` brackets: `exact` when the caller
    /// has an exact case (no evaluation happens), otherwise the first precision, doubling from
    /// [`Self::FIRST_PRECISION`], at which both bounds of `evaluation(precision)` round to one
    /// pattern. `None` when `evaluation` gives none, or when the cap is passed.
    pub fn round<F>(exact: Option<Float128>, mut evaluation: F) -> Option<Float128>
    where
        F: FnMut(u32) -> Option<Interval>,
    {
        if exact.is_some() {
            return exact;
        }
        let mut precision = Self::FIRST_PRECISION;
        while precision <= Self::MAXIMUM_PRECISION {
            let interval = evaluation(precision)?;
            let (lower, upper) = interval.round_to_quad();
            if lower == upper {
                return Some(Float128::from_bits(lower));
            }
            precision *= 2;
        }
        None
    }
}
