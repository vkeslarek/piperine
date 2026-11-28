//! The shipped layouts (abi.md §3.1 P3). Layout equality (P4) is `Layout::equals` and a
//! leaf's width `Field::leaf_size`.

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
