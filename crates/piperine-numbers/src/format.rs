//! `BinaryFormat`: the IEEE 754 binary interchange formats and correct rounding into them
//! (IEEE 754-2019 sections 3.3, 4.3.1 and 7.4).

use super::limbs::Limbs;

/// What a bit pattern of a format denotes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FloatClass {
    /// Plus or minus zero.
    Zero,
    /// A nonzero value below the smallest normal one.
    Subnormal,
    /// A finite value with an implicit leading bit.
    Normal,
    /// Plus or minus infinity.
    Infinite,
    /// Not a number.
    NaN,
}

/// The parameters of one binary interchange format.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BinaryFormat {
    /// The width of the encoding in bits.
    pub bits: u32,
    /// The significand precision in bits, the implicit bit included.
    pub precision: u32,
    /// The largest unbiased exponent of a finite value (the bias).
    pub maximum_exponent: i32,
}

impl BinaryFormat {
    /// binary16 (`f16`).
    pub const HALF: BinaryFormat = BinaryFormat { bits: 16, precision: 11, maximum_exponent: 15 };
    /// binary32 (`f32`).
    pub const SINGLE: BinaryFormat = BinaryFormat { bits: 32, precision: 24, maximum_exponent: 127 };
    /// binary64 (`f64`).
    pub const DOUBLE: BinaryFormat = BinaryFormat { bits: 64, precision: 53, maximum_exponent: 1023 };
    /// binary128 (`f128`).
    pub const QUAD: BinaryFormat = BinaryFormat { bits: 128, precision: 113, maximum_exponent: 16383 };

    /// The number of fraction bits stored (precision minus the implicit bit).
    pub fn fraction_bits(&self) -> u32 {
        self.precision - 1
    }

    /// The smallest unbiased exponent of a normal value.
    pub fn minimum_exponent(&self) -> i32 {
        1 - self.maximum_exponent
    }

    /// The biased exponent field that marks infinity and NaN.
    fn special_field(&self) -> u128 {
        2 * self.maximum_exponent as u128 + 1
    }

    /// The sign bit of the encoding.
    pub fn sign_bit(&self) -> u128 {
        1u128 << (self.bits - 1)
    }

    /// The biased exponent field of a pattern.
    fn field(&self, pattern: u128) -> u128 {
        (pattern >> self.fraction_bits()) & self.special_field()
    }

    /// The stored fraction of a pattern.
    fn fraction(&self, pattern: u128) -> u128 {
        pattern & ((1u128 << self.fraction_bits()) - 1)
    }

    /// Whether the sign bit of a pattern is set.
    pub fn is_negative(&self, pattern: u128) -> bool {
        pattern & self.sign_bit() != 0
    }

    /// The class of a bit pattern.
    pub fn classify(&self, pattern: u128) -> FloatClass {
        let field = self.field(pattern);
        let fraction = self.fraction(pattern);
        if field == self.special_field() {
            if fraction == 0 { FloatClass::Infinite } else { FloatClass::NaN }
        } else if field == 0 {
            if fraction == 0 { FloatClass::Zero } else { FloatClass::Subnormal }
        } else {
            FloatClass::Normal
        }
    }

    /// The infinity of the given sign.
    pub fn infinity(&self, negative: bool) -> u128 {
        (if negative { self.sign_bit() } else { 0 }) | self.special_field() << self.fraction_bits()
    }

    /// The canonical quiet NaN.
    pub fn quiet_nan(&self) -> u128 {
        self.special_field() << self.fraction_bits() | 1u128 << (self.fraction_bits() - 1)
    }

    /// The sign, significand and binary exponent of a finite pattern, `value = significand * 2^exponent`.
    pub fn decode(&self, pattern: u128) -> Option<(bool, u128, i64)> {
        let negative = self.is_negative(pattern);
        match self.classify(pattern) {
            FloatClass::Infinite | FloatClass::NaN => None,
            FloatClass::Zero | FloatClass::Subnormal => Some((negative, self.fraction(pattern), self.minimum_exponent() as i64 - self.fraction_bits() as i64)),
            FloatClass::Normal => {
                let significand = self.fraction(pattern) | 1u128 << self.fraction_bits();
                Some((negative, significand, self.field(pattern) as i64 - self.maximum_exponent as i64 - self.fraction_bits() as i64))
            }
        }
    }

    /// The pattern nearest to `(-1)^negative * (magnitude + epsilon) * 2^binary_exponent`, ties to
    /// even, gradual underflow, overflow to infinity (IEEE 754-2019 sections 4.3.1 and 7.4).
    /// `sticky` says the exact value lies strictly between `magnitude` and `magnitude + 1`.
    pub fn round(&self, negative: bool, magnitude: &[u64], binary_exponent: i64, sticky: bool) -> u128 {
        let sign = if negative { self.sign_bit() } else { 0 };
        let length = magnitude.bit_length() as i64;
        if length == 0 {
            return sign;
        }
        let precision = self.precision as i64;
        let top = binary_exponent + length - 1;
        let minimum = self.minimum_exponent() as i64;
        let kept_bits = if top >= minimum { precision } else { precision - (minimum - top) };
        let dropped = length - kept_bits;
        let (mut significand, mut unit_exponent) = if dropped <= 0 {
            (magnitude.shifted_left((-dropped) as u64).to_u128().unwrap_or(0), binary_exponent + dropped)
        } else {
            let dropped = dropped as u64;
            let kept = magnitude.shifted_right(dropped).to_u128().unwrap_or(0);
            let half = magnitude.bit(dropped - 1);
            let lower = magnitude.any_bit_below(dropped - 1) || sticky;
            let round_up = half && (lower || kept & 1 == 1);
            (kept + round_up as u128, binary_exponent + dropped as i64)
        };
        if significand == 0 {
            return sign;
        }
        if significand == 1u128 << self.precision {
            significand >>= 1;
            unit_exponent += 1;
        }
        self.encode(sign, significand, unit_exponent)
    }

    /// The pattern of `significand * 2^unit_exponent` where the significand is already rounded
    /// to the format (below `2^precision`).
    fn encode(&self, sign: u128, significand: u128, unit_exponent: i64) -> u128 {
        if significand < 1u128 << self.fraction_bits() {
            return sign | significand;
        }
        let biased = unit_exponent + self.fraction_bits() as i64 + self.maximum_exponent as i64;
        if biased >= self.special_field() as i64 {
            return sign | self.special_field() << self.fraction_bits();
        }
        sign | (biased as u128) << self.fraction_bits() | (significand - (1u128 << self.fraction_bits()))
    }
}
