//! `DynamicInteger`: an integer of any width the const evaluator folds (composite-values phase D,
//! CV-32, CV-36).
//!
//! The widths of the generated code are const generics (`rt::WideUInt<N, L>`); an elaboration holds
//! a width only as a number, so this type computes the same modulo-`2^N` semantics (§1 rule 6) on
//! limbs with the width as a field. Every result is the exact mathematical value, wrapped (or
//! saturated, or refused) by one rule, so each method states its rule and none repeats the
//! arithmetic of another.

use std::cmp::Ordering;

use piperine_numbers::convert::NumberConversion;
use piperine_numbers::format::{BinaryFormat, FloatClass};
use piperine_numbers::limbs::{Limbs, ToLimbs};

/// An integer of `width` bits, signed or not, held as its two's complement pattern in
/// `ceil(width / 64)` little-endian limbs with no bit set at or above `width`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::elab) struct DynamicInteger {
    /// The width in bits, at least 1.
    width: u32,
    /// Whether the value is read as two's complement (`Int<N>`) or as a magnitude (`UInt<N>`).
    signed: bool,
    /// The pattern, low limb first.
    pattern: Vec<u64>,
}

/// The exact value of an integer: a sign and a magnitude (zero is never negative).
type Exact = (bool, Vec<u64>);

impl DynamicInteger {
    /// How many limbs a pattern of `width` bits takes.
    fn limb_count(width: u32) -> usize {
        (width as usize).div_ceil(64)
    }

    /// The pattern of the low `width` bits of `limbs` (a longer slice is cut, a shorter one zero-extended).
    pub(in crate::elab) fn from_pattern(width: u32, signed: bool, limbs: &[u64]) -> DynamicInteger {
        let mut pattern = limbs.to_vec();
        pattern.resize(Self::limb_count(width), 0);
        let mut value = DynamicInteger { width, signed, pattern };
        value.mask();
        value
    }

    /// The value `negative * magnitude` wrapped modulo `2^width` (§1 rule 6).
    fn from_exact(width: u32, signed: bool, (negative, magnitude): &Exact) -> DynamicInteger {
        let value = Self::from_pattern(width, signed, magnitude);
        if *negative { value.negated() } else { value }
    }

    /// The value of an `i128` wrapped to `width` bits.
    pub(in crate::elab) fn from_i128(width: u32, signed: bool, value: i128) -> DynamicInteger {
        let magnitude = value.unsigned_abs().to_limbs();
        Self::from_exact(width, signed, &(value < 0, magnitude))
    }

    /// The width in bits.
    pub(in crate::elab) fn width(&self) -> u32 {
        self.width
    }

    /// Clears the bits at or above `width`.
    fn mask(&mut self) {
        let kept = self.width % 64;
        if kept != 0
            && let Some(top) = self.pattern.last_mut()
        {
            *top &= (1u64 << kept) - 1;
        }
    }

    /// The pattern as the IR holds a wide constant: two's complement at the width, a negative
    /// signed value sign-extended to the limb boundary (`WideData`).
    pub(in crate::elab) fn canonical_limbs(&self) -> Vec<u64> {
        let mut limbs = self.pattern.clone();
        let kept = self.width % 64;
        if self.is_negative()
            && kept != 0
            && let Some(top) = limbs.last_mut()
        {
            *top |= u64::MAX << kept;
        }
        limbs
    }

    /// Whether a signed value is below zero.
    fn is_negative(&self) -> bool {
        self.signed && self.pattern.bit(u64::from(self.width) - 1)
    }

    /// The value as sign and magnitude.
    fn exact(&self) -> Exact {
        if self.is_negative() {
            let negated = self.clone().negated();
            (true, negated.pattern.trimmed())
        } else {
            (false, self.pattern.trimmed())
        }
    }

    /// Two's complement negation within the width: the least signed value negates to itself.
    fn negated(mut self) -> DynamicInteger {
        for limb in &mut self.pattern {
            *limb = !*limb;
        }
        let mut one = vec![0u64; self.pattern.len()];
        one[0] = 1;
        self.pattern.add_assign_limbs(&one);
        self.mask();
        self
    }

    /// The least value of the type, as an exact value.
    fn least(&self) -> Exact {
        if self.signed { (true, Self::power_of_two(u64::from(self.width) - 1)) } else { (false, Vec::new()) }
    }

    /// The greatest value of the type, as an exact value.
    fn greatest(&self) -> Exact {
        let bits = if self.signed { u64::from(self.width) - 1 } else { u64::from(self.width) };
        (false, Self::power_of_two(bits).minus(&[1]).unwrap_or_default())
    }

    /// `2^bits` as a magnitude.
    fn power_of_two(bits: u64) -> Vec<u64> {
        vec![1u64].shifted_left(bits)
    }

    /// Whether the exact value is in the range of the type.
    fn fits(&self, (negative, magnitude): &Exact) -> bool {
        let length = magnitude.bit_length();
        let width = u64::from(self.width);
        match (self.signed, negative) {
            (true, true) => length < width || (length == width && magnitude.trailing_zero_bits() == width - 1),
            (true, false) => length < width,
            (false, true) => magnitude.is_zero_magnitude(),
            (false, false) => length <= width,
        }
    }

    /// The sum of two exact values.
    fn sum((an, am): &Exact, (bn, bm): &Exact) -> Exact {
        if an == bn {
            return (*an, am.plus(bm));
        }
        match am.compare_magnitude(bm) {
            Ordering::Equal => (false, Vec::new()),
            Ordering::Greater => (*an, am.minus(bm).unwrap_or_default()),
            Ordering::Less => (*bn, bm.minus(am).unwrap_or_default()),
        }
    }

    /// The exact result of `self + other`.
    fn exact_sum(&self, other: &DynamicInteger) -> Exact {
        Self::sum(&self.exact(), &other.exact())
    }

    /// The exact result of `self - other`.
    fn exact_difference(&self, other: &DynamicInteger) -> Exact {
        let (negative, magnitude) = other.exact();
        Self::sum(&self.exact(), &(!negative && !magnitude.is_zero_magnitude(), magnitude))
    }

    /// The exact result of `self * other`.
    fn exact_product(&self, other: &DynamicInteger) -> Exact {
        let ((an, am), (bn, bm)) = (self.exact(), other.exact());
        let product = am.times(&bm);
        (an != bn && !product.is_zero_magnitude(), product)
    }

    /// The value of an exact result held in this type: wrapped.
    fn wrapped(&self, exact: &Exact) -> DynamicInteger {
        Self::from_exact(self.width, self.signed, exact)
    }

    /// The value of an exact result held in this type: pinned to the nearer end outside its range.
    fn saturated(&self, exact: Exact) -> DynamicInteger {
        if self.fits(&exact) {
            self.wrapped(&exact)
        } else if exact.0 {
            self.wrapped(&self.least())
        } else {
            self.wrapped(&self.greatest())
        }
    }

    /// Wrapping addition.
    pub(in crate::elab) fn wrapping_add(&self, other: &DynamicInteger) -> DynamicInteger {
        self.wrapped(&self.exact_sum(other))
    }

    /// Wrapping subtraction.
    pub(in crate::elab) fn wrapping_sub(&self, other: &DynamicInteger) -> DynamicInteger {
        self.wrapped(&self.exact_difference(other))
    }

    /// Wrapping multiplication.
    pub(in crate::elab) fn wrapping_mul(&self, other: &DynamicInteger) -> DynamicInteger {
        self.wrapped(&self.exact_product(other))
    }

    /// Wrapping negation.
    pub(in crate::elab) fn wrapping_neg(&self) -> DynamicInteger {
        self.clone().negated()
    }

    /// Bitwise complement within the width.
    pub(in crate::elab) fn complement(&self) -> DynamicInteger {
        let mut value = self.clone();
        for limb in &mut value.pattern {
            *limb = !*limb;
        }
        value.mask();
        value
    }

    /// A bitwise operator over the two patterns.
    fn bitwise(&self, other: &DynamicInteger, operator: impl Fn(u64, u64) -> u64) -> DynamicInteger {
        let pattern = self.pattern.iter().zip(&other.pattern).map(|(a, b)| operator(*a, *b)).collect();
        DynamicInteger { width: self.width, signed: self.signed, pattern }
    }

    /// `self & other`.
    pub(in crate::elab) fn and(&self, other: &DynamicInteger) -> DynamicInteger {
        self.bitwise(other, |a, b| a & b)
    }

    /// `self | other`.
    pub(in crate::elab) fn or(&self, other: &DynamicInteger) -> DynamicInteger {
        self.bitwise(other, |a, b| a | b)
    }

    /// `self ^ other`.
    pub(in crate::elab) fn xor(&self, other: &DynamicInteger) -> DynamicInteger {
        self.bitwise(other, |a, b| a ^ b)
    }

    /// `self << amount` within the width; an amount of the width or more gives 0 (ROADMAP G5).
    pub(in crate::elab) fn shift_left(&self, amount: u64) -> DynamicInteger {
        let mut value = self.clone();
        value.pattern.shift_left_assign(amount);
        value.mask();
        value
    }

    /// `self >> amount`: logical for an unsigned value, filling with the sign for a signed one.
    pub(in crate::elab) fn shift_right(&self, amount: u64) -> DynamicInteger {
        let negative = self.is_negative();
        let mut limbs = self.canonical_limbs();
        limbs.shift_right_assign(amount, negative);
        Self::from_pattern(self.width, self.signed, &limbs)
    }

    /// The order of two values of the type.
    pub(in crate::elab) fn compare(&self, other: &DynamicInteger) -> Ordering {
        let ((an, am), (bn, bm)) = (self.exact(), other.exact());
        match (an, bn) {
            (false, true) => Ordering::Greater,
            (true, false) => Ordering::Less,
            (false, false) => am.compare_magnitude(&bm),
            (true, true) => bm.compare_magnitude(&am),
        }
    }

    /// The truncating quotient (the least value over -1 wraps to itself); `None` for a zero divisor.
    pub(in crate::elab) fn divide(&self, divisor: &DynamicInteger) -> Option<DynamicInteger> {
        let ((an, am), (bn, bm)) = (self.exact(), divisor.exact());
        let (quotient, _) = am.divide_with_remainder(&bm)?;
        Some(self.wrapped(&(an != bn && !quotient.is_zero_magnitude(), quotient)))
    }

    /// The remainder, with the sign of the dividend; `None` for a zero divisor.
    pub(in crate::elab) fn remainder(&self, divisor: &DynamicInteger) -> Option<DynamicInteger> {
        let ((an, am), (_, bm)) = (self.exact(), divisor.exact());
        let (_, remainder) = am.divide_with_remainder(&bm)?;
        Some(self.wrapped(&(an && !remainder.is_zero_magnitude(), remainder)))
    }

    /// The Euclidean remainder, `0 <= r < |divisor|` (H.1 rule 5); `None` for a zero divisor.
    pub(in crate::elab) fn remainder_euclid(&self, divisor: &DynamicInteger) -> Option<DynamicInteger> {
        let ((an, am), (_, bm)) = (self.exact(), divisor.exact());
        let (_, remainder) = am.divide_with_remainder(&bm)?;
        let adjusted = if an && !remainder.is_zero_magnitude() { bm.minus(&remainder).unwrap_or_default() } else { remainder };
        Some(self.wrapped(&(false, adjusted)))
    }

    /// `|self|`; the least signed value wraps to itself (H.1 rule 1).
    pub(in crate::elab) fn absolute(&self) -> DynamicInteger {
        self.wrapped(&(false, self.exact().1))
    }

    /// -1, 0 or 1 by the sign (H.1 rule 6).
    pub(in crate::elab) fn signum(&self) -> DynamicInteger {
        let (negative, magnitude) = self.exact();
        let sign = if magnitude.is_zero_magnitude() { 0 } else if negative { -1 } else { 1 };
        Self::from_i128(self.width, self.signed, sign)
    }

    /// `self` raised to `exponent` modulo `2^width` (`x^0 = 1`, H.1 rule 4).
    pub(in crate::elab) fn power(&self, exponent: u64) -> DynamicInteger {
        let (mut result, mut base, mut remaining) = (Self::from_i128(self.width, self.signed, 1), self.clone(), exponent);
        while remaining > 0 {
            if remaining & 1 == 1 {
                result = result.wrapping_mul(&base);
            }
            base = base.wrapping_mul(&base);
            remaining >>= 1;
        }
        result
    }

    /// The lesser of two values.
    pub(in crate::elab) fn minimum(&self, other: &DynamicInteger) -> DynamicInteger {
        if other.compare(self) == Ordering::Less { other.clone() } else { self.clone() }
    }

    /// The greater of two values.
    pub(in crate::elab) fn maximum(&self, other: &DynamicInteger) -> DynamicInteger {
        if other.compare(self) == Ordering::Greater { other.clone() } else { self.clone() }
    }

    /// `self` held in `lowest..=highest` (H.1 rule 3); `None` when `lowest > highest`.
    pub(in crate::elab) fn clamp(&self, lowest: &DynamicInteger, highest: &DynamicInteger) -> Option<DynamicInteger> {
        (lowest.compare(highest) != Ordering::Greater).then(|| self.maximum(lowest).minimum(highest))
    }

    /// `self + other` held in the range.
    pub(in crate::elab) fn saturating_add(&self, other: &DynamicInteger) -> DynamicInteger {
        self.saturated(self.exact_sum(other))
    }

    /// `self - other` held in the range.
    pub(in crate::elab) fn saturating_sub(&self, other: &DynamicInteger) -> DynamicInteger {
        self.saturated(self.exact_difference(other))
    }

    /// `self * other` held in the range.
    pub(in crate::elab) fn saturating_mul(&self, other: &DynamicInteger) -> DynamicInteger {
        self.saturated(self.exact_product(other))
    }

    /// `as` to an integer of another width or sign: sign-extend by the source, then truncate (§1.4.5).
    pub(in crate::elab) fn convert(&self, width: u32, signed: bool) -> DynamicInteger {
        Self::from_exact(width, signed, &self.exact())
    }

    /// `as` to a float of `format`: the nearest value, ties to even, overflow to infinity (§1.4.5).
    pub(in crate::elab) fn to_float(&self, format: BinaryFormat) -> u128 {
        let (negative, magnitude) = self.exact();
        NumberConversion::integer_to_float(negative, &magnitude, format)
    }

    /// `as` from a float of `format` with bit pattern `pattern`: truncated toward zero, clamped to
    /// the range, NaN as 0 (§1.4.5, ROADMAP G4).
    pub(in crate::elab) fn from_float(width: u32, signed: bool, format: BinaryFormat, pattern: u128) -> DynamicInteger {
        let zero = Self::from_i128(width, signed, 0);
        let Some((negative, significand, exponent)) = format.decode(pattern) else {
            return if format.classify(pattern) == FloatClass::Infinite { zero.clamped_to_end(format.is_negative(pattern)) } else { zero };
        };
        let limbs = significand.to_limbs();
        let magnitude = if exponent >= 0 {
            // a value certainly wider than the type clamps without building its magnitude
            if exponent as u64 > u64::from(width) + 1 && !limbs.is_zero_magnitude() { return zero.clamped_to_end(negative) } else { limbs.shifted_left(exponent as u64) }
        } else {
            limbs.shifted_right((-exponent) as u64)
        };
        let exact = (negative && !magnitude.is_zero_magnitude(), magnitude);
        if zero.fits(&exact) { zero.wrapped(&exact) } else { zero.clamped_to_end(negative) }
    }

    /// The least value of the type when `negative`, else the greatest.
    fn clamped_to_end(&self, negative: bool) -> DynamicInteger {
        self.wrapped(&if negative { self.least() } else { self.greatest() })
    }

    /// The value as an `i128` when it fits.
    pub(in crate::elab) fn to_i128(&self) -> Option<i128> {
        let (negative, magnitude) = self.exact();
        let value = i128::try_from(magnitude.to_u128()?).ok()?;
        Some(if negative { -value } else { value })
    }

    /// A shift amount: the value saturated to a `u64`; `None` for a negative value.
    pub(in crate::elab) fn shift_amount(&self) -> Option<u64> {
        let (negative, magnitude) = self.exact();
        if negative {
            return None;
        }
        Some(magnitude.to_u128().and_then(|value| u64::try_from(value).ok()).unwrap_or(u64::MAX))
    }
}
