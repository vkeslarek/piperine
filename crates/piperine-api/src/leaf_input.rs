//! `LeafInput` and `LeafEncoding`: a value a host writes to one leaf of a composite param (`--param Top.pat.binary=<n>`,
//! `Session::set_at`), and the bytes the leaf holds for it (abi.md P3). An integer leaf is written exactly, from its digits or
//! from an `f64` that is a whole number, in the range of the leaf's own type (a `Logic<60>` plane is a `UInt<60>`), whatever its
//! width: no value is rounded through an `f64` (PI-01 AC 4, Part VIII §23.1 rule 13).

use piperine_abi::raw::leaf;
use piperine_ir::ValueType;
use piperine_plan::CompositeLeaf;

use crate::number_cell::WideCell;

/// A value a host gives for one leaf of a composite param.
#[derive(Debug, Clone, PartialEq)]
pub enum LeafInput {
    /// A number: a real, a flag as 0 or 1, an integer as a whole number.
    Number(f64),
    /// Text read by the leaf's type: `true` or `false`, a real, an integer literal (decimal, `0x`, `0o`, `0b`, `_` between digits).
    Text(String),
}

impl LeafInput {
    /// The value as a refusal shows it: a number bare, a text in backticks, as the host wrote it.
    pub fn shown(&self) -> String {
        match self {
            LeafInput::Number(value) => value.to_string(),
            LeafInput::Text(text) => format!("`{}`", text.trim()),
        }
    }
}

/// One leaf of a composite pin, with the type of its scalar: what decides the bytes a value takes in it.
pub(crate) struct LeafEncoding<'a> {
    /// The leaf in the pin's layout: its `leaf::*` kind and how many elements of it the field holds.
    pub leaf: &'a CompositeLeaf,
    /// The leaf's own type (`UInt<60>` for a `Logic<60>` plane); `None` for an enum's tag or an `Option`'s presence byte, whose
    /// range is the layout's storage.
    pub ty: Option<ValueType>,
}

impl LeafEncoding<'_> {
    /// `input` as the bytes of the leaf, little-endian; the reason it cannot be when it cannot.
    pub fn bytes(&self, input: &LeafInput) -> Result<Vec<u8>, String> {
        match (self.leaf.leaf, input) {
            (leaf::F64, LeafInput::Number(value)) => Ok(value.to_le_bytes().to_vec()),
            (leaf::F64, LeafInput::Text(text)) => text.trim().parse::<f64>().map(|value| value.to_le_bytes().to_vec()).map_err(|_| "it is not a number".to_string()),
            (leaf::BOOL, LeafInput::Number(value)) if *value == 0.0 || *value == 1.0 => Ok(vec![u8::from(*value == 1.0)]),
            (leaf::BOOL, LeafInput::Number(_)) => Err("the leaf holds a whole number from 0 to 1".to_string()),
            (leaf::BOOL, LeafInput::Text(text)) => match text.trim() {
                "true" => Ok(vec![1]),
                "false" => Ok(vec![0]),
                _ => Err("a `bool` leaf is `true` or `false`".to_string()),
            },
            (kind, input) if self.storage_type().is_some() => self.integer(input).ok_or_else(|| match self.integer_type() {
                Some(ty) => format!("the leaf holds a whole number of type {ty}, in its range"),
                None => format!("leaf kind {kind} has no host value"),
            }),
            (kind, _) => Err(format!("leaf kind {kind} has no host value")),
        }
    }

    /// The exact bytes of an integer leaf for `input`: its digits, or a whole `f64` written out in full, read in the range of the
    /// leaf's type; `None` when it is no such integer.
    fn integer(&self, input: &LeafInput) -> Option<Vec<u8>> {
        let (signed, width) = match self.integer_type()? {
            ValueType::UInt(width) => (false, width),
            ValueType::Int(width) => (true, width),
            _ => return None,
        };
        let text = match input {
            LeafInput::Text(text) => text.trim().to_string(),
            // an `f64` that is a whole number is exactly the integer its full decimal expansion spells (`-0` is 0)
            LeafInput::Number(value) if value.is_finite() && value.fract() == 0.0 => format!("{:.0}", if *value == 0.0 { 0.0 } else { *value }),
            LeafInput::Number(_) => return None,
        };
        let mut bytes = WideCell::parse(signed, width, &text)?.to_bytes();
        bytes.resize(self.width(), 0);
        Some(bytes)
    }

    /// The type an integer leaf's value is read in: its own when the shape names it, else its storage.
    fn integer_type(&self) -> Option<ValueType> {
        self.ty.clone().filter(ValueType::is_integer).or_else(|| self.storage_type())
    }

    /// The integer type the leaf's storage holds: `UInt<8>` for a `U8`, `UInt<128>` for two `U64`s; `None` for a leaf that is no
    /// integer.
    fn storage_type(&self) -> Option<ValueType> {
        let bits = 8 * u32::try_from(self.width()).ok()?;
        match self.leaf.leaf {
            leaf::U8 | leaf::U16 | leaf::U32 | leaf::U64 => Some(ValueType::UInt(bits)),
            leaf::I8 | leaf::I16 | leaf::I32 | leaf::I64 => Some(ValueType::Int(bits)),
            _ => None,
        }
    }

    /// The bytes the leaf takes: its element's width times the elements the field holds.
    fn width(&self) -> usize {
        let element = match self.leaf.leaf {
            leaf::BOOL | leaf::U8 | leaf::I8 => 1,
            leaf::U16 | leaf::I16 => 2,
            leaf::U32 | leaf::I32 | leaf::F32 => 4,
            _ => 8,
        };
        element * self.leaf.count.max(1) as usize
    }
}
