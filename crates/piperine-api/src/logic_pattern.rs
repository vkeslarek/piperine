//! `LogicPattern`: the four-state text a host writes for a `Logic<N>` (L71, §22.1.4, EU-15 AC 4). It is the text the host
//! prints a `Logic<N>` cell as, read back: one character per level, highest level first, each of `0`, `1`, `x`, `z`. Any
//! other character, or a length other than `N`, is `E0702`. The value is laid out as the library's struct is on a pin:
//! the plane `binary` at offset 0 and the plane `value` after it, level `k` at bit `k` of each, 0 as binary 1 and value 0,
//! 1 as both, X as neither, Z as value alone (`abi.md` P3).

use piperine_ir::ValueType;

use crate::composite::CellShape;
use crate::failure::Failure;
use crate::number_cell::{FloatFormat, NumberCell, WideCell};

/// The levels of a pattern, as one text wrote them, and the width of the `Logic` it is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicPattern {
    /// The levels, highest first, as characters of `0`, `1`, `x` and `z`.
    levels: Vec<char>,
}

impl LogicPattern {
    /// `text` as a pattern of `width` levels: `E0702` for a character outside `0 1 x z` and for a length other than `width`,
    /// naming what the host wrote it for (`what`).
    pub fn parse(text: &str, width: u32, what: &str) -> Result<LogicPattern, Failure> {
        let text = text.trim();
        if let Some(bad) = text.chars().find(|c| !matches!(c, '0' | '1' | 'x' | 'z')) {
            return Err(Failure::Host { code: "E0702", message: format!("`{text}` for {what}: the character `{bad}` is none of `0`, `1`, `x` and `z` (§22.1.4)") });
        }
        let length = text.chars().count();
        if length != width as usize {
            return Err(Failure::Host { code: "E0702", message: format!("`{text}` for {what} has {length} levels, and the value has {width} (§22.1.4)") });
        }
        Ok(LogicPattern { levels: text.chars().collect() })
    }

    /// The bytes of a `Logic<N>` pin holding this pattern, whose two planes are `plane` bytes each: `binary` then `value`, level
    /// `k` at bit `k` of its plane, little-endian.
    pub fn bytes(&self, plane: usize) -> Vec<u8> {
        let mut bytes = vec![0u8; 2 * plane];
        for (position, level) in self.levels.iter().rev().enumerate() {
            let (binary, value) = match level {
                '0' => (true, false),
                '1' => (true, true),
                'z' => (false, true),
                _ => (false, false),
            };
            let (byte, bit) = (position / 8, position % 8);
            bytes[byte] |= u8::from(binary) << bit;
            bytes[plane + byte] |= u8::from(value) << bit;
        }
        bytes
    }

    /// The width of the planes of the library's `Logic<N>` in `shape` in bytes, and `N`; `None` for any other shape (a project's
    /// own struct of the same name included).
    pub fn planes_of(shape: &CellShape) -> Option<(u32, usize)> {
        let width = shape.logic_levels()?;
        let CellShape::Struct { fields, .. } = shape else { return None };
        let [(_, CellShape::Scalar(plane)), _] = fields.as_slice() else { return None };
        let bytes = match plane {
            number if NumberCell::is_number_width(number) => FloatFormat::of(number).map_or_else(|| 8 * WideCell::limb_count(Self::levels_of(number)), FloatFormat::bytes),
            other => shape_width(other),
        };
        Some((width, bytes))
    }

    /// The levels of an integer plane type.
    fn levels_of(ty: &ValueType) -> u32 {
        match ty {
            ValueType::UInt(n) | ValueType::Int(n) => *n,
            _ => 1,
        }
    }
}

/// The bytes of a plane that is no number width: an integer of up to 64 levels.
fn shape_width(ty: &ValueType) -> usize {
    CellShape::Scalar(ty.clone()).width()
}
