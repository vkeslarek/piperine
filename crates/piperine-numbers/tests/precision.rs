//! `MultiPrecision`, `Interval`, the reference constants, argument reduction and Ziv rounding
//! against exact rational arithmetic (`ExactRational` over `num-bigint`) and committed
//! published digits (CV-31). Every bound is checked exactly: no tolerance.

#[path = "support/exact.rs"]
mod exact;
#[path = "support/exact_algebra.rs"]
mod exact_algebra;
#[path = "support/exact_text.rs"]
mod exact_text;
#[path = "support/random.rs"]
mod random;

use exact::ExactRational;
use num_bigint::BigUint;
use piperine_numbers::constants::ReferenceConstants;
use piperine_numbers::precision::{Interval, MultiPrecision, Rounding};
use random::Xorshift;
use std::cmp::Ordering;
use std::fs;
use std::path::PathBuf;

/// The scenario of the directed-rounding test.
struct DirectedScenario {
    /// The deterministic operand source.
    random: Xorshift,
}

impl DirectedScenario {
    /// A scenario with a seeded generator.
    fn new(seed: u64) -> Self {
        DirectedScenario { random: Xorshift::new(seed) }
    }

    /// A random value of up to `bits` bits with a random exponent.
    fn operand(&mut self, bits: u64) -> MultiPrecision {
        let width = 1 + self.random.next_u64() % bits;
        let limbs: Vec<u64> = (0..width.div_ceil(64)).map(|_| self.random.next_u64()).collect();
        let exponent = (self.random.next_u64() % 400) as i64 - 200;
        MultiPrecision::exact(self.random.next_u64() & 1 == 1, &limbs, exponent)
    }

    /// The exact rational of a value.
    fn rational(value: &MultiPrecision) -> ExactRational {
        let magnitude = value.magnitude().iter().rev().fold(BigUint::from(0u32), |acc, limb| (acc << 64u32) | BigUint::from(*limb));
        ExactRational::dyadic(value.is_negative(), &magnitude, value.exponent())
    }

    /// Asserts `lower <= exact <= upper`, that both bounds have at most `precision` bits, and that
    /// they are the adjacent values of that precision (equal when `exact` is representable).
    fn assert_bracket(exact: &ExactRational, lower: &MultiPrecision, upper: &MultiPrecision, precision: u32, context: &str) {
        let (low, high) = (Self::rational(lower), Self::rational(upper));
        assert_ne!(low.compare(exact), Ordering::Greater, "{context}: lower bound above the exact value");
        assert_ne!(high.compare(exact), Ordering::Less, "{context}: upper bound below the exact value");
        for bound in [lower, upper] {
            assert!(bound.magnitude().len() as u64 * 64 - bound.magnitude().last().map_or(0, |limb| limb.leading_zeros() as u64) <= precision as u64, "{context}: more than {precision} bits");
        }
        if low.compare(&high) == Ordering::Equal {
            assert_eq!(low.compare(exact), Ordering::Equal, "{context}: equal bounds mean an exact result");
            return;
        }
        let top = lower.top_exponent().unwrap_or(0);
        let unit = ExactRational::dyadic(false, &BigUint::from(1u32), top - precision as i64 + 1);
        assert_ne!(high.minus(&low).compare(&unit), Ordering::Greater, "{context}: bounds more than one unit in the last place apart");
    }

    /// Every operation at one precision on random operands.
    fn check_precision(&mut self, precision: u32, count: usize) {
        for _ in 0..count {
            let (a, b) = (self.operand(2 * precision as u64), self.operand(2 * precision as u64));
            let (x, y) = (Self::rational(&a), Self::rational(&b));
            let context = format!("p={precision} a={a:?} b={b:?}");
            Self::assert_bracket(&x.plus(&y), &a.add(&b, precision, Rounding::Down), &a.add(&b, precision, Rounding::Up), precision, &format!("add {context}"));
            Self::assert_bracket(&x.minus(&y), &a.subtract(&b, precision, Rounding::Down), &a.subtract(&b, precision, Rounding::Up), precision, &format!("sub {context}"));
            Self::assert_bracket(&x.times(&y), &a.multiply(&b, precision, Rounding::Down), &a.multiply(&b, precision, Rounding::Up), precision, &format!("mul {context}"));
            if !b.is_zero() {
                let quotient = x.divided_by(&y).unwrap_or_else(|| x.clone());
                let (low, high) = (a.divide(&b, precision, Rounding::Down), a.divide(&b, precision, Rounding::Up));
                if let (Some(low), Some(high)) = (low, high) {
                    Self::assert_bracket(&quotient, &low, &high, precision, &format!("div {context}"));
                }
            }
            let positive = a.absolute();
            let radicand = Self::rational(&positive);
            let (low, high) = (positive.square_root(precision, Rounding::Down), positive.square_root(precision, Rounding::Up));
            if let (Some(low), Some(high)) = (low, high) {
                let (low_value, high_value) = (Self::rational(&low), Self::rational(&high));
                assert_ne!(low_value.times(&low_value).compare(&radicand), Ordering::Greater, "sqrt lower bound squared exceeds {context}");
                assert_ne!(high_value.times(&high_value).compare(&radicand), Ordering::Less, "sqrt upper bound squared is below {context}");
                let top = low.top_exponent().unwrap_or(0);
                let unit = ExactRational::dyadic(false, &BigUint::from(1u32), top - precision as i64 + 1);
                assert_ne!(high_value.minus(&low_value).compare(&unit), Ordering::Greater, "sqrt bounds apart by more than one unit {context}");
            }
        }
    }

    /// The interval operations contain the exact result of the same operation on contained points.
    fn check_intervals(&mut self, precision: u32, count: usize) {
        for _ in 0..count {
            let widen = |scenario: &mut DirectedScenario| {
                let centre = scenario.operand(precision as u64);
                let radius = MultiPrecision::exact(false, &[1 + scenario.random.next_u64() % 1000], -((scenario.random.next_u64() % 200) as i64) - 20);
                (Interval::new(centre.subtract(&radius, precision, Rounding::Down), centre.add(&radius, precision, Rounding::Up), precision), centre)
            };
            let ((first, first_point), (second, second_point)) = (widen(self), widen(self));
            let (x, y) = (Self::rational(&first_point), Self::rational(&second_point));
            let check = |interval: &Interval, exact: &ExactRational, name: &str| {
                assert_ne!(Self::rational(interval.lower()).compare(exact), Ordering::Greater, "{name}: lower bound above a contained result");
                assert_ne!(Self::rational(interval.upper()).compare(exact), Ordering::Less, "{name}: upper bound below a contained result");
            };
            check(&first.add(&second), &x.plus(&y), "interval add");
            check(&first.subtract(&second), &x.minus(&y), "interval sub");
            check(&first.multiply(&second), &x.times(&y), "interval mul");
            if let (Some(quotient), Some(exact)) = (first.divide(&second), x.divided_by(&y)) {
                check(&quotient, &exact, "interval div");
            }
            assert!(first.contains(&first_point) && second.contains(&second_point), "the centre lies in its interval");
        }
    }
}

#[test]
fn directed_rounding_brackets_the_exact_result() {
    let mut scenario = DirectedScenario::new(100);
    for precision in [120u32, 240, 1000] {
        scenario.check_precision(precision, 5_000);
        scenario.check_intervals(precision, 1_000);
    }
}

/// The published digits of pi, ln 2 and ln 10 (OEIS A000796, A002162, A002392; committed by
/// `tests/vectors/generate.py`, which also checks them against their well-known leading digits).
struct PublishedConstants;

impl PublishedConstants {
    /// The committed file's data lines as `(name, digits text)`.
    fn rows() -> Vec<(String, String)> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/vectors/constants.txt");
        fs::read_to_string(path).unwrap_or_default().lines().filter(|line| !line.starts_with('#')).filter_map(|line| line.split_once('\t')).map(|(name, digits)| (name.to_string(), digits.to_string())).collect()
    }

    /// The constant's value truncated to 1000 decimal places, as an exact rational.
    fn truncated(name: &str) -> ExactRational {
        let digits = Self::rows().into_iter().find(|(row, _)| row == name).map(|(_, digits)| digits).unwrap_or_default();
        let (whole, fraction) = digits.split_once('.').unwrap_or((&digits, ""));
        assert_eq!(fraction.len(), 1000, "{name} has 1000 decimal places");
        ExactRational::decimal(false, &format!("{whole}{fraction}").parse::<BigUint>().unwrap_or_default(), -1000)
    }

    /// The `2^20000`-scaled floor of pi from the committed hex file.
    fn pi_bits() -> BigUint {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/vectors/pi_bits.hex");
        let text = fs::read_to_string(path).unwrap_or_default();
        let hex: String = text.lines().filter(|line| !line.starts_with('#')).collect();
        BigUint::parse_bytes(hex.trim().as_bytes(), 16).unwrap_or_default()
    }

    /// Asserts the interval is consistent with `[truncated, truncated + 10^-1000)` (the constant
    /// lies in both) and no wider than two units in the last place of `precision` bits.
    fn assert_consistent(interval: &Interval, truncated: &ExactRational, precision: u32, name: &str) {
        let (lower, upper) = (DirectedScenario::rational(interval.lower()), DirectedScenario::rational(interval.upper()));
        let ceiling = truncated.plus(&ExactRational::decimal(false, &BigUint::from(1u32), -1000));
        assert_ne!(lower.compare(&ceiling), Ordering::Greater, "{name} at {precision} bits: lower bound above the published value");
        assert_ne!(upper.compare(truncated), Ordering::Less, "{name} at {precision} bits: upper bound below the published value");
        let top = interval.lower().top_exponent().unwrap_or(0);
        let two_units = ExactRational::dyadic(false, &BigUint::from(2u32), top - precision as i64 + 1);
        assert_ne!(upper.minus(&lower).compare(&two_units), Ordering::Greater, "{name} at {precision} bits: wider than two units in the last place");
    }
}

#[test]
fn reference_constants_contain_their_published_digits() {
    for (name, constant) in [("pi", ReferenceConstants::pi as fn(u32) -> Interval), ("ln2", ReferenceConstants::ln_two), ("ln10", ReferenceConstants::ln_ten)] {
        let published = PublishedConstants::truncated(name);
        for precision in [120u32, 500, 3_300, 4_000] {
            PublishedConstants::assert_consistent(&constant(precision), &published, precision, name);
        }
    }
    let pi = ReferenceConstants::pi(16_600);
    PublishedConstants::assert_consistent(&pi, &PublishedConstants::truncated("pi"), 16_600, "pi");
    let scaled = PublishedConstants::pi_bits();
    let (lower, upper) = (DirectedScenario::rational(pi.lower()), DirectedScenario::rational(pi.upper()));
    assert_ne!(lower.compare(&ExactRational::dyadic(false, &(&scaled + 1u32), -20_000)), Ordering::Greater, "pi at 16 600 bits against the 20 000-bit value (lower)");
    assert_ne!(upper.compare(&ExactRational::dyadic(false, &scaled, -20_000)), Ordering::Less, "pi at 16 600 bits against the 20 000-bit value (upper)");
    assert_eq!(ReferenceConstants::pi(500).lower().compare(ReferenceConstants::pi(500).lower()), Ordering::Equal, "a cached value is the same value");
}
