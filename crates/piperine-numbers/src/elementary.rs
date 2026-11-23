//! `ElementaryFunctions`: `exp`, `ln`, `sin`/`cos` and `atan` of an interval, with rigorous bounds.
//!
//! Each function reduces its argument (by ln 2, to `[0.75, 1.5]`, by pi/2 is done by the caller,
//! by argument halving), sums a Taylor series with interval arithmetic, and adds a proven bound
//! on the omitted tail. The series stop when a term is `2^-(precision + 8)` below the leading
//! term, so the result is relatively accurate even for a tiny argument. The working precision is
//! the precision of the argument interval; the callers add guard bits.

use super::constants::ReferenceConstants;
use super::precision::{Interval, MultiPrecision, Rounding};
use super::reduction::ArgumentReduction;

/// Elementary functions of intervals.
pub struct ElementaryFunctions;

impl ElementaryFunctions {
    /// The magnitude beyond which `exp` overflows or underflows binary128 whatever the format.
    const EXPONENT_LIMIT: i64 = 13_000;

    /// `2^-(precision + 8)` relative to `2^leading`: the exponent below which a term is dropped.
    fn stop_exponent(leading: i64, precision: u32) -> i64 {
        leading - precision as i64 - 8
    }

    /// `x` with each bound clamped into `[-EXPONENT_LIMIT, EXPONENT_LIMIT]` (an `exp` beyond it is infinity or zero anyway).
    fn clamped(x: &Interval) -> Interval {
        let limit = MultiPrecision::from_integer(false, Self::EXPONENT_LIMIT as u64);
        let clamp = |bound: &MultiPrecision| {
            if bound.compare(&limit) == std::cmp::Ordering::Greater {
                limit.clone()
            } else if bound.compare(&limit.negated()) == std::cmp::Ordering::Less {
                limit.negated()
            } else {
                bound.clone()
            }
        };
        Interval::new(clamp(x.lower()), clamp(x.upper()), x.precision())
    }

    /// `exp(x)`.
    pub fn exp(x: &Interval) -> Option<Interval> {
        let precision = x.precision();
        let reduced = ArgumentReduction::ln_two_steps(&Self::clamped(x), precision)?;
        let remainder = reduced.remainder.with_precision(precision);
        let radius = remainder.largest_magnitude();
        let one = Interval::integer(false, 1, precision);
        let mut term = one.clone();
        let mut sum = one;
        let mut index = 1u64;
        loop {
            term = term.multiply(&remainder).divide(&Interval::integer(false, index, precision))?;
            sum = sum.add(&term);
            if term.magnitude_top().is_none_or(|top| top < Self::stop_exponent(0, precision)) {
                break;
            }
            index += 1;
        }
        // The omitted tail is at most 3 * |r|^(n+1) / (n+1)!, which is below 3 times the next term.
        let next = term.largest_magnitude().multiply(&radius, precision, Rounding::Up).divide(&MultiPrecision::from_integer(false, index + 1), precision, Rounding::Up)?;
        let tail = next.multiply(&MultiPrecision::from_integer(false, 3), precision, Rounding::Up);
        let enclosed = sum.add(&Interval::symmetric(&tail, precision)).scaled(reduced.steps);
        // `exp` is positive: a bound dragged below zero by the tail must not round to `-0` (and so disagree with `+0`).
        let lower = if enclosed.lower().is_negative() { MultiPrecision::zero() } else { enclosed.lower().clone() };
        Some(Interval::new(lower, enclosed.upper().clone(), precision))
    }

    /// `ln(x)` for an interval of positive values.
    pub fn ln(x: &Interval) -> Option<Interval> {
        let precision = x.precision();
        if !x.is_positive() {
            return None;
        }
        let mut exponent = x.lower().top_exponent()?;
        let mut scaled = x.scaled(-exponent);
        let three_halves = MultiPrecision::exact(false, &[3], -1);
        if scaled.lower().compare(&three_halves) != std::cmp::Ordering::Less {
            exponent += 1;
            scaled = scaled.scaled(-1);
        }
        let one = Interval::integer(false, 1, precision);
        let t = scaled.subtract(&one).divide(&scaled.add(&one))?;
        let atanh = Self::odd_series(&t, false)?;
        let ln_two = ReferenceConstants::ln_two(precision);
        let steps = Interval::point(MultiPrecision::from_integer(exponent < 0, exponent.unsigned_abs()), precision);
        Some(steps.multiply(&ln_two).add(&atanh.scaled(1)))
    }

    /// `sum_k s_k t^(2k+1) / (2k+1)` with `s_k = (-1)^k` when `alternating`, else `1`: `atan t` or `atanh t`.
    fn odd_series(t: &Interval, alternating: bool) -> Option<Interval> {
        let precision = t.precision();
        if t.lower().is_zero() && t.upper().is_zero() {
            return Some(t.clone());
        }
        let square = t.multiply(t);
        let radius_square = square.largest_magnitude();
        let leading = t.magnitude_top()?;
        let stop = Self::stop_exponent(leading, precision);
        let mut power = t.clone();
        let mut sum = t.clone();
        let mut index = 1u64;
        loop {
            power = power.multiply(&square);
            let term = power.divide(&Interval::integer(false, 2 * index + 1, precision))?;
            sum = if alternating && index % 2 == 1 { sum.subtract(&term) } else { sum.add(&term) };
            if term.magnitude_top().is_none_or(|top| top < stop) {
                // The tail is below twice the next power (atanh, ratio below 1/2) or the next term (atan).
                let next = power.largest_magnitude().multiply(&radius_square, precision, Rounding::Up);
                let tail = if alternating { next } else { next.multiply(&MultiPrecision::from_integer(false, 2), precision, Rounding::Up) };
                return Some(sum.add(&Interval::symmetric(&tail, precision)));
            }
            index += 1;
        }
    }

    /// `(sin r, cos r)` for `|r| <= 1`.
    pub fn sin_cos(r: &Interval) -> Option<(Interval, Interval)> {
        let precision = r.precision();
        let square = r.multiply(r);
        let radius_square = square.largest_magnitude();
        let leading = r.magnitude_top().unwrap_or(0);
        let stop_sine = Self::stop_exponent(leading, precision);
        let stop_cosine = Self::stop_exponent(0, precision);
        let mut sine_term = r.clone();
        let mut cosine_term = Interval::integer(false, 1, precision);
        let mut sine = r.clone();
        let mut cosine = cosine_term.clone();
        let mut index = 1u64;
        let (mut sine_done, mut cosine_done) = (r.magnitude_top().is_none(), false);
        let mut sine_tail = MultiPrecision::zero();
        let mut cosine_tail = MultiPrecision::zero();
        while !(sine_done && cosine_done) {
            if !cosine_done {
                cosine_term = cosine_term.multiply(&square).divide(&Interval::integer(false, (2 * index - 1) * (2 * index), precision))?.negated();
                cosine = cosine.add(&cosine_term);
                if cosine_term.magnitude_top().is_none_or(|top| top < stop_cosine) {
                    cosine_done = true;
                    cosine_tail = cosine_term.largest_magnitude().multiply(&radius_square, precision, Rounding::Up);
                }
            }
            if !sine_done {
                sine_term = sine_term.multiply(&square).divide(&Interval::integer(false, (2 * index) * (2 * index + 1), precision))?.negated();
                sine = sine.add(&sine_term);
                if sine_term.magnitude_top().is_none_or(|top| top < stop_sine) {
                    sine_done = true;
                    sine_tail = sine_term.largest_magnitude().multiply(&radius_square, precision, Rounding::Up);
                }
            }
            index += 1;
        }
        Some((sine.add(&Interval::symmetric(&sine_tail, precision)), cosine.add(&Interval::symmetric(&cosine_tail, precision))))
    }

    /// `atan(x)` for an interval of nonzero values.
    pub fn atan(x: &Interval) -> Option<Interval> {
        let precision = x.precision();
        let negative = x.is_negative();
        let magnitude = if negative { x.negated() } else { x.clone() };
        if !magnitude.is_positive() {
            return None;
        }
        let one = Interval::integer(false, 1, precision);
        let beyond_one = magnitude.lower().compare(&MultiPrecision::from_integer(false, 1)) == std::cmp::Ordering::Greater;
        let argument = if beyond_one { one.divide(&magnitude)? } else { magnitude };
        let mut halved = argument;
        let mut halvings = 0i64;
        let small = MultiPrecision::exact(false, &[1], -8);
        while halved.upper().compare(&small) == std::cmp::Ordering::Greater {
            let root = halved.multiply(&halved).add(&one).square_root()?;
            halved = halved.divide(&one.add(&root))?;
            halvings += 1;
        }
        let series = Self::odd_series(&halved, true)?.scaled(halvings);
        let result = if beyond_one { ReferenceConstants::pi(precision).scaled(-1).subtract(&series) } else { series };
        Some(if negative { result.negated() } else { result })
    }
}
