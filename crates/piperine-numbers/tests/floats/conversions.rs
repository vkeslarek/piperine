//! `NumberConversion` (`as`, section 1.4.5) against exact rationals (`ExactRational` over
//! `num-bigint`) for every pair of widths, and against Rust's own `as` for the pairs Rust has:
//! `f32` and `f64` to each other, and `u64`/`i64` with `f32`/`f64` (CV-30, CV-31, CV-32).

use crate::exact;

use exact::ExactRational;
use num_bigint::{BigInt, BigUint, Sign};
use piperine_numbers::convert::NumberConversion;
use piperine_numbers::format::BinaryFormat;
use piperine_numbers::wide::{WideInt, WideUInt};
use piperine_device_fixtures::random::Xorshift;

/// The four float formats, with a name for failure messages.
const FORMATS: [(&str, BinaryFormat); 4] = [("f16", BinaryFormat::HALF), ("f32", BinaryFormat::SINGLE), ("f64", BinaryFormat::DOUBLE), ("f128", BinaryFormat::QUAD)];

/// Operand generation and the exact reference for conversions.
struct ConversionScenario {
    /// The deterministic operand source.
    random: Xorshift,
}

impl ConversionScenario {
    /// A scenario with a seeded generator.
    fn new(seed: u64) -> Self {
        ConversionScenario { random: Xorshift::new(seed) }
    }

    /// 128 random bits.
    fn word(&mut self) -> u128 {
        (self.random.next_u64() as u128) << 64 | self.random.next_u64() as u128
    }

    /// Whether a pattern of `format` is a NaN.
    fn is_nan(format: BinaryFormat, pattern: u128) -> bool {
        ExactRational::from_pattern(format, pattern).is_none() && pattern & !format.sign_bit() != format.infinity(false)
    }

    /// Patterns of `format`: the special values, random bits, and values near `2^limit` for the
    /// integer conversions (with fractions, so truncation and ties are exercised).
    fn float_patterns(&mut self, format: BinaryFormat, limit: i64, count: usize) -> Vec<u128> {
        let precision = format.precision as u64;
        let mut patterns = vec![0, format.sign_bit(), format.infinity(false), format.infinity(true), format.quiet_nan(), 1, format.infinity(false) - 1, 1u128 << format.fraction_bits()];
        for exponent in [-3i64, -1, 0, 1, 5, 7, 8, 15, 16, 31, 32, 63, 64, 65, 127, 128, 199, 200] {
            for fraction in [0u64, 1, (1u64 << (precision.min(63) - 1)) - 1, 1u64 << (precision.min(63) - 2)] {
                let significand = (BigUint::from(1u32) << (precision - 1)) | BigUint::from(fraction);
                for negative in [false, true] {
                    let exact = ExactRational::dyadic(negative, &significand, exponent - precision as i64 + 1);
                    patterns.push(exact.nearest(format));
                }
            }
        }
        for _ in 0..count {
            patterns.push(self.word() & (u128::MAX >> (128 - format.bits)));
            let significand = BigUint::from(self.word() >> (self.random.next_u64() % 128)) | BigUint::from(1u32);
            let exponent = (self.random.next_u64() % (limit as u64 + 8)) as i64 - 4 - significand.bits() as i64;
            let exact = ExactRational::dyadic(self.random.next_u64() & 1 == 1, &significand, exponent);
            patterns.push(exact.nearest(format));
        }
        patterns
    }

    /// The reference conversion of a float pattern to another format.
    fn expected_float(source: BinaryFormat, pattern: u128, target: BinaryFormat) -> u128 {
        match ExactRational::from_pattern(source, pattern) {
            Some(value) if value.is_zero() => if source.is_negative(pattern) { target.sign_bit() } else { 0 },
            Some(value) => value.nearest(target),
            None if Self::is_nan(source, pattern) => target.quiet_nan(),
            None => target.infinity(source.is_negative(pattern)),
        }
    }

    /// Asserts two float patterns are the same value (equal bits or both NaN).
    fn assert_same_float(format: BinaryFormat, expected: u128, actual: u128, context: &str) {
        assert!((Self::is_nan(format, expected) && Self::is_nan(format, actual)) || expected == actual, "{context}: expected {expected:#x}, got {actual:#x}");
    }

    /// Float to float for every ordered pair of formats, against exact rounding and Rust's `as`.
    fn check_float_to_float(&mut self) {
        for (source_name, source) in FORMATS {
            let patterns: Vec<u128> = if source == BinaryFormat::HALF { (0..=u16::MAX as u128).collect() } else { self.float_patterns(source, 40, 20_000) };
            for (target_name, target) in FORMATS {
                for &pattern in &patterns {
                    let actual = NumberConversion::float_to_float(source, pattern, target);
                    Self::assert_same_float(target, Self::expected_float(source, pattern, target), actual, &format!("{source_name} {pattern:#x} as {target_name}"));
                    if source == BinaryFormat::SINGLE && target == BinaryFormat::DOUBLE {
                        Self::assert_same_float(target, (f32::from_bits(pattern as u32) as f64).to_bits() as u128, actual, "Rust f32 as f64");
                    }
                    if source == BinaryFormat::DOUBLE && target == BinaryFormat::SINGLE {
                        Self::assert_same_float(target, (f64::from_bits(pattern as u64) as f32).to_bits() as u128, actual, "Rust f64 as f32");
                    }
                }
            }
        }
    }

    /// The case where rounding `f64 -> f32 -> f16` differs from rounding `f64 -> f16` once.
    fn check_double_rounding_case() {
        // 1 + 2^-11 + 2^-40 lies just above the midpoint of the f16 neighbours 1 and 1 + 2^-10.
        let value = f64::from_bits(1.0f64.to_bits() + (1u64 << 41) + (1u64 << 12));
        let once = NumberConversion::float_to_float(BinaryFormat::DOUBLE, value.to_bits() as u128, BinaryFormat::HALF);
        let through_single = NumberConversion::float_to_float(BinaryFormat::SINGLE, (value as f32).to_bits() as u128, BinaryFormat::HALF);
        assert_eq!(once, 0x3C01, "rounded once, the value above the midpoint goes up");
        assert_eq!(through_single, 0x3C00, "through f32 the value is the midpoint and ties to even (the double rounding the library avoids)");
    }

    /// The reference of an exact integer to a float format.
    fn expected_integer(value: &BigInt, target: BinaryFormat) -> u128 {
        ExactRational::integer(value).nearest(target)
    }

    /// Random integers of `N` bits: edges, powers of two and their neighbours, and random widths.
    fn integers(&mut self, bits: u32, count: usize) -> Vec<BigInt> {
        let mut values = vec![BigInt::from(0), BigInt::from(1), BigInt::from(-1), (BigInt::from(1) << bits) - 1, BigInt::from(1) << (bits - 1), (BigInt::from(1) << (bits - 1)) - 1, -(BigInt::from(1) << (bits - 1))];
        for exponent in [10u32, 11, 24, 25, 52, 53, 54, 63, 64, 65, 113, 114, 127] {
            if exponent < bits {
                for delta in [-1i64, 0, 1, 2] {
                    values.push((BigInt::from(1) << exponent) + delta);
                }
            }
        }
        for _ in 0..count {
            let pattern = (BigInt::from(self.word()) << 128 | BigInt::from(self.word())) >> (self.random.next_u64() % 256);
            values.push(pattern % (BigInt::from(1) << bits));
        }
        values
    }

    /// The limbs of a non-negative `BigInt` below `2^bits`.
    fn pattern_limbs(value: &BigInt) -> Vec<u64> {
        value.to_biguint().map(|magnitude| magnitude.to_u64_digits()).unwrap_or_default()
    }

    /// Integer to float for an unsigned and a signed type of `N` bits against exact rounding.
    fn check_integer_to_float<const N: u32, const L: usize>(&mut self) {
        for value in self.integers(N, 3_000) {
            let unsigned_value = if value.sign() == Sign::Minus { (BigInt::from(1) << N) + &value } else { value.clone() };
            let unsigned = WideUInt::<N, L>::from_limbs(&Self::pattern_limbs(&unsigned_value));
            let signed = WideInt::<N, L>::from_limbs(&Self::pattern_limbs(&unsigned_value));
            let signed_value = if unsigned_value >= BigInt::from(1) << (N - 1) { &unsigned_value - (BigInt::from(1) << N) } else { unsigned_value.clone() };
            for (name, format) in FORMATS {
                let context = format!("{value} as {name} (N={N})");
                Self::assert_same_float(format, Self::expected_integer(&unsigned_value, format), NumberConversion::unsigned_to_float(unsigned, format), &format!("unsigned {context}"));
                Self::assert_same_float(format, Self::expected_integer(&signed_value, format), NumberConversion::signed_to_float(signed, format), &format!("signed {context}"));
            }
            if N == 64 {
                let (as_unsigned, as_signed) = (unsigned.low_u64(), signed.low_i128() as i64);
                Self::assert_same_float(BinaryFormat::SINGLE, (as_unsigned as f32).to_bits() as u128, NumberConversion::unsigned_to_float(unsigned, BinaryFormat::SINGLE), "Rust u64 as f32");
                Self::assert_same_float(BinaryFormat::DOUBLE, (as_unsigned as f64).to_bits() as u128, NumberConversion::unsigned_to_float(unsigned, BinaryFormat::DOUBLE), "Rust u64 as f64");
                Self::assert_same_float(BinaryFormat::SINGLE, (as_signed as f32).to_bits() as u128, NumberConversion::signed_to_float(signed, BinaryFormat::SINGLE), "Rust i64 as f32");
                Self::assert_same_float(BinaryFormat::DOUBLE, (as_signed as f64).to_bits() as u128, NumberConversion::signed_to_float(signed, BinaryFormat::DOUBLE), "Rust i64 as f64");
            }
        }
    }

    /// The reference of a float to an integer range: truncate, clamp, NaN to zero.
    fn expected_integer_of_float(source: BinaryFormat, pattern: u128, minimum: &BigInt, maximum: &BigInt) -> BigInt {
        match ExactRational::from_pattern(source, pattern) {
            Some(value) => value.truncated().max(minimum.clone()).min(maximum.clone()),
            None if Self::is_nan(source, pattern) => BigInt::from(0),
            None => if source.is_negative(pattern) { minimum.clone() } else { maximum.clone() },
        }
    }

    /// Float to integer for an unsigned and a signed type of `N` bits against exact truncation.
    fn check_float_to_integer<const N: u32, const L: usize>(&mut self) {
        let unsigned_range = (BigInt::from(0), (BigInt::from(1) << N) - 1);
        let signed_range = (-(BigInt::from(1) << (N - 1)), (BigInt::from(1) << (N - 1)) - 1);
        for (name, format) in FORMATS {
            let patterns = if format == BinaryFormat::HALF { (0..=u16::MAX as u128).collect() } else { self.float_patterns(format, N as i64 + 4, 4_000) };
            for pattern in patterns {
                let unsigned: WideUInt<N, L> = NumberConversion::float_to_unsigned(format, pattern);
                let signed: WideInt<N, L> = NumberConversion::float_to_signed(format, pattern);
                let expected_unsigned = Self::expected_integer_of_float(format, pattern, &unsigned_range.0, &unsigned_range.1);
                let expected_signed = Self::expected_integer_of_float(format, pattern, &signed_range.0, &signed_range.1);
                let actual_unsigned = BigInt::from(BigUint::from_slice(&unsigned.limbs().iter().flat_map(|limb| [*limb as u32, (*limb >> 32) as u32]).collect::<Vec<u32>>()));
                assert_eq!(actual_unsigned, expected_unsigned, "{name} {pattern:#x} as UInt<{N}>");
                let actual_signed = BigInt::from_biguint(Sign::Plus, BigUint::from_slice(&signed.sign_extended_limbs().iter().flat_map(|limb| [*limb as u32, (*limb >> 32) as u32]).collect::<Vec<u32>>()));
                let actual_signed = if signed.is_negative() { actual_signed - (BigInt::from(1) << (64 * L as u32)) } else { actual_signed };
                assert_eq!(actual_signed, expected_signed, "{name} {pattern:#x} as Int<{N}>");
                if N == 64 && format == BinaryFormat::SINGLE {
                    let value = f32::from_bits(pattern as u32);
                    assert_eq!(unsigned.low_u64(), value as u64, "Rust f32 as u64");
                    assert_eq!(signed.low_i128() as i64, value as i64, "Rust f32 as i64");
                }
                if N == 64 && format == BinaryFormat::DOUBLE {
                    let value = f64::from_bits(pattern as u64);
                    assert_eq!(unsigned.low_u64(), value as u64, "Rust f64 as u64");
                    assert_eq!(signed.low_i128() as i64, value as i64, "Rust f64 as i64");
                }
            }
        }
    }
}

#[test]
fn every_as_conversion_follows_the_table_of_1_4_5() {
    let mut scenario = ConversionScenario::new(97);
    ConversionScenario::check_double_rounding_case();
    scenario.check_float_to_float();
    scenario.check_integer_to_float::<8, 1>();
    scenario.check_integer_to_float::<64, 1>();
    scenario.check_integer_to_float::<65, 2>();
    scenario.check_integer_to_float::<128, 2>();
    scenario.check_integer_to_float::<200, 4>();
    scenario.check_float_to_integer::<8, 1>();
    scenario.check_float_to_integer::<64, 1>();
    scenario.check_float_to_integer::<65, 2>();
    scenario.check_float_to_integer::<128, 2>();
    scenario.check_float_to_integer::<200, 4>();
}
