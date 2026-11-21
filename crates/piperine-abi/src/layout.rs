//! The shipped layouts (abi.md §3.1 P3) and the byte rules the solver needs: layout equality
//! (P4), the canonical form (P3) and the merge of two values of a layout (`Merge`, abi.md C7).

use std::fmt;

use crate::known::FieldKnown;
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

/// P4: equal size and alignment and, per field, equal leaf, offset, count and known flags (the
/// offset, and whether it is a mask: abi.md §3 `KNOWN_LEVELS`). Names do not count.
pub fn layout_eq(a: &Layout, b: &Layout) -> bool {
    a.size == b.size
        && a.align == b.align
        && a.fields.len() == b.fields.len()
        && a.fields.iter().zip(b.fields.iter()).all(|(x, y)| {
            x.leaf == y.leaf
                && x.offset == y.offset
                && x.count == y.count
                && FieldKnown::form_of(x) == FieldKnown::form_of(y)
        })
}

/// P3: every element of a field with known flags holds all-zero value bytes where no flag is set
/// (a plain known byte of 0, or the value bits of the levels a mask leaves unknown). Bytes
/// shorter than the layout are not canonical.
pub fn is_canonical(layout: &Layout, bytes: &[u8]) -> bool {
    if bytes.len() < layout.size as usize {
        return false;
    }
    layout.fields.iter().all(|f| {
        if FieldKnown::form_of(f).is_none() {
            return true;
        }
        FieldKnown::of(f).is_some_and(|known| (0..known.count()).all(|element| known.is_canonical(bytes, element)))
    })
}

/// One field of a layout as a merge reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct MergeField {
    /// The field's name, for an error.
    name: String,
    /// Whether the field has known flags at all.
    has_known: bool,
    /// The reader of its known flags, `None` for a field without them or with an undefined leaf.
    known: Option<FieldKnown>,
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
/// canonically (P3: known byte 0 and value bytes zero). A field with a mask (`KNOWN_LEVELS`)
/// merges each level on its own: the levels both sides hold known and equal stay, the others are
/// cleared, value bits included. Values are compared as bytes, so `+0.0` and `-0.0` disagree,
/// which only ever errs toward unknown.
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
                has_known: FieldKnown::form_of(f).is_some(),
                known: FieldKnown::of(f),
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
            if !field.has_known {
                return Err(MergeError::NoKnownByte { field: field.name.clone() });
            }
            if field.known.is_none() {
                return Err(MergeError::UnknownLeaf { field: field.name.clone() });
            }
        }
        for known in self.fields.iter().filter_map(|field| field.known) {
            for element in 0..known.count() {
                known.merge(settled, committed, element);
            }
        }
        Ok(())
    }
}
