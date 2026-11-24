//! `FloatCell` and `WideCell`: the values of `f16`, `f32`, `f128` and of the integers wider than
//! 64 bits as a host holds them (composite-values phase D, CV-29 to CV-32, user Q2 (c)): exact, never
//! through an `f64`. A float is its IEEE 754 bit pattern and its format; an integer is its
//! two's complement limbs, its width and its sign. Their text is the shortest decimal that reads back
//! to the same value (floats, QD6) and the exact decimal (integers); the plot gets the nearest `f64`.

use piperine_ir::ValueType;
use piperine_numbers::convert::NumberConversion;
use piperine_numbers::decimal::DecimalText;
use piperine_numbers::format::{BinaryFormat, FloatClass};
use piperine_numbers::limbs::Limbs;

use crate::failure::Failure;
use crate::table::Cell;

/// The interchange format of a float cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatFormat {
    /// IEEE 754 binary16 (`f16`).
    Half,
    /// IEEE 754 binary32 (`f32`).
    Single,
    /// IEEE 754 binary128 (`f128`).
    Quad,
}

impl FloatFormat {
    /// The format of the float type `ty`, `None` for any other type.
    pub fn of(ty: &ValueType) -> Option<FloatFormat> {
        match ty {
            ValueType::F16 => Some(FloatFormat::Half),
            ValueType::F32 => Some(FloatFormat::Single),
            ValueType::F128 => Some(FloatFormat::Quad),
            _ => None,
        }
    }

    /// The parameters of the format.
    pub fn binary(self) -> BinaryFormat {
        match self {
            FloatFormat::Half => BinaryFormat::HALF,
            FloatFormat::Single => BinaryFormat::SINGLE,
            FloatFormat::Quad => BinaryFormat::QUAD,
        }
    }

    /// How many bytes a value of the format takes on a pin.
    pub fn bytes(self) -> usize {
        self.binary().bits as usize / 8
    }
}

/// One `f16`, `f32` or `f128` value, exact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FloatCell {
    /// The format of the value.
    pub format: FloatFormat,
    /// The IEEE 754 bit pattern, in the low bits.
    pub bits: u128,
}

impl FloatCell {
    /// The value of the pin bytes `bytes` (little-endian, the format's own width).
    pub fn from_bytes(format: FloatFormat, bytes: &[u8]) -> Option<FloatCell> {
        (bytes.len() == format.bytes()).then(|| {
            let mut word = [0u8; 16];
            word[..bytes.len()].copy_from_slice(bytes);
            FloatCell { format, bits: u128::from_le_bytes(word) }
        })
    }

    /// The pin bytes of the value.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.bits.to_le_bytes()[..self.format.bytes()].to_vec()
    }

    /// The value read from decimal text (A.8 rule 8: the nearest value of the format, ties to even); `None` when the text is no number.
    pub fn parse(format: FloatFormat, text: &str) -> Option<FloatCell> {
        DecimalText::nearest(&text.replace('_', ""), format.binary()).map(|bits| FloatCell { format, bits })
    }

    /// The shortest decimal text that reads back to the same value in the format.
    pub fn text(&self) -> String {
        DecimalText::shortest(self.bits, self.format.binary())
    }

    /// Whether the value is neither NaN nor an infinity.
    pub fn is_finite(&self) -> bool {
        !matches!(self.format.binary().classify(self.bits), FloatClass::NaN | FloatClass::Infinite)
    }

    /// The nearest `f64`, rounded once from the value (a plot's coordinate).
    pub fn nearest_f64(&self) -> f64 {
        f64::from_bits(NumberConversion::float_to_float(self.format.binary(), self.bits, BinaryFormat::DOUBLE) as u64)
    }
}

/// One integer wider than 64 bits, exact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WideCell {
    /// Whether the value is two's complement (`Int<N>`) or a magnitude (`UInt<N>`).
    pub signed: bool,
    /// The width in bits.
    pub width: u32,
    /// The pattern, low limb first, `ceil(width / 64)` limbs, no bit set at or above `width`.
    pub limbs: Vec<u64>,
}

impl WideCell {
    /// How many limbs a value of `width` bits takes.
    pub fn limb_count(width: u32) -> usize {
        (width as usize).div_ceil(64)
    }

    /// The value of the pin bytes `bytes` of the integer type `ty` (`U64` limbs, low first, the limb boundary sign-extended).
    pub fn from_bytes(ty: &ValueType, bytes: &[u8]) -> Option<WideCell> {
        let (signed, width) = match ty {
            ValueType::UInt(width) => (false, *width),
            ValueType::Int(width) => (true, *width),
            _ => return None,
        };
        if bytes.len() != 8 * Self::limb_count(width) {
            return None;
        }
        let limbs = bytes.chunks_exact(8).map(|chunk| u64::from_le_bytes(chunk.try_into().unwrap_or([0; 8]))).collect();
        Some(WideCell::masked(signed, width, limbs))
    }

    /// The value of `limbs` held at `width` bits: the bits at or above the width cleared.
    pub fn masked(signed: bool, width: u32, mut limbs: Vec<u64>) -> WideCell {
        limbs.resize(Self::limb_count(width), 0);
        let kept = width % 64;
        if kept != 0
            && let Some(top) = limbs.last_mut()
        {
            *top &= (1u64 << kept) - 1;
        }
        WideCell { signed, width, limbs }
    }

    /// The pin bytes of the value: the limbs little-endian, a negative signed value extended with its sign to the limb boundary.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut limbs = self.limbs.clone();
        let kept = self.width % 64;
        if self.is_negative()
            && kept != 0
            && let Some(top) = limbs.last_mut()
        {
            *top |= u64::MAX << kept;
        }
        limbs.iter().flat_map(|limb| limb.to_le_bytes()).collect()
    }

    /// The value read from integer text (decimal, `0x`, `0o` or `0b`, `_` between digits, a `-` for a signed type); `None` when
    /// the text is no integer or the value lies outside the type.
    pub fn parse(signed: bool, width: u32, text: &str) -> Option<WideCell> {
        DecimalText::integer(text.trim(), width, signed).map(|limbs| WideCell::masked(signed, width, limbs))
    }

    /// Whether a signed value is below zero.
    fn is_negative(&self) -> bool {
        self.signed && self.limbs.bit(u64::from(self.width) - 1)
    }

    /// The sign and the magnitude of the value.
    fn sign_and_magnitude(&self) -> (bool, Vec<u64>) {
        if !self.is_negative() {
            return (false, self.limbs.trimmed());
        }
        let mut negated: Vec<u64> = self.limbs.iter().map(|limb| !limb).collect();
        let mut one = vec![0u64; negated.len()];
        one[0] = 1;
        negated.add_assign_limbs(&one);
        (true, WideCell::masked(false, self.width, negated).limbs.trimmed())
    }

    /// The exact decimal text.
    pub fn text(&self) -> String {
        let (negative, magnitude) = self.sign_and_magnitude();
        DecimalText::integer_text(negative, &magnitude)
    }

    /// The nearest `f64`, rounded once from the value (a plot's coordinate).
    pub fn nearest_f64(&self) -> f64 {
        let (negative, magnitude) = self.sign_and_magnitude();
        f64::from_bits(NumberConversion::integer_to_float(negative, &magnitude, BinaryFormat::DOUBLE) as u64)
    }
}

/// The cells of the types whose value is not an `f64`, an `i128` or a flag.
pub struct NumberCell;

impl NumberCell {
    /// Whether `ty` is a type whose cell is a `FloatCell` or a `WideCell`.
    pub fn is_number_width(ty: &ValueType) -> bool {
        FloatFormat::of(ty).is_some() || matches!(ty, ValueType::UInt(width) | ValueType::Int(width) if *width > 64)
    }

    /// The cell of the pin bytes `bytes` of a value of the number-width type `ty`; a length other than the type's is a `Failure`.
    pub fn decode(ty: &ValueType, bytes: &[u8]) -> Result<Cell, Failure> {
        let wrong = || Failure::Backend(format!("cannot decode a value of type {ty} from {} bytes", bytes.len()));
        if let Some(format) = FloatFormat::of(ty) {
            return FloatCell::from_bytes(format, bytes).map(Cell::Float).ok_or_else(wrong);
        }
        WideCell::from_bytes(ty, bytes).map(Cell::Wide).ok_or_else(wrong)
    }
}
