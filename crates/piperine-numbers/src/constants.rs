//! `ReferenceConstants`: pi, ln 2 and ln 10 to any precision as rigorous intervals.
//!
//! pi comes from Machin's formula `pi = 16 atan(1/5) - 4 atan(1/239)`; ln 2 and ln 10 from the
//! `atanh` series, `ln 2 = 2 atanh(1/3)` and `ln 10 = 3 ln 2 + 2 atanh(1/9)` (because
//! `ln 10 = 3 ln 2 + ln(5/4)` and `ln((1 + t)/(1 - t)) = 2 atanh(t)` at `t = 1/9`). Every series
//! carries a proven bound on its omitted tail, so the interval always contains the constant. Each
//! result is cached per precision behind `std::sync::OnceLock`, which is safe `std`.

use super::precision::{Interval, MultiPrecision, Rounding};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Extra bits the series are summed with beyond the requested precision.
const GUARD_BITS: u32 = 32;

/// Which constant a cache entry holds.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Constant {
    /// The ratio of a circle's circumference to its diameter.
    Pi,
    /// The natural logarithm of 2.
    LnTwo,
    /// The natural logarithm of 10.
    LnTen,
}

/// The cache of computed constants, one entry per constant and precision.
struct ConstantCache;

impl ConstantCache {
    /// The process-wide table.
    fn table() -> &'static Mutex<HashMap<(Constant, u32), Interval>> {
        static TABLE: OnceLock<Mutex<HashMap<(Constant, u32), Interval>>> = OnceLock::new();
        TABLE.get_or_init(|| Mutex::new(HashMap::new()))
    }
}

/// pi, ln 2 and ln 10 as intervals of a given precision.
pub struct ReferenceConstants;

impl ReferenceConstants {
    /// pi, to `precision` bits (the interval is much narrower than two units in the last place).
    pub fn pi(precision: u32) -> Interval {
        Self::cached(Constant::Pi, precision)
    }

    /// ln 2, to `precision` bits.
    pub fn ln_two(precision: u32) -> Interval {
        Self::cached(Constant::LnTwo, precision)
    }

    /// ln 10, to `precision` bits.
    pub fn ln_ten(precision: u32) -> Interval {
        Self::cached(Constant::LnTen, precision)
    }

    /// The cached value of a constant, computed on first use.
    fn cached(constant: Constant, precision: u32) -> Interval {
        let key = (constant, precision);
        if let Ok(guard) = ConstantCache::table().lock() {
            if let Some(found) = guard.get(&key) {
                return found.clone();
            }
        }
        let working = precision + GUARD_BITS;
        let value = match constant {
            Constant::Pi => Self::compute_pi(working),
            Constant::LnTwo => Self::compute_ln_two(working),
            Constant::LnTen => Self::compute_ln_ten(working),
        };
        if let Ok(mut guard) = ConstantCache::table().lock() {
            guard.insert(key, value.clone());
        }
        value
    }

    /// Machin: `16 atan(1/5) - 4 atan(1/239)`.
    fn compute_pi(working: u32) -> Interval {
        let five = ReciprocalSeries::new(5, working, true).sum();
        let two_thirty_nine = ReciprocalSeries::new(239, working, true).sum();
        Self::times_integer(&five, 16, working).subtract(&Self::times_integer(&two_thirty_nine, 4, working))
    }

    /// `2 atanh(1/3)`.
    fn compute_ln_two(working: u32) -> Interval {
        Self::times_integer(&ReciprocalSeries::new(3, working, false).sum(), 2, working)
    }

    /// `3 ln 2 + 2 atanh(1/9)`.
    fn compute_ln_ten(working: u32) -> Interval {
        let ln_two = Self::compute_ln_two(working);
        Self::times_integer(&ln_two, 3, working).add(&Self::times_integer(&ReciprocalSeries::new(9, working, false).sum(), 2, working))
    }

    /// `interval * factor` for a small exact integer.
    fn times_integer(interval: &Interval, factor: u64, precision: u32) -> Interval {
        interval.multiply(&Interval::integer(false, factor, precision))
    }
}

/// The series `sum_k s_k / ((2k + 1) m^(2k + 1))` of `atan(1/m)` (alternating signs) or `atanh(1/m)`.
struct ReciprocalSeries {
    /// The reciprocal's integer `m`.
    base: u64,
    /// The working precision in bits.
    precision: u32,
    /// Whether the terms alternate in sign (`atan`) or all add (`atanh`).
    alternating: bool,
}

impl ReciprocalSeries {
    /// The series for `1/base` at `precision` bits.
    fn new(base: u64, precision: u32, alternating: bool) -> Self {
        ReciprocalSeries { base, precision, alternating }
    }

    /// The interval of the sum, with the omitted tail bounded by the first omitted term (`atan`) or
    /// by twice it (`atanh`, whose terms shrink by at least 1/9 each, so the tail is below 9/8 of it).
    fn sum(&self) -> Interval {
        let precision = self.precision;
        let square = MultiPrecision::from_integer(false, self.base * self.base);
        let mut power_lower = MultiPrecision::from_integer(false, 1).divide(&MultiPrecision::from_integer(false, self.base), precision, Rounding::Down).unwrap_or_else(MultiPrecision::zero);
        let mut power_upper = MultiPrecision::from_integer(false, 1).divide(&MultiPrecision::from_integer(false, self.base), precision, Rounding::Up).unwrap_or_else(MultiPrecision::zero);
        let mut lower = MultiPrecision::zero();
        let mut upper = MultiPrecision::zero();
        let stop = -(precision as i64) - 4;
        let mut index = 0u64;
        loop {
            let divisor = MultiPrecision::from_integer(false, 2 * index + 1);
            let term_lower = power_lower.divide(&divisor, precision, Rounding::Down).unwrap_or_else(MultiPrecision::zero);
            let term_upper = power_upper.divide(&divisor, precision, Rounding::Up).unwrap_or_else(MultiPrecision::zero);
            let subtract = self.alternating && index % 2 == 1;
            if subtract {
                lower = lower.subtract(&term_upper, precision, Rounding::Down);
                upper = upper.subtract(&term_lower, precision, Rounding::Up);
            } else {
                lower = lower.add(&term_lower, precision, Rounding::Down);
                upper = upper.add(&term_upper, precision, Rounding::Up);
            }
            if power_upper.top_exponent().is_some_and(|top| top < stop) {
                break;
            }
            power_lower = power_lower.divide(&square, precision, Rounding::Down).unwrap_or_else(MultiPrecision::zero);
            power_upper = power_upper.divide(&square, precision, Rounding::Up).unwrap_or_else(MultiPrecision::zero);
            index += 1;
        }
        // The first omitted term is at most power_upper / square; atanh's tail is below twice that.
        let tail = power_upper.scaled(if self.alternating { 0 } else { 1 });
        if self.alternating {
            let next_sign_negative = index % 2 == 0;
            if next_sign_negative {
                lower = lower.subtract(&tail, precision, Rounding::Down);
            } else {
                upper = upper.add(&tail, precision, Rounding::Up);
            }
        } else {
            upper = upper.add(&tail, precision, Rounding::Up);
        }
        Interval::new(lower, upper, precision)
    }
}
