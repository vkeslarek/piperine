//! The shipped layouts (abi.md §3.1 P3) and the byte rules the solver needs: layout equality
//! (P4) and the canonical form (P3).

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
