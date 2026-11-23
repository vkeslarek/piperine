//! `BinaryFormat::round` and `DecimalText` against exact rational arithmetic (`ExactRational`,
//! over `num-bigint`), and for binary32 and binary64 also against Rust's own `f32`/`f64` parsing
//! (CV-30, CV-31; IEEE 754-2019 sections 4.3.1 and 7.4).

#[path = "support/exact.rs"]
mod exact;
#[path = "support/random.rs"]
mod random;

use num_bigint::BigUint;
use piperine_numbers::format::{BinaryFormat, FloatClass};
use piperine_numbers::limbs::ToLimbs;
use exact::ExactRational;
use random::Xorshift;

/// The rounding checks of one format.
struct RoundingScenario {
    /// The format under test.
    format: BinaryFormat,
    /// The deterministic operand source.
    random: Xorshift,
}

impl RoundingScenario {
    /// A scenario for `format`.
    fn new(format: BinaryFormat) -> Self {
        RoundingScenario { format, random: Xorshift::new(u64::from(format.bits) * 977) }
    }

    /// Asserts the library rounds the exact rational to the reference bits.
    fn expect_round(&self, value: &ExactRational, context: &str) {
        let expected = value.nearest(self.format);
        let shift = 2 * (self.format.maximum_exponent as u32 + self.format.precision) + 400;
        let (magnitude, sticky) = value.scaled(shift);
        let actual = self.format.round(value.is_negative(), &magnitude, -(shift as i64), sticky);
        assert_eq!(actual, expected, "{} bits: {context}: {:?}", self.format.bits, value);
    }

    /// A random significand of 1 to `bits` bits.
    fn random_significand(&mut self, bits: u64) -> BigUint {
        let width = 1 + self.random.next_u64() % bits;
        let mut value = BigUint::from(0u32);
        for _ in 0..width.div_ceil(64) {
            value = (value << 64u32) | BigUint::from(self.random.next_u64());
        }
        value >> ((width.div_ceil(64) * 64) - width)
    }

    /// A random exponent that reaches subnormals and overflow of the format.
    fn random_exponent(&mut self) -> i64 {
        let span = 2 * (self.format.maximum_exponent as i64 + self.format.precision as i64) + 20;
        (self.random.next_u64() % span as u64) as i64 - (self.format.maximum_exponent as i64 + self.format.precision as i64 + 10)
    }

    /// Every exact tie between neighbours, one above and below, for edge and random neighbours.
    fn check_ties(&mut self) {
        let precision = self.format.precision as u64;
        let mut neighbours: Vec<BigUint> = vec![BigUint::from(1u32) << (precision - 1), (BigUint::from(1u32) << precision) - 1u32, (BigUint::from(1u32) << (precision - 1)) + 1u32, BigUint::from(1u32)];
        for _ in 0..200 {
            neighbours.push((BigUint::from(1u32) << (precision - 1)) | (self.random_significand(precision) % (BigUint::from(1u32) << (precision - 1))));
        }
        let minimum = self.format.minimum_exponent() as i64 - precision as i64 + 1;
        for neighbour in neighbours {
            for exponent in [minimum, minimum + 1, 0, self.random_exponent(), self.format.maximum_exponent as i64 - precision as i64 + 1] {
                let midpoint = (&neighbour << 1u32) + 1u32;
                for negative in [false, true] {
                    self.expect_round(&ExactRational::dyadic(negative, &midpoint, exponent - 1), "tie");
                    self.expect_round(&ExactRational::dyadic(negative, &(&midpoint * 4u32 + 1u32), exponent - 3), "above tie");
                    self.expect_round(&ExactRational::dyadic(negative, &(&midpoint * 4u32 - 1u32), exponent - 3), "below tie");
                }
            }
        }
    }

    /// The extremes: the largest finite value, the first overflow, the smallest subnormal and half of it.
    fn check_extremes(&self) {
        let precision = self.format.precision as u64;
        let maximum_unit = self.format.maximum_exponent as i64 - precision as i64 + 1;
        let largest = (BigUint::from(1u32) << precision) - 1u32;
        self.expect_round(&ExactRational::dyadic(false, &largest, maximum_unit), "largest finite");
        let threshold = (&largest << 1u32) + 1u32;
        self.expect_round(&ExactRational::dyadic(false, &threshold, maximum_unit - 1), "overflow threshold (a tie rounding to even is infinity)");
        assert_eq!(self.format.round(false, &threshold.to_u64_digits(), maximum_unit - 1, false), self.format.infinity(false));
        assert_eq!(self.format.round(false, &largest.to_u64_digits(), maximum_unit, false), self.format.infinity(false) - 1);
        let smallest_unit = self.format.minimum_exponent() as i64 - precision as i64 + 1;
        self.expect_round(&ExactRational::dyadic(false, &BigUint::from(1u32), smallest_unit), "smallest subnormal");
        assert_eq!(self.format.round(false, &1u64.to_limbs(), smallest_unit, false), 1);
        self.expect_round(&ExactRational::dyadic(false, &BigUint::from(1u32), smallest_unit - 1), "half the smallest subnormal ties to zero");
        assert_eq!(self.format.round(false, &1u64.to_limbs(), smallest_unit - 1, false), 0);
        assert_eq!(self.format.round(false, &1u64.to_limbs(), smallest_unit - 1, true), 1, "above half rounds up");
        assert_eq!(self.format.round(true, &[], 0, false), self.format.sign_bit(), "negative zero keeps its sign");
    }

    /// Random dyadic and decimal rationals over the whole exponent range.
    fn check_random(&mut self, count: usize) {
        for _ in 0..count {
            let significand = self.random_significand(150);
            let exponent = self.random_exponent();
            let negative = self.random.next_u64() & 1 == 1;
            self.expect_round(&ExactRational::dyadic(negative, &significand, exponent), "random dyadic");
            let power = (self.random.next_u64() % 120) as i64 - 60;
            let digits = self.random_significand(130);
            self.expect_round(&ExactRational::decimal(negative, &digits, power * 40), "random decimal");
        }
    }

    /// Every decoded random pattern rounds back to itself.
    fn check_decode(&mut self, count: usize) {
        for _ in 0..count {
            let pattern = ((self.random.next_u64() as u128) << 64 | self.random.next_u64() as u128) & (u128::MAX >> (128 - self.format.bits));
            if let Some((negative, significand, exponent)) = self.format.decode(pattern) {
                let round_trip = self.format.round(negative, &significand.to_limbs(), exponent, false);
                assert_eq!(round_trip, pattern, "decode/round of {pattern:#x} in {} bits", self.format.bits);
            } else {
                assert!(matches!(self.format.classify(pattern), FloatClass::Infinite | FloatClass::NaN));
            }
        }
    }
}

/// Rust's own parser as the oracle for binary32 and binary64: a decimal `digits * 10^power`.
struct RustParser;

impl RustParser {
    /// The bits Rust's correctly rounded `str::parse` gives for the decimal value, in `format`.
    fn bits(format: BinaryFormat, negative: bool, digits: &BigUint, power: i64) -> u128 {
        let text = format!("{}{digits}e{power}", if negative { "-" } else { "" });
        if format == BinaryFormat::SINGLE {
            text.parse::<f32>().map(|value| value.to_bits() as u128).unwrap_or(0)
        } else {
            text.parse::<f64>().map(|value| value.to_bits() as u128).unwrap_or(0)
        }
    }
}

#[test]
fn rounding_to_every_format_is_nearest_ties_to_even() {
    for format in [BinaryFormat::HALF, BinaryFormat::SINGLE, BinaryFormat::DOUBLE, BinaryFormat::QUAD] {
        let mut scenario = RoundingScenario::new(format);
        scenario.check_extremes();
        scenario.check_ties();
        scenario.check_random(20_000);
        scenario.check_decode(20_000);
    }
    let mut random = Xorshift::new(94);
    for format in [BinaryFormat::SINGLE, BinaryFormat::DOUBLE] {
        for _ in 0..20_000 {
            let digits = BigUint::from(random.next_u64() >> (random.next_u64() % 60)) * BigUint::from(1_000u32).pow((random.next_u64() % 3) as u32);
            let power = (random.next_u64() % 100) as i64 - 60;
            let negative = random.next_u64() & 1 == 1;
            let rational = ExactRational::decimal(negative, &digits, power);
            let (expected, nearest) = (RustParser::bits(format, negative, &digits, power), rational.nearest(format));
            // Rust parses a zero as a signed zero; the reference gives +0 for any zero.
            let both_zero = digits == BigUint::from(0u32);
            assert!(both_zero || expected == nearest, "fixture vs Rust {} bits: {rational:?}", format.bits);
        }
    }
}
