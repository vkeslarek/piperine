//! `NumberConversion`: `as` between every width of float and integer (section 1.4.5).
//!
//! Each conversion is rounded once from its source, never through an intermediate format: a float
//! to a float and an integer to a float are the nearest value, ties to even, overflow to
//! infinity, NaN to NaN; a float to an integer truncates toward zero, clamps to the target's
//! range, and maps NaN to 0.

use super::format::{BinaryFormat, FloatClass};
use super::limbs::{Limbs, ToLimbs};
use super::wide::{WideInt, WideUInt};

/// The owner of every `as` conversion among numeric widths.
pub struct NumberConversion;

impl NumberConversion {
    /// A float of `source` to the nearest float of `target`; NaN to the canonical NaN.
    pub fn float_to_float(source: BinaryFormat, pattern: u128, target: BinaryFormat) -> u128 {
        match source.decode(pattern) {
            Some((negative, significand, exponent)) => target.round(negative, &significand.to_limbs(), exponent, false),
            None if source.classify(pattern) == FloatClass::Infinite => target.infinity(source.is_negative(pattern)),
            None => target.quiet_nan(),
        }
    }

    /// An integer given by sign and magnitude limbs to the nearest float of `target`.
    pub fn integer_to_float(negative: bool, magnitude: &[u64], target: BinaryFormat) -> u128 {
        target.round(negative, magnitude, 0, false)
    }

    /// An unsigned integer of any width to the nearest float of `target`.
    pub fn unsigned_to_float<const N: u32, const L: usize>(value: WideUInt<N, L>, target: BinaryFormat) -> u128 {
        Self::integer_to_float(false, value.limbs(), target)
    }

    /// A signed integer of any width to the nearest float of `target`.
    pub fn signed_to_float<const N: u32, const L: usize>(value: WideInt<N, L>, target: BinaryFormat) -> u128 {
        Self::integer_to_float(value.is_negative(), value.magnitude().limbs(), target)
    }

    /// The truncated magnitude of a finite float, or `None` when it is certainly wider than `limit` bits.
    fn truncated_magnitude(significand: u128, exponent: i64, limit: u64) -> Option<Vec<u64>> {
        let limbs = significand.to_limbs();
        if exponent >= 0 {
            if exponent as u64 > limit {
                return if limbs.is_empty() { Some(limbs) } else { None };
            }
            return Some(limbs.shifted_left(exponent as u64));
        }
        Some(limbs.shifted_right((-exponent) as u64))
    }

    /// A float to an unsigned integer of `N` bits: truncate, clamp to `0..=2^N - 1`, NaN to 0.
    pub fn float_to_unsigned<const N: u32, const L: usize>(source: BinaryFormat, pattern: u128) -> WideUInt<N, L> {
        match source.decode(pattern) {
            Some((negative, significand, exponent)) => match Self::truncated_magnitude(significand, exponent, N as u64 + 1) {
                _ if negative => WideUInt::ZERO,
                Some(magnitude) if magnitude.bit_length() <= N as u64 => WideUInt::from_limbs(&magnitude),
                _ => WideUInt::maximum(),
            },
            None if source.classify(pattern) == FloatClass::Infinite && !source.is_negative(pattern) => WideUInt::maximum(),
            None => WideUInt::ZERO,
        }
    }

    /// A float to a signed integer of `N` bits: truncate, clamp to `-2^(N-1)..=2^(N-1) - 1`, NaN to 0.
    pub fn float_to_signed<const N: u32, const L: usize>(source: BinaryFormat, pattern: u128) -> WideInt<N, L> {
        let clamp = |negative: bool| if negative { WideInt::minimum() } else { WideInt::maximum() };
        match source.decode(pattern) {
            Some((negative, significand, exponent)) => match Self::truncated_magnitude(significand, exponent, N as u64 + 1) {
                Some(magnitude) => {
                    let length = magnitude.bit_length();
                    let fits = length < N as u64 || (negative && length == N as u64 && magnitude.trailing_zero_bits() == N as u64 - 1);
                    if fits { WideInt::from_magnitude(negative, WideUInt::from_limbs(&magnitude)) } else { clamp(negative) }
                }
                None => clamp(negative),
            },
            None if source.classify(pattern) == FloatClass::Infinite => clamp(source.is_negative(pattern)),
            None => WideInt::ZERO,
        }
    }
}
