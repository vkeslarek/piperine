//! The shipped layouts (abi.md §3.1 P3) and the byte rules the solver needs: layout equality
//! (P4), the canonical form (P3) and the merge of two values of a layout (`Merge`, abi.md C7).

use std::fmt;

use crate::raw::{Field, Layout, List, Str, field_flag, leaf};

const V: Str = Str::from_static("v");

static BIT_FIELDS: [Field; 1] =
    [Field { name: V, leaf: leaf::BOOL, offset: 0, count: 1, flags: field_flag::HAS_KNOWN, known: 1 }];
static REAL_FIELDS: [Field; 1] =
    [Field { name: V, leaf: leaf::F64, offset: 0, count: 1, flags: field_flag::HAS_KNOWN, known: 8 }];
static BYTE_FIELDS: [Field; 1] =
    [Field { name: V, leaf: leaf::U8, offset: 0, count: 1, flags: field_flag::HAS_KNOWN, known: 1 }];

/// `{ v: BOOL @0, known @1 }`, size 2, align 1.
pub static BIT: Layout = Layout { size: 2, align: 1, fields: List::from_slice(&BIT_FIELDS) };
/// `{ v: F64 @0, known @8 }`, size 16, align 8.
pub static REAL: Layout = Layout { size: 16, align: 8, fields: List::from_slice(&REAL_FIELDS) };
/// `{ v: U8 @0, known @1 }`, size 2, align 1.
pub static BYTE: Layout = Layout { size: 2, align: 1, fields: List::from_slice(&BYTE_FIELDS) };

/// One `F64` with no known byte: the layout of a continuous pin (abi.md §3).
pub static F64: Layout = Layout { size: 8, align: 8, fields: List::from_slice(&F64_FIELDS) };
static F64_FIELDS: [Field; 1] =
    [Field { name: V, leaf: leaf::F64, offset: 0, count: 1, flags: 0, known: 0 }];
/// No bytes: the layout of an event pin (abi.md §3).
pub static NONE: Layout = Layout { size: 0, align: 1, fields: List::empty() };

/// A known false `BIT`.
pub const LOW: [u8; 2] = [0, 1];
/// A known true `BIT`.
pub const HIGH: [u8; 2] = [1, 1];

/// The size in bytes of a `leaf::*` constant; `None` for an unknown one.
pub fn leaf_size(l: u32) -> Option<usize> {
    match l {
        leaf::BOOL | leaf::U8 | leaf::I8 => Some(1),
        leaf::U16 | leaf::I16 => Some(2),
        leaf::U32 | leaf::I32 | leaf::F32 => Some(4),
        leaf::U64 | leaf::I64 | leaf::F64 => Some(8),
        _ => None,
    }
}

/// The offset of a field's known bytes, `None` when it has none.
fn known_of(f: &Field) -> Option<u32> {
    (f.flags & field_flag::HAS_KNOWN != 0).then_some(f.known)
}

/// P4: equal size and alignment and, per field, equal leaf, offset, count and
/// known offset. Names do not count.
pub fn layout_eq(a: &Layout, b: &Layout) -> bool {
    a.size == b.size
        && a.align == b.align
        && a.fields.len() == b.fields.len()
        && a.fields.iter().zip(b.fields.iter()).all(|(x, y)| {
            x.leaf == y.leaf && x.offset == y.offset && x.count == y.count && known_of(x) == known_of(y)
        })
}

/// P3: every element of a field with a known byte whose known byte is 0 has
/// all-zero value bytes. Bytes shorter than the layout are not canonical.
pub fn is_canonical(layout: &Layout, bytes: &[u8]) -> bool {
    if bytes.len() < layout.size as usize {
        return false;
    }
    layout.fields.iter().all(|f| {
        let Some(known) = known_of(f) else { return true };
        let Some(width) = leaf_size(f.leaf) else { return false };
        (0..f.count as usize).all(|e| {
            let value = f.offset as usize + e * width;
            match (bytes.get(known as usize + e), bytes.get(value..value + width)) {
                (Some(0), Some(v)) => v.iter().all(|b| *b == 0),
                (Some(_), Some(_)) => true,
                _ => false,
            }
        })
    })
}

/// One field of a layout as a merge reads it: where its value elements and known bytes lie.
#[derive(Clone, Debug, PartialEq, Eq)]
struct MergeField {
    /// The field's name, for an error.
    name: String,
    /// The offset of the first value element.
    value: usize,
    /// The size of one element, `None` for a leaf this ABI does not define.
    width: Option<usize>,
    /// The array length.
    count: usize,
    /// The offset of the first known byte, `None` for a field that has none.
    known: Option<usize>,
}

/// Why two values could not be merged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MergeError {
    /// A field has no known byte, so "unknown" cannot be said of it (abi.md C7).
    NoKnownByte { field: String },
    /// A field's leaf is not one of `leaf::*`.
    UnknownLeaf { field: String },
    /// A value is shorter than the layout.
    Short { have: usize, need: usize },
}

impl fmt::Display for MergeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MergeError::NoKnownByte { field } => write!(f, "field `{field}` has no known byte: it cannot be merged"),
            MergeError::UnknownLeaf { field } => write!(f, "field `{field}` has a leaf this ABI does not define"),
            MergeError::Short { have, need } => write!(f, "a value of {have} bytes is shorter than its layout's {need}"),
        }
    }
}

impl std::error::Error for MergeError {}

/// The merge of two values of one layout, level by level (part I §3.3.3, abi.md C7): per element
/// of each field, a known value both sides agree on is kept; anything else becomes unknown,
/// canonically (P3: known byte 0 and value bytes zero). Values are compared as bytes, so `+0.0`
/// and `-0.0` disagree, which only ever errs toward unknown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Merge {
    /// The layout's size: the least length of a value.
    size: usize,
    fields: Vec<MergeField>,
}

impl Merge {
    /// The merge of values of `layout`.
    pub fn of(layout: &Layout) -> Merge {
        let fields = layout
            .fields
            .iter()
            .map(|f| MergeField {
                name: f.name.as_str().to_string(),
                value: f.offset as usize,
                width: leaf_size(f.leaf),
                count: f.count as usize,
                known: known_of(f).map(|k| k as usize),
            })
            .collect();
        Merge { size: layout.size as usize, fields }
    }

    /// Merges `committed` into `settled`, which then holds the result. Nothing is written when
    /// the merge is refused: a field without a known byte, an undefined leaf or a short value.
    pub fn merge_into(&self, settled: &mut [u8], committed: &[u8]) -> Result<(), MergeError> {
        for bytes in [settled.len(), committed.len()] {
            if bytes < self.size {
                return Err(MergeError::Short { have: bytes, need: self.size });
            }
        }
        for field in &self.fields {
            if field.known.is_none() {
                return Err(MergeError::NoKnownByte { field: field.name.clone() });
            }
            if field.width.is_none() {
                return Err(MergeError::UnknownLeaf { field: field.name.clone() });
            }
        }
        for field in &self.fields {
            let (Some(known), Some(width)) = (field.known, field.width) else { continue };
            for element in 0..field.count {
                let value = field.value + element * width..field.value + (element + 1) * width;
                let agree = settled[known + element] != 0 && committed[known + element] != 0 && settled[value.clone()] == committed[value.clone()];
                if !agree {
                    settled[known + element] = 0;
                    settled[value].fill(0);
                }
            }
        }
        Ok(())
    }
}
