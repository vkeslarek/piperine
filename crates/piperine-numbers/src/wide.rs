//! `WideUInt` and `WideInt`: integers of any width, as little-endian `u64` limbs (CV-32).
//!
//! Const generic arithmetic is not stable, so a type spells both its width `N` in bits and its
//! limb count `L = ceil(N / 64)`; the pair is checked when a value is built. Arithmetic wraps
//! modulo `2^N` (section 1 rule 6) and a signed value is its two's complement pattern. `/` and `%`
//! panic in the language, so here they are total methods returning `Option`.

use super::limbs::{Limbs, ToLimbs};
use core::cmp::Ordering;
use core::ops::{Add, BitAnd, BitOr, BitXor, Mul, Neg, Not, Sub};

/// An unsigned integer of `N` bits held in `L = ceil(N / 64)` limbs, always below `2^N`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct WideUInt<const N: u32, const L: usize>([u64; L]);

/// A signed integer of `N` bits: the two's complement pattern in `L` limbs, always below `2^N`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct WideInt<const N: u32, const L: usize>(WideUInt<N, L>);

impl<const N: u32, const L: usize> WideUInt<N, L> {
    /// Compile-time proof that `L` is the limb count of `N` bits (and `N >= 1`).
    const LAYOUT: () = assert!(N >= 1 && L == (N as usize).div_ceil(64), "L must be ceil(N / 64) and N at least 1");
    /// Zero.
    pub const ZERO: Self = {
        let () = Self::LAYOUT;
        WideUInt([0; L])
    };

    /// The value `1`.
    pub fn one() -> Self {
        Self::from_u64(1)
    }

    /// The least `N` bits of `limbs` (a longer slice is truncated, a shorter one zero-extended).
    pub fn from_limbs(limbs: &[u64]) -> Self {
        let () = Self::LAYOUT;
        let mut pattern = [0u64; L];
        for (slot, limb) in pattern.iter_mut().zip(limbs) {
            *slot = *limb;
        }
        WideUInt(pattern).masked()
    }

    /// The value of a `u64`, truncated to `N` bits.
    pub fn from_u64(value: u64) -> Self {
        Self::from_limbs(&[value])
    }

    /// The value of a `u128`, truncated to `N` bits.
    pub fn from_u128(value: u128) -> Self {
        Self::from_limbs(&[value as u64, (value >> 64) as u64])
    }

    /// The limbs, least significant first.
    pub fn limbs(&self) -> &[u64; L] {
        &self.0
    }

    /// The least 64 bits.
    pub fn low_u64(&self) -> u64 {
        self.0[0]
    }

    /// The least 128 bits.
    pub fn low_u128(&self) -> u128 {
        self.0[0] as u128 | (self.0.get(1).copied().unwrap_or(0) as u128) << 64
    }

    /// The greatest value, `2^N - 1`.
    pub fn maximum() -> Self {
        Self::from_limbs(&[u64::MAX; L])
    }

    /// Clears the bits at or above `N`.
    fn masked(mut self) -> Self {
        let kept = N % 64;
        if kept != 0 {
            self.0[L - 1] &= (1u64 << kept) - 1;
        }
        self
    }

    /// The value as a normalized magnitude.
    pub fn magnitude(&self) -> Vec<u64> {
        self.0.trimmed()
    }

    /// Wrapping addition.
    pub fn wrapping_add(self, other: Self) -> Self {
        let mut sum = self.0;
        sum.add_assign_limbs(&other.0);
        WideUInt(sum).masked()
    }

    /// Wrapping subtraction.
    pub fn wrapping_sub(self, other: Self) -> Self {
        let mut difference = self.0;
        difference.sub_assign_limbs(&other.0);
        WideUInt(difference).masked()
    }

    /// Wrapping multiplication.
    pub fn wrapping_mul(self, other: Self) -> Self {
        Self::from_limbs(&self.0.times(&other.0))
    }

    /// Wrapping negation: `2^N - self`.
    pub fn wrapping_neg(self) -> Self {
        Self::ZERO.wrapping_sub(self)
    }

    /// `self << amount` within `N` bits; an amount of `N` or more gives zero (ROADMAP G5).
    pub fn shift_left(self, amount: u64) -> Self {
        let mut shifted = self.0;
        shifted.shift_left_assign(amount);
        WideUInt(shifted).masked()
    }

    /// `self >> amount`, filling with zeros; an amount of `N` or more gives zero.
    pub fn shift_right(self, amount: u64) -> Self {
        let mut shifted = self.0;
        shifted.shift_right_assign(amount, false);
        WideUInt(shifted)
    }

    /// Whether `bit` (counted from the least significant) is set.
    pub fn bit(&self, bit: u64) -> bool {
        self.0.bit(bit)
    }

    /// The quotient, or `None` for a zero divisor.
    pub fn checked_divide(self, divisor: Self) -> Option<Self> {
        self.0.divide_with_remainder(&divisor.0).map(|(quotient, _)| Self::from_limbs(&quotient))
    }

    /// The remainder, or `None` for a zero divisor.
    pub fn checked_remainder(self, divisor: Self) -> Option<Self> {
        self.0.divide_with_remainder(&divisor.0).map(|(_, remainder)| Self::from_limbs(&remainder))
    }

    /// The Euclidean remainder (the plain remainder for an unsigned value), `None` for zero.
    pub fn rem_euclid_checked(self, divisor: Self) -> Option<Self> {
        self.checked_remainder(divisor)
    }

    /// `clamp(lo, hi)` (H.1 rule 3), `None` when `lo > hi`.
    pub fn clamp_checked(self, lowest: Self, highest: Self) -> Option<Self> {
        if lowest > highest {
            return None;
        }
        Some(if self < lowest { lowest } else if self > highest { highest } else { self })
    }

    /// The lesser of two values.
    pub fn minimum(self, other: Self) -> Self {
        if other < self { other } else { self }
    }

    /// The greater of two values.
    pub fn maximum_of(self, other: Self) -> Self {
        if other > self { other } else { self }
    }

    /// `self` raised to `exponent`, modulo `2^N` (`x^0 = 1`, H.1 rule 4).
    pub fn pow(self, exponent: u32) -> Self {
        let mut result = Self::one();
        let mut base = self;
        let mut remaining = exponent;
        while remaining > 0 {
            if remaining & 1 == 1 {
                result = result.wrapping_mul(base);
            }
            base = base.wrapping_mul(base);
            remaining >>= 1;
        }
        result
    }

    /// `self + other`, or `None` past `2^N - 1`.
    pub fn checked_add(self, other: Self) -> Option<Self> {
        let exact = self.0.plus(&other.0);
        (exact.bit_length() <= N as u64).then(|| Self::from_limbs(&exact))
    }

    /// `self - other`, or `None` below zero.
    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.0.minus(&other.0).map(|exact| Self::from_limbs(&exact))
    }

    /// `self * other`, or `None` past `2^N - 1`.
    pub fn checked_mul(self, other: Self) -> Option<Self> {
        let exact = self.0.times(&other.0);
        (exact.bit_length() <= N as u64).then(|| Self::from_limbs(&exact))
    }

    /// `self + other` held at `2^N - 1`.
    pub fn saturating_add(self, other: Self) -> Self {
        self.checked_add(other).unwrap_or_else(Self::maximum)
    }

    /// `self - other` held at zero.
    pub fn saturating_sub(self, other: Self) -> Self {
        self.checked_sub(other).unwrap_or(Self::ZERO)
    }

    /// `self * other` held at `2^N - 1`.
    pub fn saturating_mul(self, other: Self) -> Self {
        self.checked_mul(other).unwrap_or_else(Self::maximum)
    }

    /// Whether `self + other` leaves `0..=2^N - 1`.
    pub fn add_overflows(self, other: Self) -> bool {
        self.checked_add(other).is_none()
    }

    /// Whether `self - other` leaves `0..=2^N - 1`.
    pub fn sub_overflows(self, other: Self) -> bool {
        self.checked_sub(other).is_none()
    }

    /// Whether `self * other` leaves `0..=2^N - 1`.
    pub fn mul_overflows(self, other: Self) -> bool {
        self.checked_mul(other).is_none()
    }

    /// `as` to an unsigned integer of another width: truncate or zero-extend (section 1.4.5).
    pub fn to_unsigned<const M: u32, const K: usize>(self) -> WideUInt<M, K> {
        WideUInt::from_limbs(&self.0)
    }

    /// `as` to a signed integer of another width: truncate or zero-extend, then reinterpret.
    pub fn to_signed<const M: u32, const K: usize>(self) -> WideInt<M, K> {
        WideInt(WideUInt::from_limbs(&self.0))
    }
}

impl<const N: u32, const L: usize> PartialOrd for WideUInt<N, L> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<const N: u32, const L: usize> Ord for WideUInt<N, L> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.compare_magnitude(&other.0)
    }
}

impl<const N: u32, const L: usize> Add for WideUInt<N, L> {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        self.wrapping_add(other)
    }
}

impl<const N: u32, const L: usize> Sub for WideUInt<N, L> {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        self.wrapping_sub(other)
    }
}

impl<const N: u32, const L: usize> Mul for WideUInt<N, L> {
    type Output = Self;
    fn mul(self, other: Self) -> Self {
        self.wrapping_mul(other)
    }
}

impl<const N: u32, const L: usize> Neg for WideUInt<N, L> {
    type Output = Self;
    fn neg(self) -> Self {
        self.wrapping_neg()
    }
}

impl<const N: u32, const L: usize> Not for WideUInt<N, L> {
    type Output = Self;
    fn not(self) -> Self {
        let mut inverted = self.0;
        for limb in inverted.iter_mut() {
            *limb = !*limb;
        }
        WideUInt(inverted).masked()
    }
}

impl<const N: u32, const L: usize> BitAnd for WideUInt<N, L> {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        let mut result = self.0;
        for (limb, right) in result.iter_mut().zip(other.0) {
            *limb &= right;
        }
        WideUInt(result)
    }
}

impl<const N: u32, const L: usize> BitOr for WideUInt<N, L> {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        let mut result = self.0;
        for (limb, right) in result.iter_mut().zip(other.0) {
            *limb |= right;
        }
        WideUInt(result)
    }
}

impl<const N: u32, const L: usize> BitXor for WideUInt<N, L> {
    type Output = Self;
    fn bitxor(self, other: Self) -> Self {
        let mut result = self.0;
        for (limb, right) in result.iter_mut().zip(other.0) {
            *limb ^= right;
        }
        WideUInt(result)
    }
}

impl<const N: u32, const L: usize> WideInt<N, L> {
    /// Zero.
    pub const ZERO: Self = WideInt(WideUInt::<N, L>::ZERO);

    /// The least `N` bits of `limbs` read as a two's complement pattern.
    pub fn from_limbs(limbs: &[u64]) -> Self {
        WideInt(WideUInt::from_limbs(limbs))
    }

    /// The value of an `i64`, sign-extended and truncated to `N` bits.
    pub fn from_i64(value: i64) -> Self {
        Self::from_i128(value as i128)
    }

    /// The value of an `i128`, sign-extended and truncated to `N` bits.
    pub fn from_i128(value: i128) -> Self {
        let fill = if value < 0 { u64::MAX } else { 0 };
        let mut limbs = vec![fill; L.max(2)];
        limbs[0] = value as u64;
        limbs[1] = (value >> 64) as u64;
        Self::from_limbs(&limbs)
    }

    /// The two's complement pattern: `N` bits in `L` limbs.
    pub fn limbs(&self) -> &[u64; L] {
        self.0.limbs()
    }

    /// The pattern sign-extended to the full `64 * L` bits.
    pub fn sign_extended_limbs(&self) -> [u64; L] {
        let mut limbs = *self.0.limbs();
        let kept = N % 64;
        if self.is_negative() && kept != 0 {
            limbs[L - 1] |= u64::MAX << kept;
        }
        limbs
    }

    /// The least 128 bits of the sign-extended value, as a pattern.
    pub fn low_i128(&self) -> i128 {
        let extended = self.sign_extended_limbs();
        let low = extended[0] as u128;
        let high = if L > 1 { extended[1] as u128 } else if self.is_negative() { u64::MAX as u128 } else { 0 };
        (low | high << 64) as i128
    }

    /// Whether the value is below zero.
    pub fn is_negative(&self) -> bool {
        self.0.bit(N as u64 - 1)
    }

    /// The least value, `-2^(N-1)`.
    pub fn minimum() -> Self {
        WideInt(WideUInt::<N, L>::one().shift_left(N as u64 - 1))
    }

    /// The greatest value, `2^(N-1) - 1`.
    pub fn maximum() -> Self {
        WideInt(WideUInt::<N, L>::one().shift_left(N as u64 - 1).wrapping_sub(WideUInt::one()))
    }

    /// `|self|` as an unsigned value (the least value's magnitude `2^(N-1)` fits `N` bits).
    pub fn magnitude(&self) -> WideUInt<N, L> {
        if self.is_negative() { self.0.wrapping_neg() } else { self.0 }
    }

    /// The value with the given sign and magnitude, wrapping modulo `2^N`.
    pub fn from_magnitude(negative: bool, magnitude: WideUInt<N, L>) -> Self {
        WideInt(if negative { magnitude.wrapping_neg() } else { magnitude })
    }

    /// Wrapping addition.
    pub fn wrapping_add(self, other: Self) -> Self {
        WideInt(self.0.wrapping_add(other.0))
    }

    /// Wrapping subtraction.
    pub fn wrapping_sub(self, other: Self) -> Self {
        WideInt(self.0.wrapping_sub(other.0))
    }

    /// Wrapping multiplication.
    pub fn wrapping_mul(self, other: Self) -> Self {
        WideInt(self.0.wrapping_mul(other.0))
    }

    /// Wrapping negation (the least value negates to itself).
    pub fn wrapping_neg(self) -> Self {
        WideInt(self.0.wrapping_neg())
    }

    /// `self << amount`; an amount of `N` or more gives zero.
    pub fn shift_left(self, amount: u64) -> Self {
        WideInt(self.0.shift_left(amount))
    }

    /// `self >> amount` filling with the sign; an amount of `N` or more gives zero or `-1`.
    pub fn shift_right(self, amount: u64) -> Self {
        let mut shifted = self.sign_extended_limbs();
        shifted.shift_right_assign(amount, self.is_negative());
        Self::from_limbs(&shifted)
    }

    /// The truncating quotient (the least value over `-1` wraps to itself), `None` for zero.
    pub fn checked_divide(self, divisor: Self) -> Option<Self> {
        let quotient = self.magnitude().checked_divide(divisor.magnitude())?;
        Some(Self::from_magnitude(self.is_negative() != divisor.is_negative(), quotient))
    }

    /// The remainder with the sign of the dividend, `None` for a zero divisor.
    pub fn checked_remainder(self, divisor: Self) -> Option<Self> {
        let remainder = self.magnitude().checked_remainder(divisor.magnitude())?;
        Some(Self::from_magnitude(self.is_negative(), remainder))
    }

    /// The Euclidean remainder, `0 <= r < |divisor|` (H.1 rule 5), `None` for a zero divisor.
    pub fn rem_euclid_checked(self, divisor: Self) -> Option<Self> {
        let remainder = self.magnitude().checked_remainder(divisor.magnitude())?;
        let adjusted = if self.is_negative() && remainder != WideUInt::ZERO { divisor.magnitude().wrapping_sub(remainder) } else { remainder };
        Some(WideInt(adjusted))
    }

    /// `|self|`; the least value wraps to itself (H.1 rule 1).
    pub fn abs(self) -> Self {
        WideInt(self.magnitude())
    }

    /// `-1`, `0` or `1` by the sign (H.1 rule 6).
    pub fn signum(self) -> Self {
        if self.is_negative() {
            Self::from_i64(-1)
        } else if self.0 == WideUInt::ZERO {
            Self::ZERO
        } else {
            Self::from_i64(1)
        }
    }

    /// The lesser of two values.
    pub fn minimum_of(self, other: Self) -> Self {
        if other < self { other } else { self }
    }

    /// The greater of two values.
    pub fn maximum_of(self, other: Self) -> Self {
        if other > self { other } else { self }
    }

    /// `clamp(lo, hi)`, `None` when `lo > hi`.
    pub fn clamp_checked(self, lowest: Self, highest: Self) -> Option<Self> {
        if lowest > highest {
            return None;
        }
        Some(if self < lowest { lowest } else if self > highest { highest } else { self })
    }

    /// `self` raised to `exponent`, modulo `2^N`.
    pub fn pow(self, exponent: u32) -> Self {
        WideInt(self.0.pow(exponent))
    }

    /// Whether the exact value `negative * magnitude` (a magnitude as limbs) fits `N` signed bits.
    fn fits(negative: bool, magnitude: &[u64]) -> Option<Self> {
        let length = magnitude.bit_length();
        let limit = N as u64 - 1;
        let fits = length <= limit || (negative && length == N as u64 && magnitude.trailing_zero_bits() == limit);
        fits.then(|| Self::from_magnitude(negative, WideUInt::from_limbs(magnitude)))
    }

    /// `self + other`, or `None` outside `-2^(N-1)..=2^(N-1)-1`.
    pub fn checked_add(self, other: Self) -> Option<Self> {
        let wrapped = self.wrapping_add(other);
        let overflow = self.is_negative() == other.is_negative() && wrapped.is_negative() != self.is_negative();
        (!overflow).then_some(wrapped)
    }

    /// `self - other`, or `None` outside the range.
    pub fn checked_sub(self, other: Self) -> Option<Self> {
        let wrapped = self.wrapping_sub(other);
        let overflow = self.is_negative() != other.is_negative() && wrapped.is_negative() != self.is_negative();
        (!overflow).then_some(wrapped)
    }

    /// `self * other`, or `None` outside the range.
    pub fn checked_mul(self, other: Self) -> Option<Self> {
        let exact = self.magnitude().limbs().times(other.magnitude().limbs());
        Self::fits(self.is_negative() != other.is_negative(), &exact)
    }

    /// `self + other` held in `MIN..=MAX`.
    pub fn saturating_add(self, other: Self) -> Self {
        self.checked_add(other).unwrap_or_else(|| if other.is_negative() { Self::minimum() } else { Self::maximum() })
    }

    /// `self - other` held in `MIN..=MAX`.
    pub fn saturating_sub(self, other: Self) -> Self {
        self.checked_sub(other).unwrap_or_else(|| if self.is_negative() { Self::minimum() } else { Self::maximum() })
    }

    /// `self * other` held in `MIN..=MAX`.
    pub fn saturating_mul(self, other: Self) -> Self {
        self.checked_mul(other).unwrap_or_else(|| if self.is_negative() != other.is_negative() { Self::minimum() } else { Self::maximum() })
    }

    /// Whether `self + other` leaves `MIN..=MAX`.
    pub fn add_overflows(self, other: Self) -> bool {
        self.checked_add(other).is_none()
    }

    /// Whether `self - other` leaves `MIN..=MAX`.
    pub fn sub_overflows(self, other: Self) -> bool {
        self.checked_sub(other).is_none()
    }

    /// Whether `self * other` leaves `MIN..=MAX`.
    pub fn mul_overflows(self, other: Self) -> bool {
        self.checked_mul(other).is_none()
    }

    /// `as` to an unsigned integer of another width: sign-extend by the source, then truncate.
    pub fn to_unsigned<const M: u32, const K: usize>(self) -> WideUInt<M, K> {
        WideUInt::from_limbs(&self.extended_to(K))
    }

    /// `as` to a signed integer of another width: sign-extend by the source, then truncate.
    pub fn to_signed<const M: u32, const K: usize>(self) -> WideInt<M, K> {
        WideInt::from_limbs(&self.extended_to(K))
    }

    /// The sign-extended pattern as `count` limbs (extended with the sign, or cut).
    fn extended_to(&self, count: usize) -> Vec<u64> {
        let mut limbs = self.sign_extended_limbs().to_vec();
        limbs.resize(count.max(L), if self.is_negative() { u64::MAX } else { 0 });
        limbs
    }

    /// The magnitude as decimal digits with a leading `-` when negative.
    pub fn to_decimal_string(&self) -> String {
        let digits = self.magnitude().limbs().to_decimal_string();
        if self.is_negative() { format!("-{digits}") } else { digits }
    }
}

impl<const N: u32, const L: usize> PartialOrd for WideInt<N, L> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<const N: u32, const L: usize> Ord for WideInt<N, L> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.is_negative(), other.is_negative()) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => self.0.cmp(&other.0),
        }
    }
}

impl<const N: u32, const L: usize> Add for WideInt<N, L> {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        self.wrapping_add(other)
    }
}

impl<const N: u32, const L: usize> Sub for WideInt<N, L> {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        self.wrapping_sub(other)
    }
}

impl<const N: u32, const L: usize> Mul for WideInt<N, L> {
    type Output = Self;
    fn mul(self, other: Self) -> Self {
        self.wrapping_mul(other)
    }
}

impl<const N: u32, const L: usize> Neg for WideInt<N, L> {
    type Output = Self;
    fn neg(self) -> Self {
        self.wrapping_neg()
    }
}

impl<const N: u32, const L: usize> Not for WideInt<N, L> {
    type Output = Self;
    fn not(self) -> Self {
        WideInt(!self.0)
    }
}

impl<const N: u32, const L: usize> BitAnd for WideInt<N, L> {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        WideInt(self.0 & other.0)
    }
}

impl<const N: u32, const L: usize> BitOr for WideInt<N, L> {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        WideInt(self.0 | other.0)
    }
}

impl<const N: u32, const L: usize> BitXor for WideInt<N, L> {
    type Output = Self;
    fn bitxor(self, other: Self) -> Self {
        WideInt(self.0 ^ other.0)
    }
}

impl<const N: u32, const L: usize> WideUInt<N, L> {
    /// The value as decimal digits.
    pub fn to_decimal_string(&self) -> String {
        self.0.to_decimal_string()
    }

    /// The value as normalized limbs through [`ToLimbs`] of a `u128` when it fits.
    pub fn to_u128(&self) -> Option<u128> {
        self.0.to_u128()
    }
}

impl<const N: u32, const L: usize> ToLimbs for WideUInt<N, L> {
    fn to_limbs(self) -> Vec<u64> {
        self.0.trimmed()
    }
}
