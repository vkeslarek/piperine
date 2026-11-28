//! `BinaryFormat::round` and `DecimalText` against exact rational arithmetic (`ExactRational`,
//! over `num-bigint`), and for binary32 and binary64 also against Rust's own `f32`/`f64` parsing
//! (CV-30, CV-31; IEEE 754-2019 sections 4.3.1 and 7.4).

use crate::exact;
use crate::random;

use num_bigint::BigUint;
use num_bigint::BigInt;
use piperine_numbers::decimal::DecimalText;
use piperine_numbers::format::{BinaryFormat, FloatClass};
use piperine_numbers::limbs::ToLimbs;
use exact::ExactRational;
use random::Xorshift;

/// Big-integer helper shared with the integer targets.
struct BigOracle;

impl BigOracle {
    /// The `BigUint` of a little-endian limb slice.
    fn value(limbs: &[u64]) -> BigUint {
        let mut value = BigUint::from(0u32);
        for limb in limbs.iter().rev() {
            value = (value << 64u32) | BigUint::from(*limb);
        }
        value
    }
}

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

/// Decimal text reading and printing against exact rationals and Rust's own `str::parse` and
/// `{:e}` formatting (A.8 rule 8, QD6).
struct TextScenario {
    /// The deterministic operand source.
    random: Xorshift,
}

impl TextScenario {
    /// A scenario with a seeded generator.
    fn new(seed: u64) -> Self {
        TextScenario { random: Xorshift::new(seed) }
    }

    /// A random decimal text with its exact value: up to 40 digits, an optional fraction point
    /// and an optional exponent. Returns the text and `(negative, digits, power)`.
    fn random_text(&mut self) -> (String, bool, BigUint, i64) {
        let digit_count = 1 + (self.random.next_u64() % 40) as usize;
        let digits: String = (0..digit_count).map(|_| char::from(b'0' + (self.random.next_u64() % 10) as u8)).collect();
        let point = 1 + (self.random.next_u64() % digit_count as u64) as usize;
        let mut text = String::new();
        let negative = self.random.next_u64() & 1 == 1;
        if negative {
            text.push('-');
        }
        text.push_str(&digits[..point]);
        if point < digit_count {
            text.push('.');
            text.push_str(&digits[point..]);
        }
        let mut power = -((digit_count - point) as i64);
        if self.random.next_u64() % 3 != 0 {
            let exponent = (self.random.next_u64() % 140) as i64 - 70;
            text.push_str(&format!("e{exponent}"));
            power += exponent;
        }
        (text, negative, digits.parse::<BigUint>().unwrap_or_default(), power)
    }

    /// Reading a text gives the nearest value of every format, and Rust's for `f32` and `f64`.
    fn check_reading(&mut self, count: usize) {
        for _ in 0..count {
            let (text, negative, digits, power) = self.random_text();
            let exact = ExactRational::decimal(negative, &digits, power);
            let zero = digits == BigUint::from(0u32);
            for format in [BinaryFormat::HALF, BinaryFormat::SINGLE, BinaryFormat::DOUBLE, BinaryFormat::QUAD] {
                let actual = DecimalText::nearest(&text, format).unwrap_or(u128::MAX);
                let expected = if zero && negative { format.sign_bit() } else { exact.nearest(format) };
                assert_eq!(actual, expected, "reading {text} in {} bits", format.bits);
            }
            assert_eq!(DecimalText::nearest(&text, BinaryFormat::SINGLE), text.parse::<f32>().ok().map(|value| value.to_bits() as u128), "Rust f32 parse of {text}");
            assert_eq!(DecimalText::nearest(&text, BinaryFormat::DOUBLE), text.parse::<f64>().ok().map(|value| value.to_bits() as u128), "Rust f64 parse of {text}");
        }
    }

    /// Every power of ten in range, and exact midpoints of 40 digits or fewer, read to the reference.
    fn check_powers_and_midpoints(&mut self) {
        for format in [BinaryFormat::HALF, BinaryFormat::QUAD] {
            let limit = if format == BinaryFormat::HALF { 25 } else { 5000 };
            for power in -limit..limit {
                let text = format!("1e{power}");
                let expected = ExactRational::decimal(false, &BigUint::from(1u32), power).nearest(format);
                assert_eq!(DecimalText::nearest(&text, format), Some(expected), "{text} in {} bits", format.bits);
            }
        }
        let precision = BinaryFormat::QUAD.precision as u64;
        for _ in 0..500 {
            // In [2^113, 2^114) binary128 values are the even integers: every odd one is a tie.
            let neighbour = (BigUint::from(1u32) << precision) + BigUint::from(2 * (self.random.next_u64() % (1 << 19)));
            let midpoint = &neighbour + 1u32;
            for (text, value) in [(midpoint.to_string(), midpoint.clone()), (format!("{midpoint}0001e-4"), midpoint.clone() * 10_000u32 + 1u32), (format!("{neighbour}9999e-4"), neighbour.clone() * 10_000u32 + 9_999u32)] {
                let exponent = if text.contains('e') { -4 } else { 0 };
                let exact = ExactRational::decimal(false, &value, exponent);
                assert_eq!(DecimalText::nearest(&text, BinaryFormat::QUAD), Some(exact.nearest(BinaryFormat::QUAD)), "reading {text}");
                assert_eq!(DecimalText::nearest(&text, BinaryFormat::HALF), Some(exact.nearest(BinaryFormat::HALF)), "reading {text} in f16");
            }
            let tie = ExactRational::decimal(false, &midpoint, 0).nearest(BinaryFormat::QUAD);
            let even = if neighbour.bit(1) { &neighbour + 2u32 } else { neighbour.clone() };
            assert_eq!(tie, ExactRational::decimal(false, &even, 0).nearest(BinaryFormat::QUAD), "the tie {midpoint} goes to the even significand");
        }
        for (text, expected) in [("2049", 0x6800u128), ("2051", 0x6802), ("2049.0000000000000000000001", 0x6801), ("65519.99999", 0x7BFF), ("65520", 0x7C00), ("1.5k", 0x65DC), ("1_0.5e0", 0x4940)] {
            assert_eq!(DecimalText::nearest(text, BinaryFormat::HALF), Some(expected), "reading {text} in f16");
        }
        assert_eq!(DecimalText::nearest("2.5m", BinaryFormat::DOUBLE), Some(0.0025f64.to_bits() as u128), "SI multiplier is exact, never a product");
        for malformed in ["", "e5", "1e", "1..2", "abc", "1e5k", "--1", "_1"] {
            assert_eq!(DecimalText::nearest(malformed, BinaryFormat::DOUBLE), None, "{malformed:?} is not a number");
        }
        assert_eq!(DecimalText::nearest("inf", BinaryFormat::HALF), Some(0x7C00));
        assert_eq!(DecimalText::nearest("-inf", BinaryFormat::HALF), Some(0xFC00));
    }

    /// The pattern a decimal `digits * 10^power` reads to, by exact rational rounding.
    fn read_exact(format: BinaryFormat, negative: bool, digits: &str, power: i64) -> u128 {
        ExactRational::decimal(negative, &digits.parse::<BigUint>().unwrap_or_default(), power).nearest(format)
    }

    /// Splits the scientific text `d.ddde<x>` into its digits and the exponent of the first digit.
    fn split(text: &str) -> (bool, String, i64) {
        let (mantissa, exponent) = text.split_once('e').unwrap_or((text, "0"));
        let negative = mantissa.starts_with('-');
        (negative, mantissa.trim_start_matches('-').replace('.', ""), exponent.parse().unwrap_or(0))
    }

    /// The shortest text of `pattern` reads back to it by exact rounding and no shorter text does.
    fn check_shortest(&self, format: BinaryFormat, pattern: u128, minimal: bool) {
        let scientific = DecimalText::shortest_scientific(pattern, format);
        let plain = DecimalText::shortest(pattern, format);
        let (negative, digits, exponent) = Self::split(&scientific);
        let finite = ExactRational::from_pattern_for_text(format, pattern);
        let Some(value) = finite else {
            assert!(scientific == "NaN" || scientific.ends_with("inf"), "{scientific}");
            return;
        };
        if value.is_zero() {
            assert_eq!(scientific, if negative { "-0e0" } else { "0e0" });
            return;
        }
        let power = exponent - digits.len() as i64 + 1;
        assert_eq!(Self::read_exact(format, negative, &digits, power), pattern, "{scientific} must read back to {pattern:#x} ({} bits)", format.bits);
        assert_eq!(DecimalText::nearest(&plain, format), Some(pattern), "plain text {plain} must read back to {pattern:#x}");
        assert_eq!(DecimalText::nearest(&scientific, format), Some(pattern), "{scientific} must read back");
        if minimal && digits.len() > 1 {
            // Neither (n-1)-digit decimal next to the value reads back: the text is the shortest.
            let shorter = digits.len() as i64 - 1;
            let unit = exponent - shorter + 1;
            let scaled = value.magnitude_over_power_of_ten(-unit);
            for candidate in [scaled.clone(), scaled + 1u32] {
                assert_ne!(Self::read_exact(format, negative, &candidate.to_string(), unit), pattern, "{scientific} is not the shortest for {pattern:#x}");
            }
        }
    }
}

#[test]
fn decimal_text_reads_to_the_nearest_value_and_prints_the_shortest() {
    let mut scenario = TextScenario::new(98);
    scenario.check_reading(20_000);
    scenario.check_powers_and_midpoints();
    for bits in 0..=u16::MAX as u128 {
        scenario.check_shortest(BinaryFormat::HALF, bits, true);
    }
    for _ in 0..20_000 {
        let word = (scenario.random.next_u64() as u128) << 64 | scenario.random.next_u64() as u128;
        let minimal = scenario.random.next_u64() % 10 == 0;
        scenario.check_shortest(BinaryFormat::QUAD, word, minimal);
        let single = (word as u32) as u128;
        let double = (word >> 64) as u64 as u128;
        for (format, pattern, rust) in [(BinaryFormat::SINGLE, single, format!("{:e}", f32::from_bits(single as u32))), (BinaryFormat::DOUBLE, double, format!("{:e}", f64::from_bits(double as u64)))] {
            assert_eq!(DecimalText::shortest_scientific(pattern, format), rust, "shortest text of {pattern:#x} in {} bits vs Rust", format.bits);
            scenario.check_shortest(format, pattern, false);
        }
    }
    assert_eq!(DecimalText::shortest(0x3C00, BinaryFormat::HALF), "1");
    assert_eq!(DecimalText::shortest(0x3800, BinaryFormat::HALF), "0.5");
    assert_eq!(DecimalText::shortest(0x7BFF, BinaryFormat::HALF), "65500");
    assert_eq!(DecimalText::shortest(0x0001, BinaryFormat::HALF), "6e-8");
    assert_eq!(DecimalText::shortest(0x7C00, BinaryFormat::HALF), "inf");
    assert_eq!(DecimalText::shortest(0xFC00, BinaryFormat::HALF), "-inf");
    assert_eq!(DecimalText::shortest(0x7E00, BinaryFormat::HALF), "NaN");
    assert_eq!(DecimalText::shortest(0x8000, BinaryFormat::HALF), "-0");
    assert_eq!(DecimalText::shortest(0.1f64.to_bits() as u128, BinaryFormat::DOUBLE), "0.1");
    assert_eq!(DecimalText::shortest(1e21f64.to_bits() as u128, BinaryFormat::DOUBLE), "1e21");
    let integers = [("0xFF", 8, false, Some("255")), ("256", 8, false, None), ("-128", 8, true, Some("128")), ("-129", 8, true, None), ("127", 8, true, Some("127")), ("128", 8, true, None), ("-1", 8, false, None), ("1_000", 16, false, Some("1000")), ("0b1010", 4, false, Some("10")), ("0o777", 9, false, Some("511")), ("0o1000", 9, false, None), ("", 8, false, None), ("0x", 8, false, None), ("12a", 16, false, None)];
    for (text, width, signed, expected) in integers {
        let actual = DecimalText::integer(text, width, signed).map(|limbs| BigOracle::value(&limbs).to_string());
        assert_eq!(actual.as_deref(), expected, "integer {text:?} as {}{width}", if signed { "i" } else { "u" });
    }
    let maximum = ((BigInt::from(1u32) << 128u32) - 1u32).to_string();
    assert_eq!(DecimalText::integer(&maximum, 128, false).map(|limbs| BigOracle::value(&limbs).to_string()), Some(maximum.clone()));
    assert_eq!(DecimalText::integer(&(BigInt::from(1u32) << 128u32).to_string(), 128, false), None);
    let wide = (BigInt::from(1u32) << 999u32) - 1u32;
    assert_eq!(DecimalText::integer(&format!("-{}", wide.clone() + 1u32), 1000, true).map(|limbs| BigOracle::value(&limbs).bits()), Some(1000));
    assert_eq!(DecimalText::integer_text(true, &[5]), "-5");
    assert_eq!(DecimalText::integer_text(false, &[]), "0");
}
