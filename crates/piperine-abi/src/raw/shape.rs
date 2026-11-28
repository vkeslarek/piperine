//! The shape of abi.md §3: the complete, immutable description of an instance.

use super::{Bytes, List, Str, leaf};

// ─── The shape ──────────────────────────────────────────────────────────────

/// The complete, immutable description of an instance.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Shape<'a> {
    pub size: u32,
    pub pins: List<'a, Pin<'a>>,
    pub terms: List<'a, Term<'a>>,
    pub inputs: List<'a, Input<'a>>,
    pub watches: List<'a, Watch<'a>>,
    pub functions: List<'a, Function<'a>>,
    pub reactions: List<'a, Reaction<'a>>,
}

/// A net seen from the instance (abi.md §3.1).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Pin<'a> {
    pub name: Str<'a>,
    /// `"file:line:col"` of the source item.
    pub origin: Str<'a>,
    /// `kind::CONTINUOUS | DISCRETE | EVENT`.
    pub kind: u32,
    /// `role::PORT | PARAM | GLOBAL | PRIVATE`.
    pub role: u32,
    /// Role `GLOBAL`: a `global::*` constant.
    pub global: u32,
    /// `pin_flag::*` bits.
    pub flags: u32,
    /// `CONTINUOUS`: one `F64`. `EVENT`: empty.
    pub layout: Layout<'a>,
    pub unit: Str<'a>,
    /// `HAS_INIT`: the first commit at (0,0), a `PARAM`'s default, or Newton's start.
    pub init: Bytes<'a>,
    /// `HAS_CHECK`: a discrete `BOOL` pin that must be true for a host write to be accepted.
    pub check: u32,
    /// `CONTINUOUS`: tolerance of the unknown, of its row; 0 = session default.
    pub abstol: [f64; 2],
    /// `DISCRETE`: 0 is one carrier (every pin before arrays of carriers); `>= 1` makes this an array
    /// pin of `count` elements, each of `layout` (abi.md §3.1, appended in place at `MINOR` 0).
    pub count: u64,
}

/// The only description of bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Layout<'a> {
    pub size: u32,
    pub align: u32,
    pub fields: List<'a, Field<'a>>,
}

/// One scalar leaf, or an array of them, inside a layout.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Field<'a> {
    pub name: Str<'a>,
    /// A `leaf::*` constant, little-endian.
    pub leaf: u32,
    pub offset: u32,
    /// Array length.
    pub count: u32,
}

impl Layout<'_> {
    /// P4: equal size and alignment and, per field, equal leaf, offset and count. Names do not
    /// count.
    pub fn equals(&self, other: &Layout) -> bool {
        self.size == other.size
            && self.align == other.align
            && self.fields.len() == other.fields.len()
            && self.fields.iter().zip(other.fields.iter()).all(|(x, y)| {
                x.leaf == y.leaf && x.offset == y.offset && x.count == y.count
            })
    }
}

impl Field<'_> {
    /// The size in bytes of a `leaf::*` constant; `None` for an unknown one.
    pub fn leaf_size(constant: u32) -> Option<usize> {
        match constant {
            leaf::BOOL | leaf::U8 | leaf::I8 => Some(1),
            leaf::U16 | leaf::I16 => Some(2),
            leaf::U32 | leaf::I32 | leaf::F32 => Some(4),
            leaf::U64 | leaf::I64 | leaf::F64 => Some(8),
            _ => None,
        }
    }
}

/// A real value added to a row through an operator.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Term<'a> {
    pub name: Str<'a>,
    pub origin: Str<'a>,
    /// `op::FLOW | DDT | DELAY`.
    pub op: u32,
    /// A continuous pin, or `GROUND`.
    pub plus: u32,
    pub minus: u32,
    /// `DDT` state tolerance: a discrete `F64` pin of this instance, or `DEFAULT`.
    pub abstol: u32,
    /// `DELAY` bound: a discrete `F64` pin of this instance, or `DEFAULT`.
    pub max: u32,
}

/// A held input (abi.md §4.2).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Input<'a> {
    pub name: Str<'a>,
    pub origin: Str<'a>,
    /// `input::LIMIT | NOISE | STIM`.
    pub kind: u32,
}

/// An analog output that enters no row (abi.md §4.3).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Watch<'a> {
    pub name: Str<'a>,
    pub origin: Str<'a>,
    /// `watch::VAR | CROSS | CAP | TRACE`.
    pub kind: u32,
    /// `VAR`, `TRACE` (`CROSS`, `CAP`: one `F64`).
    pub layout: Layout<'a>,
    /// `CROSS`: the event pin it fires.
    pub event: u32,
    /// Tolerance pins, or `DEFAULT`.
    pub ttol: u32,
    pub etol: u32,
}

/// The address of one thing a function or reaction reads or writes.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    /// `cell::PIN | TERM | INPUT | WATCH | AMBIENT`.
    pub kind: u32,
    /// The entry's index; for `AMBIENT`, an `ambient::*` constant.
    pub index: u32,
    /// A `part::*` constant.
    pub part: u32,
}

/// `d(writes[write], the F64 at byte to) / d(reads[read], the F64 at byte at)`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Partial {
    pub write: u32,
    pub to: u32,
    pub read: u32,
    pub at: u32,
}

/// Pure code with declared reads and writes.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Function<'a> {
    pub name: Str<'a>,
    pub origin: Str<'a>,
    /// `fn_flag::*` bits.
    pub flags: u32,
    pub reads: List<'a, Cell>,
    pub writes: List<'a, Cell>,
    pub partials: List<'a, Partial>,
    /// Empty, or one entry per `reads` entry: the element of an array pin that read names, or [`WHOLE`]
    /// (abi.md §3, arrays of carriers). A read of a non-array pin carries 0.
    pub elements: List<'a, u64>,
}

/// Code run once at an instant when an event pin it listens to fires.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Reaction<'a> {
    pub name: Str<'a>,
    pub origin: Str<'a>,
    /// `reaction_flag::*` bits.
    pub flags: u32,
    /// Event pins.
    pub on: List<'a, u32>,
    pub reads: List<'a, Cell>,
    pub writes: List<'a, Statement>,
    /// Empty, or one entry per `reads` entry: the element of an array pin that read names, or [`WHOLE`]
    /// (abi.md §3, arrays of carriers). A read of a non-array pin carries 0.
    pub elements: List<'a, u64>,
}

/// One declared write of a reaction.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Statement {
    pub pin: u32,
    pub offset: u32,
    pub len: u32,
    /// `statement_flag::*` bits.
    pub flags: u32,
    /// `LATER`: every delay is at least this, in seconds.
    pub min_after: f64,
}

