//! `MultiPrecision`, `Interval`, the reference constants, argument reduction and Ziv rounding
//! against exact rational arithmetic (`ExactRational` over `num-bigint`) and committed
//! published digits (CV-31). Every bound is checked exactly: no tolerance.


use piperine_device_fixtures::exact::ExactRational;
use num_bigint::{BigInt, BigUint};
use piperine_numbers::constants::ReferenceConstants;
use piperine_numbers::quad::Float128;
use piperine_numbers::reduction::ArgumentReduction;
use piperine_numbers::ziv::ZivRounding;
use piperine_numbers::precision::{Interval, MultiPrecision, Rounding};
use piperine_device_fixtures::random::ShiftGenerator;
use std::cmp::Ordering;
use std::fs;
use std::path::PathBuf;

/// The scenario of the directed-rounding test.
struct DirectedScenario {
    /// The deterministic operand source.
    random: ShiftGenerator,
}

impl DirectedScenario {
    /// A scenario with a seeded generator.
    fn new(seed: u64) -> Self {
        DirectedScenario { random: ShiftGenerator::new(seed) }
    }

    /// A random value of up to `bits` bits with a random exponent.
    fn operand(&mut self, bits: u64) -> MultiPrecision {
        let width = 1 + self.random.next_bits() % bits;
        let limbs: Vec<u64> = (0..width.div_ceil(64)).map(|_| self.random.next_bits()).collect();
        let exponent = (self.random.next_bits() % 400) as i64 - 200;
        MultiPrecision::exact(self.random.next_bits() & 1 == 1, &limbs, exponent)
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
                let radius = MultiPrecision::exact(false, &[1 + scenario.random.next_bits() % 1000], -((scenario.random.next_bits() % 200) as i64) - 20);
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

/// Argument reduction against exact rational arithmetic with pi at 20 000 bits and ln 2 at 1000 decimals.
struct ReductionScenario {
    /// The deterministic operand source.
    random: ShiftGenerator,
    /// `floor(pi * 2^20000)`.
    pi_scaled: BigUint,
}

impl ReductionScenario {
    /// A scenario with a seeded generator and pi from the committed file.
    fn new(seed: u64) -> Self {
        ReductionScenario { random: ShiftGenerator::new(seed), pi_scaled: PublishedConstants::pi_bits() }
    }

    /// pi/2 as a rational, within `2^-20001` of the truth.
    fn half_pi(&self) -> ExactRational {
        ExactRational::dyadic(false, &self.pi_scaled, -20_001)
    }

    /// `2^power` as a rational.
    fn power_of_two(power: i64) -> ExactRational {
        ExactRational::dyadic(false, &BigUint::from(1u32), power)
    }

    /// `|value|`.
    fn absolute(value: &ExactRational) -> ExactRational {
        if value.is_negative() { value.negated() } else { value.clone() }
    }

    /// The `f128` arguments of `half_pi.tsv`: the binary128 nearest to odd multiples of pi/2.
    fn worst_cases() -> Vec<u128> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/vectors/half_pi.tsv");
        fs::read_to_string(path).unwrap_or_default().lines().filter(|line| !line.starts_with('#')).filter_map(|line| u128::from_str_radix(line.split_once('\t')?.1, 16).ok()).collect()
    }

    /// A random finite pattern: random bits, a window of exponents around 1, or any exponent.
    fn random_argument(&mut self) -> u128 {
        let word = (self.random.next_bits() as u128) << 64 | self.random.next_bits() as u128;
        let field = match self.random.next_bits() % 3 {
            0 => (word >> 112) & 0x7FFE,
            1 => 0x3FFF - 20 + (self.random.next_bits() % 60) as u128,
            _ => (self.random.next_bits() % 0x7FFE) as u128,
        };
        (word & (1u128 << 127 | ((1u128 << 112) - 1))) | field << 112
    }

    /// The exact reduction of `x` against the 20 000-bit pi: the turns `n`, the remainder
    /// `r` with `|r| <= pi/4`, and whether `r` is so close to `+-pi/4` that `n` is ambiguous.
    fn exact_reduction(&self, x: &ExactRational) -> (BigInt, ExactRational, bool) {
        let half_pi = self.half_pi();
        let turns = x.divided_by(&half_pi).map(|quotient| quotient.floor()).unwrap_or_default();
        let remainder = x.remainder_euclid(&half_pi);
        let twice = remainder.plus(&remainder);
        let ambiguous = Self::absolute(&twice.minus(&half_pi)).compare(&Self::power_of_two(-200)) == Ordering::Less;
        if twice.compare(&half_pi) == Ordering::Greater { (turns + 1, remainder.minus(&half_pi), ambiguous) } else { (turns, remainder, ambiguous) }
    }

    /// Checks one argument at each precision: the interval meets the exact remainder, the quadrant
    /// agrees, and the interval is no wider than `2^-precision` relative.
    fn check(&self, bits: u128, precisions: &[u32]) {
        let exact = DirectedScenario::rational(&MultiPrecision::from_quad_bits(bits).unwrap_or_else(MultiPrecision::zero));
        let reduction = self.exact_reduction(&exact);
        for precision in precisions {
            Self::check_precision(bits, *precision, &reduction);
        }
    }

    /// Checks one argument at one precision against its exact reduction.
    fn check_precision(bits: u128, precision: u32, reduction: &(BigInt, ExactRational, bool)) {
        let reduced = ArgumentReduction::quarter_turns(Float128::from_bits(bits), precision);
        assert!(reduced.is_some(), "{bits:#x} is finite and must reduce");
        let Some(reduced) = reduced else { return };
        let (turns, remainder, ambiguous) = (&reduction.0, &reduction.1, reduction.2);
        let (lower, upper) = (DirectedScenario::rational(reduced.remainder.lower()), DirectedScenario::rational(reduced.remainder.upper()));
        // The oracle's remainder is within |n| * 2^-20001 of the true one.
        let error = ExactRational::dyadic(false, &(turns.magnitude() + 1u32), -20_001);
        if ambiguous {
            return;
        }
        assert_ne!(lower.compare(&remainder.plus(&error)), Ordering::Greater, "remainder interval of {bits:#x} at {precision} bits lies above the exact remainder");
        assert_ne!(upper.compare(&remainder.minus(&error)), Ordering::Less, "remainder interval of {bits:#x} at {precision} bits lies below the exact remainder");
        let modulus = BigInt::from(4);
        let quadrant = (((turns % &modulus) + &modulus) % &modulus).to_u64_digits().1.first().copied().unwrap_or(0);
        assert_eq!(u64::from(reduced.quadrant), quadrant, "quadrant of {bits:#x}");
        let relative_width = upper.minus(&lower).times(&Self::power_of_two(precision as i64));
        assert_ne!(relative_width.compare(&Self::absolute(remainder)), Ordering::Greater, "remainder of {bits:#x} at {precision} bits is wider than 2^-{precision} relative");
    }

    /// Checks `x = k ln 2 + r` for an `f128` value at one precision against ln 2 at 1000 decimals.
    fn check_ln_two(&self, bits: u128, precision: u32) {
        let Some(value) = MultiPrecision::from_quad_bits(bits) else { return };
        let point = Interval::point(value, precision + 64);
        let steps = ArgumentReduction::ln_two_steps(&point, precision);
        assert!(steps.is_some(), "{bits:#x} must reduce by ln 2");
        let Some(steps) = steps else { return };
        let ln_two = PublishedConstants::truncated("ln2");
        let exact = DirectedScenario::rational(point.lower());
        let reconstructed = exact.minus(&ln_two.times(&ExactRational { numerator: BigInt::from(steps.steps), denominator: BigUint::from(1u32) }));
        let slack = ExactRational::decimal(false, &BigUint::from(steps.steps.unsigned_abs() + 1), -1000);
        let (lower, upper) = (DirectedScenario::rational(steps.remainder.lower()), DirectedScenario::rational(steps.remainder.upper()));
        assert_ne!(lower.compare(&reconstructed.plus(&slack)), Ordering::Greater, "ln 2 remainder of {bits:#x} lies above");
        assert_ne!(upper.compare(&reconstructed.minus(&slack)), Ordering::Less, "ln 2 remainder of {bits:#x} lies below");
        let half_ln_two = ln_two.times(&Self::power_of_two(-1));
        let margin = ExactRational::decimal(false, &BigUint::from(1u32), -900);
        assert_ne!(Self::absolute(&reconstructed).compare(&half_ln_two.plus(&margin)), Ordering::Greater, "|r| <= ln 2 / 2 for {bits:#x}");
        assert_ne!(upper.minus(&lower).compare(&Self::power_of_two(-(precision as i64))), Ordering::Greater, "ln 2 remainder of {bits:#x} wider than 2^-{precision}");
    }
}

#[test]
fn reduction_keeps_full_precision_for_every_finite_argument() {
    let mut scenario = ReductionScenario::new(102);
    let mut arguments = ReductionScenario::worst_cases();
    arguments.extend([0x7FFE_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFFu128, 0xFFFE_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF, 1, 0x3FFF_0000_0000_0000_0000_0000_0000_0000, 0x3FFE_0000_0000_0000_0000_0000_0000_0000, 0x4000_0000_0000_0000_0000_0000_0000_0000, 0x3FFE_921F_B544_42D1_8469_898C_C517_01B8]);
    for _ in 0..600 {
        arguments.push(scenario.random_argument());
    }
    for bits in arguments {
        scenario.check(bits, &[160, 400]);
    }
    for _ in 0..500 {
        let value = (scenario.random_argument() & !(0x7FFFu128 << 112)) | (0x3FFF - 20 + (scenario.random.next_bits() % 35) as u128) << 112;
        scenario.check_ln_two(value, 160);
        scenario.check_ln_two(value, 400);
    }
}

/// Synthetic evaluations around a midpoint of binary128, to watch the Ziv loop.
struct MidpointScenario;

impl MidpointScenario {
    /// `1 + 2^-113`: the midpoint of 1 and its successor `1 + 2^-112`.
    fn midpoint() -> MultiPrecision {
        MultiPrecision::exact(false, &[1, 1 << 49], -113)
    }

    /// The interval `[midpoint + low, midpoint + high]` for offsets given as powers of two (sign, exponent).
    fn around(low: (bool, i64), high: (bool, i64), precision: u32) -> Interval {
        let offset = |(negative, exponent): (bool, i64)| MultiPrecision::exact(negative, &[1], exponent);
        let lower = Self::midpoint().add(&offset(low), 2_000, Rounding::Down);
        let upper = Self::midpoint().add(&offset(high), 2_000, Rounding::Up);
        Interval::new(lower, upper, precision)
    }
}

#[test]
fn ziv_rounding_raises_precision_until_the_bounds_agree() {
    let successor = Float128::from_bits(0x3FFF_0000_0000_0000_0000_0000_0000_0001);
    // Straddles the midpoint at 160 and 320 bits, lies above it at 640: three evaluations.
    let mut evaluations = Vec::new();
    let rounded = ZivRounding::round(None, |precision| {
        evaluations.push(precision);
        Some(if precision < 640 { MidpointScenario::around((true, -130), (false, -130), precision) } else { MidpointScenario::around((false, -300), (false, -290), precision) })
    });
    assert_eq!(rounded.map(Float128::to_bits), Some(successor.to_bits()), "the value above the midpoint rounds up");
    assert_eq!(evaluations, vec![160, 320, 640], "precision doubles until the bounds agree");
    // An exact case returns without evaluating.
    let mut calls = 0;
    let exact = ZivRounding::round(Some(Float128::ONE), |_| {
        calls += 1;
        None
    });
    assert_eq!((exact.map(Float128::to_bits), calls), (Some(Float128::ONE.to_bits()), 0), "an exact case never evaluates");
    // A midpoint with no exact case straddles at every precision: None at the cap.
    let mut attempts = 0;
    let undecided = ZivRounding::round(None, |precision| {
        attempts += 1;
        Some(MidpointScenario::around((true, -(precision as i64) - 200), (false, -(precision as i64) - 200), precision))
    });
    assert_eq!(undecided.map(Float128::to_bits), None, "an undecidable midpoint gives None at the cap");
    assert_eq!(attempts, 13, "160 doubling up to 2^20 is thirteen evaluations");
    // A function outside its domain (no interval) stops the loop at once.
    assert_eq!(ZivRounding::round(None, |_| None).map(Float128::to_bits), None);
}
