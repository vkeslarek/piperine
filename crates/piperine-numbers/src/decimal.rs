//! `DecimalText`: decimal literals to the nearest value of any format, and the host's text.
//!
//! Reading is exact (A.8 rule 8): the decimal value is an integer over a power of ten, converted
//! with [`BinaryFormat::round`] and a sticky bit, never through an intermediate `f64`. Printing is
//! the shortest decimal that reads back to the same bits (QD6), and among those the closest to
//! the value, as Rust prints `f32` and `f64`.

use super::format::{BinaryFormat, FloatClass};
use super::limbs::{Limbs, ToLimbs};
use core::cmp::Ordering;

/// SI multipliers of Appendix A.2 and their decimal exponents.
const SI_SUFFIXES: [(char, i64); 10] = [('T', 12), ('G', 9), ('M', 6), ('k', 3), ('m', -3), ('u', -6), ('n', -9), ('p', -12), ('f', -15), ('a', -18)];

/// A decimal number as written: `(-1)^negative * digits * 10^exponent`.
struct ParsedDecimal {
    /// The sign.
    negative: bool,
    /// The significant digits as an integer.
    digits: Vec<u64>,
    /// The number of decimal digits of `digits` (0 for zero).
    digit_count: i64,
    /// The power of ten the digits are scaled by.
    exponent: i64,
}

impl ParsedDecimal {
    /// Reads `[sign] digits [. digits] [e [sign] digits | SI letter]`, underscores between digits.
    fn parse(text: &str) -> Option<ParsedDecimal> {
        let (negative, rest) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text.strip_prefix('+').unwrap_or(text)),
        };
        let mut digits: Vec<u64> = Vec::new();
        let mut digit_count = 0i64;
        let mut fraction_digits = 0i64;
        let mut seen_digit = false;
        let mut in_fraction = false;
        let mut chars = rest.char_indices().peekable();
        let mut exponent_text: Option<&str> = None;
        let mut multiplier = 0i64;
        while let Some((index, character)) = chars.next() {
            match character {
                '0'..='9' => {
                    seen_digit = true;
                    digits = digits.times_small_plus(10, character as u64 - '0' as u64);
                    if !digits.is_empty() {
                        digit_count += 1;
                    }
                    if in_fraction {
                        fraction_digits += 1;
                    }
                }
                '_' if seen_digit => {}
                '.' if !in_fraction && seen_digit => in_fraction = true,
                'e' | 'E' if seen_digit => {
                    exponent_text = Some(&rest[index + 1..]);
                    break;
                }
                other if seen_digit && chars.peek().is_none() => multiplier = SI_SUFFIXES.iter().find(|(letter, _)| *letter == other)?.1,
                _ => return None,
            }
        }
        if !seen_digit {
            return None;
        }
        let exponent = match exponent_text {
            Some(text) => Self::exponent(text)?,
            None => 0,
        };
        Some(ParsedDecimal { negative, digits, digit_count, exponent: exponent + multiplier - fraction_digits })
    }

    /// The value of an exponent field: an optionally signed run of digits.
    fn exponent(text: &str) -> Option<i64> {
        let (negative, digits) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text.strip_prefix('+').unwrap_or(text)),
        };
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        let magnitude = digits.parse::<i64>().unwrap_or(i64::MAX / 4).min(i64::MAX / 4);
        Some(if negative { -magnitude } else { magnitude })
    }
}

/// Decimal text reading and printing for every binary format.
pub struct DecimalText;

impl DecimalText {
    /// `10^power` as a magnitude.
    fn power_of_ten(power: u64) -> Vec<u64> {
        let mut result = 1u64.to_limbs();
        let mut base = 10u64.to_limbs();
        let mut remaining = power;
        while remaining > 0 {
            if remaining & 1 == 1 {
                result = result.times(&base);
            }
            remaining >>= 1;
            if remaining > 0 {
                base = base.times(&base);
            }
        }
        result
    }

    /// The pattern nearest to `(-1)^negative * digits * 10^exponent`, ties to even.
    fn nearest_of(negative: bool, digits: &[u64], digit_count: i64, exponent: i64, format: BinaryFormat) -> u128 {
        if digits.is_zero_magnitude() {
            return if negative { format.sign_bit() } else { 0 };
        }
        let position = digit_count + exponent;
        if position > 4_934 {
            return format.infinity(negative);
        }
        if position < -4_970 {
            return if negative { format.sign_bit() } else { 0 };
        }
        if exponent >= 0 {
            return format.round(negative, &digits.times(&Self::power_of_ten(exponent as u64)), 0, false);
        }
        let denominator = Self::power_of_ten((-exponent) as u64);
        let shift = denominator.bit_length() + format.precision as u64 + 4;
        let (quotient, remainder) = digits.shifted_left(shift).divide_with_remainder(&denominator).unwrap_or_default();
        format.round(negative, &quotient, -(shift as i64), !remainder.is_empty())
    }

    /// The pattern of the decimal literal `text` in `format`: the nearest value, ties to even
    /// (A.8 rule 8). `inf`, `-inf` and `NaN` (any case) are read as themselves. `None` when the
    /// text is not a decimal number.
    pub fn nearest(text: &str, format: BinaryFormat) -> Option<u128> {
        match text.to_ascii_lowercase().as_str() {
            "inf" | "+inf" | "infinity" | "+infinity" => return Some(format.infinity(false)),
            "-inf" | "-infinity" => return Some(format.infinity(true)),
            "nan" | "+nan" | "-nan" => return Some(format.quiet_nan()),
            _ => {}
        }
        let parsed = ParsedDecimal::parse(text)?;
        Some(Self::nearest_of(parsed.negative, &parsed.digits, parsed.digit_count, parsed.exponent, format))
    }

    /// The integer literal `text` (decimal, `0x`, `0o` or `0b`, underscores allowed, `-` only when
    /// `signed`) as the two's complement pattern of `width` bits in normalized limbs; `None` when
    /// the text is malformed or the value lies outside the type (A.2 rule 8).
    pub fn integer(text: &str, width: u32, signed: bool) -> Option<Vec<u64>> {
        let (negative, rest) = match text.strip_prefix('-') {
            Some(rest) if signed => (true, rest),
            Some(_) => return None,
            None => (false, text),
        };
        let lowered = rest.to_ascii_lowercase();
        let (radix, digits) = if let Some(body) = lowered.strip_prefix("0x") {
            (16u64, body.to_string())
        } else if let Some(body) = lowered.strip_prefix("0o") {
            (8, body.to_string())
        } else if let Some(body) = lowered.strip_prefix("0b") {
            (2, body.to_string())
        } else {
            (10, lowered)
        };
        let mut magnitude: Vec<u64> = Vec::new();
        let mut seen_digit = false;
        for character in digits.chars() {
            if character == '_' && seen_digit {
                continue;
            }
            let value = character.to_digit(radix as u32)? as u64;
            seen_digit = true;
            magnitude = magnitude.times_small_plus(radix, value);
        }
        if !seen_digit {
            return None;
        }
        let length = magnitude.bit_length();
        let width = width as u64;
        let fits = match (signed, negative) {
            (false, _) => length <= width,
            (true, false) => length < width,
            (true, true) => length < width || (length == width && magnitude.trailing_zero_bits() == width - 1),
        };
        if !fits {
            return None;
        }
        Some(if negative { 1u64.to_limbs().shifted_left(width).minus(&magnitude).unwrap_or_default() } else { magnitude })
    }

    /// The exact decimal text of an integer given by sign and magnitude.
    pub fn integer_text(negative: bool, magnitude: &[u64]) -> String {
        let digits = magnitude.to_decimal_string();
        if negative && !magnitude.is_zero_magnitude() { format!("-{digits}") } else { digits }
    }

    /// The text the host shows for a float pattern: the shortest decimal that reads back to the
    /// same bits, in plain notation for decimal exponents -5 to 16 and as `d.ddde<x>` outside it.
    /// `inf`, `-inf` and `NaN` for the special values.
    pub fn shortest(pattern: u128, format: BinaryFormat) -> String {
        match ShortestDigits::of(pattern, format) {
            Some((negative, digits, exponent)) if !(-5..17).contains(&exponent) => Self::scientific(negative, &digits, exponent),
            Some((negative, digits, exponent)) => Self::plain(negative, &digits, exponent),
            None => Self::special(pattern, format),
        }
    }

    /// The shortest text always in the form of Rust's `{:e}`: `d.ddde<x>`, `0e0`, `inf`, `NaN`.
    pub fn shortest_scientific(pattern: u128, format: BinaryFormat) -> String {
        match ShortestDigits::of(pattern, format) {
            Some((negative, digits, exponent)) => Self::scientific(negative, &digits, exponent),
            None => Self::special(pattern, format),
        }
    }

    /// The text of a pattern that is zero, infinite or NaN (zero is `0` or `-0`).
    fn special(pattern: u128, format: BinaryFormat) -> String {
        let sign = if format.is_negative(pattern) { "-" } else { "" };
        match format.classify(pattern) {
            FloatClass::NaN => "NaN".to_string(),
            FloatClass::Infinite => format!("{sign}inf"),
            _ => format!("{sign}0"),
        }
    }

    /// `d.ddde<x>` for digits and the exponent of the first digit.
    fn scientific(negative: bool, digits: &str, exponent: i64) -> String {
        let sign = if negative { "-" } else { "" };
        let (first, rest) = digits.split_at(1);
        if rest.is_empty() { format!("{sign}{first}e{exponent}") } else { format!("{sign}{first}.{rest}e{exponent}") }
    }

    /// Plain notation for digits and the exponent of the first digit (no trailing `.0`).
    fn plain(negative: bool, digits: &str, exponent: i64) -> String {
        let sign = if negative { "-" } else { "" };
        let length = digits.len() as i64;
        if exponent < 0 {
            return format!("{sign}0.{}{digits}", "0".repeat((-exponent - 1) as usize));
        }
        if length <= exponent + 1 {
            return format!("{sign}{digits}{}", "0".repeat((exponent + 1 - length) as usize));
        }
        let (whole, fraction) = digits.split_at(exponent as usize + 1);
        format!("{sign}{whole}.{fraction}")
    }
}

/// The search for the shortest digits of a finite nonzero value.
struct ShortestDigits {
    /// The 36-digit floor of the value scaled to `10^35 <= x < 10^36`.
    scaled: u128,
    /// Whether the scaling dropped a nonzero remainder.
    has_remainder: bool,
    /// The decimal exponent of the leading digit.
    exponent: i64,
    /// The scaled lower boundary of the rounding interval and whether it is included.
    lower: Boundary,
    /// The scaled upper boundary of the rounding interval and whether it is included.
    upper: Boundary,
    /// The remainder of the division that produced `scaled`, and its denominator, to rank candidates.
    fraction: (Vec<u64>, Vec<u64>),
}

/// A boundary of the rounding interval as `numerator / denominator`, scaled like the digits.
struct Boundary {
    /// The numerator.
    numerator: Vec<u64>,
    /// The denominator.
    denominator: Vec<u64>,
    /// Whether a value equal to the boundary rounds to the pattern (ties to even).
    included: bool,
}

impl Boundary {
    /// The ordering of the integer `value` against the boundary.
    fn compare(&self, value: u128) -> Ordering {
        value.to_limbs().times(&self.denominator).compare_magnitude(&self.numerator)
    }
}

impl ShortestDigits {
    /// The sign, digits and exponent of the first digit of the shortest text of a finite pattern,
    /// zero giving digits `0` and exponent 0; `None` for infinity and NaN.
    fn of(pattern: u128, format: BinaryFormat) -> Option<(bool, String, i64)> {
        let (negative, significand, exponent) = format.decode(pattern)?;
        if significand == 0 {
            return Some((negative, "0".to_string(), 0));
        }
        let search = Self::locate(format, significand, exponent)?;
        let count = search.shortest_count();
        let (digits, decimal_exponent) = search.digits(count);
        Some((negative, digits, decimal_exponent))
    }

    /// Scales the value to 36 digits and computes the scaled rounding interval.
    fn locate(format: BinaryFormat, significand: u128, exponent: i64) -> Option<ShortestDigits> {
        let top = exponent + 127 - significand.leading_zeros() as i64;
        let mut decimal = ((top as f64) * core::f64::consts::LOG10_2).floor() as i64;
        loop {
            let (numerator, denominator) = Self::scaled_fraction(significand, exponent, 35 - decimal);
            let (quotient, remainder) = numerator.divide_with_remainder(&denominator)?;
            let value = quotient.to_u128()?;
            if value >= 10u128.pow(36) {
                decimal += 1;
                continue;
            }
            if value < 10u128.pow(35) {
                decimal -= 1;
                continue;
            }
            let (lower, upper) = Self::interval(format, significand, exponent, decimal);
            return Some(ShortestDigits { scaled: value, has_remainder: !remainder.is_empty(), exponent: decimal, lower, upper, fraction: (remainder, denominator) });
        }
    }

    /// `significand * 2^exponent * 10^power` as a numerator and a denominator.
    fn scaled_fraction(significand: u128, exponent: i64, power: i64) -> (Vec<u64>, Vec<u64>) {
        let mut numerator = significand.to_limbs();
        let mut denominator = 1u64.to_limbs();
        if power >= 0 {
            numerator = numerator.times(&DecimalText::power_of_ten(power as u64));
        } else {
            denominator = DecimalText::power_of_ten((-power) as u64);
        }
        if exponent >= 0 {
            numerator = numerator.shifted_left(exponent as u64);
        } else {
            denominator = denominator.shifted_left((-exponent) as u64);
        }
        (numerator, denominator)
    }

    /// The boundaries of the values that round to `significand * 2^exponent`, scaled by `10^(35 - decimal)`.
    fn interval(format: BinaryFormat, significand: u128, exponent: i64, decimal: i64) -> (Boundary, Boundary) {
        let power_of_two = significand == 1u128 << format.fraction_bits() && exponent > format.minimum_exponent() as i64 - format.fraction_bits() as i64;
        let included = significand & 1 == 0;
        let (below, above) = if power_of_two { (1u128, 2u128) } else { (2, 2) };
        let build = |offset_numerator: u128, subtract: bool| {
            // value +- offset * 2^(exponent - 2), with the offset in quarters of a unit in the last place.
            let quarter = 4 * significand;
            let total = if subtract { quarter - offset_numerator } else { quarter + offset_numerator };
            let (numerator, denominator) = Self::scaled_fraction(total, exponent - 2, 35 - decimal);
            Boundary { numerator, denominator, included }
        };
        (build(below, true), build(above, false))
    }

    /// Whether `candidate` (an integer scaled like `scaled`) reads back to the pattern.
    fn reads_back(&self, candidate: u128) -> bool {
        let above_lower = match self.lower.compare(candidate) {
            Ordering::Greater => true,
            Ordering::Equal => self.lower.included,
            Ordering::Less => false,
        };
        let below_upper = match self.upper.compare(candidate) {
            Ordering::Less => true,
            Ordering::Equal => self.upper.included,
            Ordering::Greater => false,
        };
        above_lower && below_upper
    }

    /// The candidates of `count` digits: the floor and the ceiling, as integers scaled like `scaled`.
    fn candidates(&self, count: u32) -> (u128, Option<u128>) {
        let unit = 10u128.pow(36 - count);
        let floor = self.scaled / unit * unit;
        let exact = self.scaled % unit == 0 && !self.has_remainder;
        (floor, if exact { None } else { Some(floor + unit) })
    }

    /// The least digit count with a candidate that reads back (a read-back at `n` implies one at `n + 1`).
    fn shortest_count(&self) -> u32 {
        let (mut low, mut high) = (1u32, 36u32);
        while low < high {
            let middle = (low + high) / 2;
            let (floor, ceiling) = self.candidates(middle);
            if self.reads_back(floor) || ceiling.is_some_and(|value| self.reads_back(value)) {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        low
    }

    /// The digits (trailing zeros dropped) of the best candidate of `count` digits and the
    /// exponent of the first digit: the one that reads back, the closer when both do.
    fn digits(&self, count: u32) -> (String, i64) {
        let (floor, ceiling) = self.candidates(count);
        let chosen = match ceiling {
            Some(up) if self.reads_back(up) && (!self.reads_back(floor) || self.upper_is_closer(floor, up)) => up,
            _ => floor,
        };
        let mut exponent = self.exponent;
        let mut text = chosen.to_string();
        if chosen >= 10u128.pow(36) {
            exponent += 1;
            text.truncate(36);
        }
        let trimmed = text.trim_end_matches('0');
        (if trimmed.is_empty() { "0".to_string() } else { trimmed.to_string() }, exponent)
    }

    /// Whether the ceiling candidate is at least as close to the value as the floor (a tie goes
    /// up, as Rust's shortest formatting breaks it).
    fn upper_is_closer(&self, floor: u128, ceiling: u128) -> bool {
        // distance to floor: (scaled - floor) + fraction; to ceiling: (ceiling - scaled) - fraction.
        // Compare 2 * (scaled - floor) + 2 * fraction with (ceiling - floor) over the denominator.
        let (remainder, denominator) = &self.fraction;
        let lower_twice = (2 * (self.scaled - floor)).to_limbs().times(denominator).plus(&remainder.times(&2u64.to_limbs()));
        let gap = (ceiling - floor).to_limbs().times(denominator);
        lower_twice.compare_magnitude(&gap) != Ordering::Less
    }
}
