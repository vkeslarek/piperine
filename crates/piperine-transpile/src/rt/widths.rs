//! MSL's rules over the number widths of `rt/numbers` (composite-values phase D): the operations
//! the spec makes a panic (`rt::Panic`, the library returns `Option` for them), the bytes and levels
//! of each type on a pin (`Carried`, §2), and `as` among every float and integer width
//! (`Convert`, §1.4.5), each conversion rounded once from its source.
//!
//! Embedded with `rt/numbers/*.rs` and compiled by generated code only when a design uses a new
//! width; the files here refer to their siblings as `super::`.

use core::ops::{Div, Rem};

use super::control::Panic;
use super::int::{Int, UInt};
use super::numbers::convert::NumberConversion;
use super::numbers::format::BinaryFormat;
use super::numbers::half::Float16;
use super::numbers::quad::Float128;
use super::numbers::wide::{WideInt, WideUInt};
use super::pin::{Carried, LevelWords};

impl Default for Float16 {
    /// Positive zero, the `Default` of a float (H.2).
    fn default() -> Float16 {
        Float16::ZERO
    }
}

impl Default for Float128 {
    /// Positive zero, the `Default` of a float (H.2).
    fn default() -> Float128 {
        Float128::ZERO
    }
}

impl<const N: u32, const L: usize> Default for WideUInt<N, L> {
    /// Zero, the `Default` of an integer (H.1).
    fn default() -> Self {
        WideUInt::ZERO
    }
}

impl<const N: u32, const L: usize> Default for WideInt<N, L> {
    /// Zero, the `Default` of an integer (H.1).
    fn default() -> Self {
        WideInt::ZERO
    }
}

impl Float16 {
    /// `self` limited to `lowest..=highest`; a crossed pair of bounds or a NaN bound panics (H.2 rule 4).
    pub fn clamp(self, lowest: Float16, highest: Float16) -> Float16 {
        match self.clamp_checked(lowest, highest) {
            Some(value) => value,
            None => Panic::raise(format!("`clamp` on f16 with a lower bound {} above the upper bound {}, or a NaN bound", lowest.to_f32(), highest.to_f32())),
        }
    }
}

impl Float128 {
    /// `self` limited to `lowest..=highest`; a crossed pair of bounds or a NaN bound panics (H.2 rule 4).
    pub fn clamp(self, lowest: Float128, highest: Float128) -> Float128 {
        match self.clamp_checked(lowest, highest) {
            Some(value) => value,
            None => Panic::raise(format!("`clamp` on f128 with a lower bound 0x{:x} above the upper bound 0x{:x}, or a NaN bound", lowest.to_bits(), highest.to_bits())),
        }
    }
}

impl<const N: u32, const L: usize> WideUInt<N, L> {
    /// `self` limited to `lowest..=highest`; `lowest > highest` panics (H.1 rule 3).
    pub fn clamp(self, lowest: Self, highest: Self) -> Self {
        match self.clamp_checked(lowest, highest) {
            Some(value) => value,
            None => Panic::raise(format!("`clamp` on UInt<{N}> with a lower bound {} above the upper bound {}", lowest.to_decimal_string(), highest.to_decimal_string())),
        }
    }

    /// The Euclidean remainder; a zero divisor panics naming the operation (§1 rule 11).
    pub fn rem_euclid(self, divisor: Self) -> Self {
        match self.rem_euclid_checked(divisor) {
            Some(value) => value,
            None => Panic::raise(format!("`rem_euclid` by zero: UInt<{N}> {} rem_euclid 0", self.to_decimal_string())),
        }
    }
}

impl<const N: u32, const L: usize> WideInt<N, L> {
    /// `self` limited to `lowest..=highest`; `lowest > highest` panics (H.1 rule 3).
    pub fn clamp(self, lowest: Self, highest: Self) -> Self {
        match self.clamp_checked(lowest, highest) {
            Some(value) => value,
            None => Panic::raise(format!("`clamp` on Int<{N}> with a lower bound {} above the upper bound {}", lowest.to_decimal_string(), highest.to_decimal_string())),
        }
    }

    /// The Euclidean remainder; a zero divisor panics naming the operation (§1 rule 11).
    pub fn rem_euclid(self, divisor: Self) -> Self {
        match self.rem_euclid_checked(divisor) {
            Some(value) => value,
            None => Panic::raise(format!("`rem_euclid` by zero: Int<{N}> {} rem_euclid 0", self.to_decimal_string())),
        }
    }
}

impl<const N: u32, const L: usize> Div for WideUInt<N, L> {
    type Output = WideUInt<N, L>;
    /// Truncating division; a zero divisor panics naming the operation (§1 rule 11).
    fn div(self, rhs: Self) -> Self {
        match self.checked_divide(rhs) {
            Some(value) => value,
            None => Panic::raise(format!("integer division by zero: UInt<{N}> {} / 0", self.to_decimal_string())),
        }
    }
}

impl<const N: u32, const L: usize> Rem for WideUInt<N, L> {
    type Output = WideUInt<N, L>;
    /// The remainder; a zero divisor panics naming the operation (§1 rule 11).
    fn rem(self, rhs: Self) -> Self {
        match self.checked_remainder(rhs) {
            Some(value) => value,
            None => Panic::raise(format!("integer remainder by zero: UInt<{N}> {} % 0", self.to_decimal_string())),
        }
    }
}

impl<const N: u32, const L: usize> Div for WideInt<N, L> {
    type Output = WideInt<N, L>;
    /// Truncating division, the least value over `-1` wrapping to itself; a zero divisor panics.
    fn div(self, rhs: Self) -> Self {
        match self.checked_divide(rhs) {
            Some(value) => value,
            None => Panic::raise(format!("integer division by zero: Int<{N}> {} / 0", self.to_decimal_string())),
        }
    }
}

impl<const N: u32, const L: usize> Rem for WideInt<N, L> {
    type Output = WideInt<N, L>;
    /// The remainder with the sign of the dividend; a zero divisor panics.
    fn rem(self, rhs: Self) -> Self {
        match self.checked_remainder(rhs) {
            Some(value) => value,
            None => Panic::raise(format!("integer remainder by zero: Int<{N}> {} % 0", self.to_decimal_string())),
        }
    }
}

/// The levels of the low `count` bits of `limbs`, level `k` being bit `k` (§2).
struct LimbLevels;

impl LimbLevels {
    /// `count` levels from `limbs`, low limb first.
    fn of(limbs: &[u64], count: u32) -> LevelWords {
        let mut levels = LevelWords::empty();
        let mut remaining = count;
        for limb in limbs {
            if remaining == 0 {
                break;
            }
            let take = remaining.min(64);
            levels.append(&LevelWords::of(*limb, take));
            remaining -= take;
        }
        levels
    }

    /// The `count` limbs the first levels of `words` form, low limb first.
    fn limbs(words: &LevelWords, count: usize) -> Vec<u64> {
        (0..count).map(|k| words.slice(64 * k as u32, 64).low()).collect()
    }

    /// The little-endian bytes of `limbs`.
    fn write(limbs: &[u64], out: &mut [u8]) {
        for (k, limb) in limbs.iter().enumerate() {
            out[8 * k..8 * k + 8].copy_from_slice(&limb.to_le_bytes());
        }
    }

    /// `count` limbs read from little-endian `bytes`.
    fn read(bytes: &[u8], count: usize) -> Vec<u64> {
        (0..count)
            .map(|k| {
                let mut word = [0u8; 8];
                word.copy_from_slice(&bytes[8 * k..8 * k + 8]);
                u64::from_le_bytes(word)
            })
            .collect()
    }
}

impl Carried for f32 {
    const WIDTH: usize = 4;
    fn encode(self, out: &mut [u8]) {
        out[..4].copy_from_slice(&self.to_le_bytes());
    }
    fn decode(bytes: &[u8]) -> f32 {
        let mut word = [0u8; 4];
        word.copy_from_slice(&bytes[..4]);
        f32::from_le_bytes(word)
    }
    const LEVELS: u32 = 32;
    fn to_levels(self) -> LevelWords {
        LevelWords::of(u64::from(self.to_bits()), 32)
    }
    fn from_levels(words: &LevelWords) -> f32 {
        f32::from_bits(words.low() as u32)
    }
}

impl Carried for Float16 {
    const WIDTH: usize = 2;
    fn encode(self, out: &mut [u8]) {
        out[..2].copy_from_slice(&self.to_bits().to_le_bytes());
    }
    fn decode(bytes: &[u8]) -> Float16 {
        Float16::from_bits(u16::from_le_bytes([bytes[0], bytes[1]]))
    }
    const LEVELS: u32 = 16;
    fn to_levels(self) -> LevelWords {
        LevelWords::of(u64::from(self.to_bits()), 16)
    }
    fn from_levels(words: &LevelWords) -> Float16 {
        Float16::from_bits(words.low() as u16)
    }
}

impl Carried for Float128 {
    const WIDTH: usize = 16;
    fn encode(self, out: &mut [u8]) {
        out[..16].copy_from_slice(&self.to_bits().to_le_bytes());
    }
    fn decode(bytes: &[u8]) -> Float128 {
        let mut word = [0u8; 16];
        word.copy_from_slice(&bytes[..16]);
        Float128::from_bits(u128::from_le_bytes(word))
    }
    const LEVELS: u32 = 128;
    fn to_levels(self) -> LevelWords {
        let bits = self.to_bits();
        LimbLevels::of(&[bits as u64, (bits >> 64) as u64], 128)
    }
    fn from_levels(words: &LevelWords) -> Float128 {
        let limbs = LimbLevels::limbs(words, 2);
        Float128::from_bits(u128::from(limbs[0]) | u128::from(limbs[1]) << 64)
    }
}

impl<const N: u32, const L: usize> Carried for WideUInt<N, L> {
    const WIDTH: usize = 8 * L;
    fn encode(self, out: &mut [u8]) {
        LimbLevels::write(self.limbs(), out);
    }
    fn decode(bytes: &[u8]) -> Self {
        WideUInt::from_limbs(&LimbLevels::read(bytes, L))
    }
    const LEVELS: u32 = N;
    fn to_levels(self) -> LevelWords {
        LimbLevels::of(self.limbs(), N)
    }
    fn from_levels(words: &LevelWords) -> Self {
        WideUInt::from_limbs(&LimbLevels::limbs(words, L))
    }
}

impl<const N: u32, const L: usize> Carried for WideInt<N, L> {
    const WIDTH: usize = 8 * L;
    fn encode(self, out: &mut [u8]) {
        LimbLevels::write(&self.sign_extended_limbs(), out);
    }
    fn decode(bytes: &[u8]) -> Self {
        WideInt::from_limbs(&LimbLevels::read(bytes, L))
    }
    const LEVELS: u32 = N;
    fn to_levels(self) -> LevelWords {
        LimbLevels::of(self.limbs(), N)
    }
    fn from_levels(words: &LevelWords) -> Self {
        WideInt::from_limbs(&LimbLevels::limbs(words, L))
    }
}

/// A float type `as` converts from and to: its IEEE 754 format and bit pattern.
pub trait FloatRepr: Copy {
    /// The interchange format of the type.
    const FORMAT: BinaryFormat;
    /// The bit pattern, in the low bits.
    fn to_pattern(self) -> u128;
    /// The value of a bit pattern of the format.
    fn from_pattern(pattern: u128) -> Self;
}

impl FloatRepr for f32 {
    const FORMAT: BinaryFormat = BinaryFormat::SINGLE;
    fn to_pattern(self) -> u128 {
        u128::from(self.to_bits())
    }
    fn from_pattern(pattern: u128) -> f32 {
        f32::from_bits(pattern as u32)
    }
}

impl FloatRepr for f64 {
    const FORMAT: BinaryFormat = BinaryFormat::DOUBLE;
    fn to_pattern(self) -> u128 {
        u128::from(self.to_bits())
    }
    fn from_pattern(pattern: u128) -> f64 {
        f64::from_bits(pattern as u64)
    }
}

impl FloatRepr for Float16 {
    const FORMAT: BinaryFormat = BinaryFormat::HALF;
    fn to_pattern(self) -> u128 {
        u128::from(self.to_bits())
    }
    fn from_pattern(pattern: u128) -> Float16 {
        Float16::from_bits(pattern as u16)
    }
}

impl FloatRepr for Float128 {
    const FORMAT: BinaryFormat = BinaryFormat::QUAD;
    fn to_pattern(self) -> u128 {
        self.to_bits()
    }
    fn from_pattern(pattern: u128) -> Float128 {
        Float128::from_bits(pattern)
    }
}

/// An integer type `as` converts from and to: its value as sign and magnitude, as two's complement
/// limbs, and from a float or limbs truncated to its width.
pub trait IntegerRepr: Copy {
    /// How many 64-bit limbs the type's value occupies.
    const LIMBS: usize;
    /// Whether the value is negative (a signed type below zero).
    fn negative(self) -> bool;
    /// The absolute value as limbs, low limb first.
    fn magnitude(self) -> Vec<u64>;
    /// The two's complement limbs extended with the sign (or zero) to at least `count` limbs.
    fn extended(self, count: usize) -> Vec<u64>;
    /// The value of the low bits of `limbs` at this type's width.
    fn truncated(limbs: &[u64]) -> Self;
    /// The truncated, clamped value of a float of `format` (NaN is 0).
    fn from_float(format: BinaryFormat, pattern: u128) -> Self;
}

impl<const N: u32> IntegerRepr for UInt<N> {
    const LIMBS: usize = 1;
    fn negative(self) -> bool {
        false
    }
    fn magnitude(self) -> Vec<u64> {
        vec![self.get()]
    }
    fn extended(self, count: usize) -> Vec<u64> {
        let mut limbs = vec![0u64; count.max(1)];
        limbs[0] = self.get();
        limbs
    }
    fn truncated(limbs: &[u64]) -> Self {
        UInt::new(limbs[0])
    }
    fn from_float(format: BinaryFormat, pattern: u128) -> Self {
        UInt::new(NumberConversion::float_to_unsigned::<N, 1>(format, pattern).low_u64())
    }
}

impl<const N: u32> IntegerRepr for Int<N> {
    const LIMBS: usize = 1;
    fn negative(self) -> bool {
        self.get() < 0
    }
    fn magnitude(self) -> Vec<u64> {
        vec![self.get().unsigned_abs()]
    }
    fn extended(self, count: usize) -> Vec<u64> {
        let mut limbs = vec![if self.get() < 0 { u64::MAX } else { 0 }; count.max(1)];
        limbs[0] = self.get() as u64;
        limbs
    }
    fn truncated(limbs: &[u64]) -> Self {
        Int::new(limbs[0] as i64)
    }
    fn from_float(format: BinaryFormat, pattern: u128) -> Self {
        Int::new(NumberConversion::float_to_signed::<N, 1>(format, pattern).low_i128() as i64)
    }
}

impl<const N: u32, const L: usize> IntegerRepr for WideUInt<N, L> {
    const LIMBS: usize = L;
    fn negative(self) -> bool {
        false
    }
    fn magnitude(self) -> Vec<u64> {
        WideUInt::magnitude(&self)
    }
    fn extended(self, count: usize) -> Vec<u64> {
        let mut limbs = self.limbs().to_vec();
        limbs.resize(count.max(L), 0);
        limbs
    }
    fn truncated(limbs: &[u64]) -> Self {
        WideUInt::from_limbs(limbs)
    }
    fn from_float(format: BinaryFormat, pattern: u128) -> Self {
        NumberConversion::float_to_unsigned::<N, L>(format, pattern)
    }
}

impl<const N: u32, const L: usize> IntegerRepr for WideInt<N, L> {
    const LIMBS: usize = L;
    fn negative(self) -> bool {
        self.is_negative()
    }
    fn magnitude(self) -> Vec<u64> {
        WideInt::magnitude(&self).limbs().to_vec()
    }
    fn extended(self, count: usize) -> Vec<u64> {
        let mut limbs = self.sign_extended_limbs().to_vec();
        limbs.resize(count.max(L), if self.is_negative() { u64::MAX } else { 0 });
        limbs
    }
    fn truncated(limbs: &[u64]) -> Self {
        WideInt::from_limbs(limbs)
    }
    fn from_float(format: BinaryFormat, pattern: u128) -> Self {
        NumberConversion::float_to_signed::<N, L>(format, pattern)
    }
}

/// `as` among every float and integer width (§1.4.5): each result is rounded (or wrapped, or
/// clamped) once from its source, never through an intermediate format.
pub struct Convert;

impl Convert {
    /// A float to the nearest float of another format, ties to even; NaN to NaN.
    pub fn float_to_float<A: FloatRepr, B: FloatRepr>(value: A) -> B {
        B::from_pattern(NumberConversion::float_to_float(A::FORMAT, value.to_pattern(), B::FORMAT))
    }

    /// An integer to the nearest float, ties to even; past the range, an infinity.
    pub fn integer_to_float<I: IntegerRepr, F: FloatRepr>(value: I) -> F {
        F::from_pattern(NumberConversion::integer_to_float(value.negative(), &value.magnitude(), F::FORMAT))
    }

    /// A float to an integer: truncated toward zero, clamped to the range of the type, NaN to 0.
    pub fn float_to_integer<F: FloatRepr, I: IntegerRepr>(value: F) -> I {
        I::from_float(F::FORMAT, value.to_pattern())
    }

    /// An integer to an integer: sign-extended by the source, then truncated to the target's width.
    pub fn integer_to_integer<A: IntegerRepr, B: IntegerRepr>(value: A) -> B {
        B::truncated(&value.extended(B::LIMBS))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The helpers of the tests of this file.
    struct Fixture;

    impl Fixture {
        /// The text of the panic `run` raises, `None` when it does not.
        fn panic_of(run: impl FnOnce() + std::panic::UnwindSafe) -> Option<String> {
            std::panic::catch_unwind(run).err().map(|payload| payload.downcast_ref::<String>().cloned().or_else(|| payload.downcast_ref::<&str>().map(|text| text.to_string())).unwrap_or_default())
        }
    }

    #[test]
    fn the_runtime_raises_msl_panics_and_carries_every_width() {
        // §1 rule 11: an integer `/` or `%` by zero, and `clamp` with crossed bounds, name the operation
        let divided = Fixture::panic_of(|| {
            let _ = WideUInt::<200, 4>::from_u64(7) / WideUInt::ZERO;
        })
        .expect("a zero divisor panics");
        assert!(divided.contains("integer division by zero") && divided.contains("UInt<200>") && divided.contains("7 / 0"), "{divided}");
        let remainder = Fixture::panic_of(|| {
            let _ = WideInt::<100, 2>::from_i64(-7) % WideInt::ZERO;
        })
        .expect("a zero divisor panics");
        assert!(remainder.contains("integer remainder by zero") && remainder.contains("Int<100>") && remainder.contains("-7 % 0"), "{remainder}");
        let euclid = Fixture::panic_of(|| {
            let _ = WideUInt::<65, 2>::from_u64(5).rem_euclid(WideUInt::ZERO);
        })
        .expect("a zero divisor panics");
        assert!(euclid.contains("`rem_euclid`") && euclid.contains("UInt<65>"), "{euclid}");
        let crossed = Fixture::panic_of(|| {
            let _ = WideInt::<70, 2>::from_i64(0).clamp(WideInt::from_i64(2), WideInt::from_i64(1));
        })
        .expect("crossed bounds panic");
        assert!(crossed.contains("`clamp`") && crossed.contains("Int<70>"), "{crossed}");
        let nan_bound = Fixture::panic_of(|| {
            let _ = Float16::from_f32(1.0).clamp(Float16::NAN, Float16::from_f32(2.0));
        })
        .expect("a NaN bound panics");
        assert!(nan_bound.contains("`clamp` on f16"), "{nan_bound}");
        let quad_crossed = Fixture::panic_of(|| {
            let _ = Float128::ONE.clamp(Float128::ONE + Float128::ONE, Float128::ONE);
        })
        .expect("crossed bounds panic");
        assert!(quad_crossed.contains("`clamp` on f128"), "{quad_crossed}");
        assert_eq!(WideUInt::<200, 4>::from_u64(17) / WideUInt::from_u64(5), WideUInt::from_u64(3));
        assert_eq!(WideInt::<100, 2>::from_i64(-17) % WideInt::from_i64(5), WideInt::from_i64(-2));
        assert_eq!(WideInt::<100, 2>::from_i64(-17).rem_euclid(WideInt::from_i64(5)), WideInt::from_i64(3));

        // §2: bytes round trip and level counts, for each type
        let wide = WideUInt::<200, 4>::from_limbs(&[1, 2, 3, 0xff]);
        let mut bytes = vec![0u8; WideUInt::<200, 4>::WIDTH];
        wide.encode(&mut bytes);
        assert_eq!(bytes.len(), 32);
        assert_eq!(WideUInt::<200, 4>::decode(&bytes), wide);
        assert_eq!(WideUInt::<200, 4>::from_levels(&wide.to_levels()), wide);
        assert_eq!((WideUInt::<200, 4>::LEVELS, wide.to_levels().len()), (200, 200));
        let negative = WideInt::<70, 2>::from_i64(-3);
        let mut bytes = vec![0u8; 16];
        negative.encode(&mut bytes);
        assert_eq!(bytes[8..], [0xff; 8], "an Int<70> is sign-extended to its limbs");
        assert_eq!(WideInt::<70, 2>::decode(&bytes), negative);
        assert_eq!(WideInt::<70, 2>::from_levels(&negative.to_levels()), negative);
        let half = Float16::from_bits(0x3c00);
        assert_eq!(Float16::decode(&half.pin()), half);
        assert_eq!(Float16::from_levels(&half.to_levels()), half);
        let quad = Float128::from_bits(0x3fff_8000_0000_0000_0000_0000_0000_0001);
        assert_eq!(Float128::decode(&quad.pin()), quad);
        assert_eq!(Float128::from_levels(&quad.to_levels()).to_bits(), quad.to_bits());
        assert_eq!((Float128::LEVELS, quad.to_levels().len(), f32::LEVELS, Float16::LEVELS), (128, 128, 32, 16));
        assert_eq!(f32::decode(&1.5f32.pin()), 1.5);
        assert_eq!(f32::from_levels(&1.5f32.to_levels()), 1.5);
        assert_eq!(Float128::WIDTH + f32::WIDTH + Float16::WIDTH, 22);

        // §1.4.5: conversions are those of NumberConversion, rounded once from the source
        let source = 1.0f64 + 2f64.powi(-11) + 2f64.powi(-40);
        let direct: Float16 = Convert::float_to_float(source);
        assert_eq!(direct.to_bits(), 0x3c01, "rounded once from f64, not through f32");
        let widened: Float128 = Convert::float_to_float(0.1f32);
        assert_eq!(widened.to_bits(), NumberConversion::float_to_float(BinaryFormat::SINGLE, u128::from(0.1f32.to_bits()), BinaryFormat::QUAD));
        assert_eq!(Convert::integer_to_integer::<Int<8>, WideUInt<70, 2>>(Int::new(-1)), WideUInt::<70, 2>::from_limbs(&[u64::MAX, 0x3f]), "sign-extended, then truncated to 70 bits");
        assert_eq!(Convert::integer_to_integer::<WideUInt<200, 4>, UInt<8>>(WideUInt::from_limbs(&[0x1ff, 5, 0, 0])).get(), 0xff);
        let saturated: UInt<8> = Convert::float_to_integer(300.5f32);
        assert_eq!(saturated.get(), 255);
        let truncated: Int<8> = Convert::float_to_integer(-3.9f64);
        assert_eq!(truncated.get(), -3);
        let from_nan: WideUInt<100, 2> = Convert::float_to_integer(f64::NAN);
        assert_eq!(from_nan, WideUInt::ZERO);
        let big: f32 = Convert::integer_to_float(WideUInt::<200, 4>::from_limbs(&[0, 1, 0, 0]));
        assert_eq!(big, 2f32.powi(64));
        let negative_float: f64 = Convert::integer_to_float(Int::<8>::new(-5));
        assert_eq!(negative_float, -5.0);
    }
}
