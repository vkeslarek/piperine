//! The shipped layouts (abi.md §3.1 P3) and the byte rules the solver needs: layout equality
//! (P4).

use crate::raw::{Field, Layout, List, Str, leaf};

const V: Str = Str::from_static("v");

static BIT_FIELDS: [Field; 1] = [Field { name: V, leaf: leaf::BOOL, offset: 0, count: 1 }];
static BYTE_FIELDS: [Field; 1] = [Field { name: V, leaf: leaf::U8, offset: 0, count: 1 }];
static F64_FIELDS: [Field; 1] = [Field { name: V, leaf: leaf::F64, offset: 0, count: 1 }];

/// `{ v: BOOL @0 }`, size 1, align 1.
pub static BIT: Layout = Layout { size: 1, align: 1, fields: List::from_slice(&BIT_FIELDS) };
/// `{ v: U8 @0 }`, size 1, align 1.
pub static BYTE: Layout = Layout { size: 1, align: 1, fields: List::from_slice(&BYTE_FIELDS) };
/// `{ v: F64 @0 }`, size 8, align 8: the layout of a continuous pin (abi.md §3) and of an `f64`.
pub static F64: Layout = Layout { size: 8, align: 8, fields: List::from_slice(&F64_FIELDS) };
/// No bytes: the layout of an event pin (abi.md §3).
pub static NONE: Layout = Layout { size: 0, align: 1, fields: List::empty() };

/// A false `BIT`.
pub const LOW: [u8; 1] = [0];
/// A true `BIT`.
pub const HIGH: [u8; 1] = [1];

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

/// P4: equal size and alignment and, per field, equal leaf, offset and count. Names do not
/// count.
pub fn layout_eq(a: &Layout, b: &Layout) -> bool {
    a.size == b.size
        && a.align == b.align
        && a.fields.len() == b.fields.len()
        && a.fields.iter().zip(b.fields.iter()).all(|(x, y)| {
            x.leaf == y.leaf && x.offset == y.offset && x.count == y.count
        })
}
