//! The C contract of abi.md §3: every struct `#[repr(C)]`
//! with its fields in the listed order, every enumeration a group of named
//! `u32` constants. Values start at 1 (flags at bit 0), so an all-zero entry
//! never carries a valid kind. Beside it, the one typed view of a cell's
//! bytes (`Pod`, `pod_ref`, `pod_mut`, `pod_bytes`) the lenses use.

use core::ffi::c_void;
use core::mem::{align_of, size_of};

// ─── Strings, bytes, lists ──────────────────────────────────────────────────

/// UTF-8 text owned by the producer.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Str {
    pub ptr: *const u8,
    pub len: usize,
}

/// Raw bytes owned by the producer.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Bytes {
    pub ptr: *const u8,
    pub len: usize,
}

/// A list whose elements lie `stride` bytes apart: the element size as the
/// producer compiled it. A consumer reads the fields it knows (abi.md §3, §12
/// "Versioning").
#[repr(C)]
#[derive(Debug)]
pub struct List<T> {
    pub ptr: *const T,
    pub len: usize,
    pub stride: usize,
}

impl<T> Clone for List<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for List<T> {}

// SAFETY: abi.md §3 "The complete, immutable description": the text a `Str`
// points to is never written after it is produced.
unsafe impl Send for Str {}
// SAFETY: as for `Send` above; shared reads of immutable bytes do not race.
unsafe impl Sync for Str {}
// SAFETY: abi.md §3: the bytes of a `Value` or `init` are immutable once produced.
unsafe impl Send for Bytes {}
// SAFETY: as for `Send` above.
unsafe impl Sync for Bytes {}
// SAFETY: abi.md §3 "immutable description": a list's elements are never
// written after it is produced, so sharing it is sharing `&T`.
unsafe impl<T: Sync> Send for List<T> {}
// SAFETY: as for `Send` above.
unsafe impl<T: Sync> Sync for List<T> {}

impl Str {
    /// The empty string, with a non-null pointer.
    pub const EMPTY: Str = Str { ptr: core::ptr::NonNull::<u8>::dangling().as_ptr(), len: 0 };

    /// A `Str` over a static string.
    pub const fn from_static(s: &'static str) -> Str {
        Str { ptr: s.as_ptr(), len: s.len() }
    }

    /// The bytes of the string.
    pub fn as_bytes(&self) -> &[u8] {
        if self.len == 0 {
            return &[];
        }
        // SAFETY: abi.md §12 "Ownership": a `Str` of a shape, library or frame
        // points to `len` readable bytes for as long as its owner lives.
        unsafe { core::slice::from_raw_parts(self.ptr, self.len) }
    }

    /// The text, or `"<invalid utf-8>"` when the producer broke abi.md §3 ("UTF-8").
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(self.as_bytes()).unwrap_or("<invalid utf-8>")
    }
}

impl Bytes {
    /// No bytes, with a non-null pointer.
    pub const EMPTY: Bytes = Bytes { ptr: core::ptr::NonNull::<u8>::dangling().as_ptr(), len: 0 };

    /// A `Bytes` over a static slice.
    pub const fn from_static(b: &'static [u8]) -> Bytes {
        Bytes { ptr: b.as_ptr(), len: b.len() }
    }

    /// The bytes.
    pub fn as_slice(&self) -> &[u8] {
        if self.len == 0 {
            return &[];
        }
        // SAFETY: abi.md §12 "Ownership": a `Bytes` points to `len` readable
        // bytes for as long as its owner lives.
        unsafe { core::slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl<T> List<T> {
    /// The empty list: a non-null, aligned pointer and no element.
    pub const fn empty() -> List<T> {
        List { ptr: core::ptr::NonNull::<T>::dangling().as_ptr(), len: 0, stride: size_of::<T>() }
    }

    /// A list over a slice, with this compilation's stride.
    pub const fn from_slice(items: &[T]) -> List<T> {
        if items.is_empty() {
            return List::empty();
        }
        List { ptr: items.as_ptr(), len: items.len(), stride: size_of::<T>() }
    }

    /// Element `i`, read at `ptr + i * stride`; `None` past the end.
    pub fn get(&self, i: usize) -> Option<&T> {
        if i >= self.len {
            return None;
        }
        // SAFETY: abi.md §3 "stride = element size as the producer compiled it"
        // and §12 "Ownership": `len` elements lie `stride` bytes apart and live
        // as long as the list's owner; the producer compiled at least the
        // fields of `T` this version knows, so `T` is readable there.
        unsafe { Some(&*self.ptr.cast::<u8>().add(i * self.stride).cast::<T>()) }
    }

    /// Every element in order, each read by `stride`.
    pub fn iter(&self) -> impl Iterator<Item = &T> + '_ {
        (0..self.len).filter_map(move |i| self.get(i))
    }

    /// The number of elements.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the list has no element.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

// ─── Library, module, instance ──────────────────────────────────────────────

/// The value `piperine_abi_1()` returns.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Library {
    pub size: u32,
    pub minor: u32,
    pub name: Str,
    pub build: Str,
    pub modules: List<Module>,
}

/// `create` of abi.md §2.
pub type CreateFn = unsafe extern "C" fn(*const Module, List<Value>, *mut Note) -> Created;
/// `destroy` of abi.md §2.
pub type DestroyFn = unsafe extern "C" fn(*mut Instance);
/// `eval` of abi.md §2.
pub type EvalFn = unsafe extern "C" fn(*mut Instance, u32, *const Eval) -> u32;
/// `react` of abi.md §2.
pub type ReactFn = unsafe extern "C" fn(*mut Instance, u32, *const React) -> u32;
/// `commit` of abi.md §2, the solver's entry.
pub type CommitFn =
    unsafe extern "C" fn(*mut c_void, statement: u32, offset: u32, len: u32, bytes: *const u8, after: f64) -> u32;

/// A named factory in a library.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Module {
    pub name: Str,
    /// The shape at default values: lists every `PARAM` pin, its layout, its default.
    pub prototype: *const Shape,
    /// The module's own data (an interpreter's IR, an OSDI descriptor).
    pub data: *const c_void,
    pub create: CreateFn,
    pub destroy: DestroyFn,
    pub eval: EvalFn,
    pub react: ReactFn,
}

// SAFETY: abi.md §12 "The library and its prototypes live until the process
// ends" and "A library has no mutable global state": a `Module` is immutable
// data plus functions callable from any thread on distinct instances.
unsafe impl Send for Module {}
// SAFETY: as for `Send` above.
unsafe impl Sync for Module {}

/// An initial value of a `PARAM` pin of the prototype, in its layout.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Value {
    pub pin: u32,
    pub bytes: Bytes,
}

/// The result of `create`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Created {
    pub status: u32,
    pub instance: *mut Instance,
    pub shape: *const Shape,
}

/// An instance, opaque to the solver.
#[repr(C)]
pub struct Instance {
    _opaque: [u8; 0],
}

/// A message and a device-chosen code, written on `FAULT`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Note {
    pub code: u32,
    pub len: u32,
    pub text: [u8; NOTE_TEXT],
}

/// The capacity of `Note.text` in bytes.
pub const NOTE_TEXT: usize = 248;

// ─── The shape ──────────────────────────────────────────────────────────────

/// The complete, immutable description of an instance.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Shape {
    pub size: u32,
    pub pins: List<Pin>,
    pub terms: List<Term>,
    pub inputs: List<Input>,
    pub watches: List<Watch>,
    pub functions: List<Function>,
    pub reactions: List<Reaction>,
}

/// A net seen from the instance (abi.md §3.1).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Pin {
    pub name: Str,
    /// `"file:line:col"` of the source item.
    pub origin: Str,
    /// `kind::CONTINUOUS | DISCRETE | EVENT`.
    pub kind: u32,
    /// `role::PORT | PARAM | GLOBAL | PRIVATE`.
    pub role: u32,
    /// Role `GLOBAL`: a `global::*` constant.
    pub global: u32,
    /// `pin_flag::*` bits.
    pub flags: u32,
    /// `CONTINUOUS`: one `F64`. `EVENT`: empty.
    pub layout: Layout,
    pub unit: Str,
    /// `HAS_INIT`: the first commit at (0,0), a `PARAM`'s default, or Newton's start.
    pub init: Bytes,
    /// `HAS_CHECK`: a discrete `BOOL` pin that must be true for a host write to be accepted.
    pub check: u32,
    /// `CONTINUOUS`: tolerance of the unknown, of its row; 0 = session default.
    pub abstol: [f64; 2],
}

/// The only description of bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub size: u32,
    pub align: u32,
    pub fields: List<Field>,
}

/// One scalar leaf, or an array of them, inside a layout.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Field {
    pub name: Str,
    /// A `leaf::*` constant, little-endian.
    pub leaf: u32,
    pub offset: u32,
    /// Array length.
    pub count: u32,
    /// `field_flag::*` bits.
    pub flags: u32,
    /// `HAS_KNOWN`: the offset of one byte per element; 0 there = no value.
    pub known: u32,
}

/// A real value added to a row through an operator.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Term {
    pub name: Str,
    pub origin: Str,
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
pub struct Input {
    pub name: Str,
    pub origin: Str,
    /// `input::LIMIT | NOISE | STIM`.
    pub kind: u32,
}

/// An analog output that enters no row (abi.md §4.3).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Watch {
    pub name: Str,
    pub origin: Str,
    /// `watch::VAR | CROSS | CAP | TRACE`.
    pub kind: u32,
    /// `VAR`, `TRACE` (`CROSS`, `CAP`: one `F64`).
    pub layout: Layout,
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
pub struct Function {
    pub name: Str,
    pub origin: Str,
    /// `fn_flag::*` bits.
    pub flags: u32,
    pub reads: List<Cell>,
    pub writes: List<Cell>,
    pub partials: List<Partial>,
}

/// Code run once at an instant when an event pin it listens to fires.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Reaction {
    pub name: Str,
    pub origin: Str,
    /// `reaction_flag::*` bits.
    pub flags: u32,
    /// Event pins.
    pub on: List<u32>,
    pub reads: List<Cell>,
    pub writes: List<Statement>,
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

// ─── The frames ─────────────────────────────────────────────────────────────

/// The frame of `eval`. Every pointer is non-null and has the length the shape says.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Eval {
    pub size: u32,
    /// `[reads.len]` the bytes of each read cell.
    pub input: *const *const u8,
    /// `[writes.len]` a buffer per written cell, pre-filled with its current bytes.
    pub output: *const *mut u8,
    /// `[partials.len]` 1 = this partial is wanted.
    pub seed: *const u8,
    /// `[partials.len]` pre-zeroed.
    pub jac: *mut f64,
    pub note: *mut Note,
}

/// The frame of `react`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct React {
    pub size: u32,
    /// `[reads.len]`.
    pub input: *const *const u8,
    pub host: *const Host,
    pub host_data: *mut c_void,
    pub note: *mut Note,
}

/// The solver's side of a `react` call.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Host {
    /// A later minor may append fields.
    pub size: u32,
    pub commit: CommitFn,
}

// ─── Constants ──────────────────────────────────────────────────────────────

/// The `plus`/`minus` of a term that names the reference node.
pub const GROUND: u32 = u32::MAX;
/// A tolerance or bound index meaning "the session default".
pub const DEFAULT: u32 = u32::MAX - 1;
/// The ABI major; it is the exported symbol's suffix.
pub const MAJOR: u32 = 1;
/// The ABI minor this crate implements.
pub const MINOR: u32 = 0;
/// The one exported symbol of a library.
pub const SYMBOL: &str = "piperine_abi_1";

/// `Pin.kind`.
pub mod kind {
    pub const CONTINUOUS: u32 = 1;
    pub const DISCRETE: u32 = 2;
    pub const EVENT: u32 = 3;
}

/// `Pin.role`.
pub mod role {
    pub const PORT: u32 = 1;
    pub const PARAM: u32 = 2;
    pub const GLOBAL: u32 = 3;
    pub const PRIVATE: u32 = 4;
}

/// `Pin.global`, for role `GLOBAL`.
pub mod global {
    pub const TEMPERATURE: u32 = 1;
    pub const RESOLUTION: u32 = 2;
    pub const DC: u32 = 3;
    pub const TRAN: u32 = 4;
    pub const AC: u32 = 5;
    pub const NOISE: u32 = 6;
    pub const START: u32 = 7;
    pub const SETTLE: u32 = 8;
    /// The host's session parameter called `Pin.name`.
    pub const NAMED: u32 = 9;
}

/// `Pin.flags` bits.
pub mod pin_flag {
    pub const STRUCTURAL: u32 = 1 << 0;
    pub const OPTIONAL: u32 = 1 << 1;
    pub const HAS_INIT: u32 = 1 << 2;
    pub const HAS_CHECK: u32 = 1 << 3;
    pub const QUIET: u32 = 1 << 4;
    pub const RECORD: u32 = 1 << 5;
    pub const RELAX: u32 = 1 << 6;
}

/// `Field.leaf`.
pub mod leaf {
    pub const BOOL: u32 = 1;
    pub const U8: u32 = 2;
    pub const U16: u32 = 3;
    pub const U32: u32 = 4;
    pub const U64: u32 = 5;
    pub const I8: u32 = 6;
    pub const I16: u32 = 7;
    pub const I32: u32 = 8;
    pub const I64: u32 = 9;
    pub const F32: u32 = 10;
    pub const F64: u32 = 11;
}

/// `Field.flags` bits.
pub mod field_flag {
    pub const HAS_KNOWN: u32 = 1 << 0;
}

/// `Term.op`.
pub mod op {
    pub const FLOW: u32 = 1;
    pub const DDT: u32 = 2;
    pub const DELAY: u32 = 3;
}

/// `Input.kind` (held inputs).
pub mod input {
    pub const LIMIT: u32 = 1;
    pub const NOISE: u32 = 2;
    pub const STIM: u32 = 3;
}

/// `Watch.kind`.
pub mod watch {
    pub const VAR: u32 = 1;
    pub const CROSS: u32 = 2;
    pub const CAP: u32 = 3;
    pub const TRACE: u32 = 4;
}

/// `Cell.kind`.
pub mod cell {
    pub const PIN: u32 = 1;
    pub const TERM: u32 = 2;
    pub const INPUT: u32 = 3;
    pub const WATCH: u32 = 4;
    pub const AMBIENT: u32 = 5;
}

/// `Cell.part` (abi.md §4.1).
pub mod part {
    pub const VALUE: u32 = 1;
    pub const PREVIOUS: u32 = 2;
    pub const CHANGED: u32 = 3;
    pub const TIME: u32 = 4;
    pub const LAST: u32 = 5;
    pub const AGE: u32 = 6;
    pub const PROPOSED: u32 = 7;
    pub const LIMITED: u32 = 8;
    pub const PSD: u32 = 9;
    pub const RE: u32 = 10;
    pub const IM: u32 = 11;
}

/// `Cell.index` of an `AMBIENT` cell (abi.md §4.2).
pub mod ambient {
    pub const TIME: u32 = 1;
    pub const FREQ: u32 = 2;
    pub const STATIC: u32 = 3;
    pub const GMIN: u32 = 4;
    pub const SOURCE_SCALE: u32 = 5;
    pub const TICK: u32 = 6;
    pub const HORIZON: u32 = 7;
}

/// `Function.flags` bits.
pub mod fn_flag {
    pub const OWNS_CORRECTION: u32 = 1 << 0;
}

/// `Reaction.flags` bits.
pub mod reaction_flag {
    pub const IRREVERSIBLE: u32 = 1 << 0;
}

/// `Statement.flags` bits.
pub mod statement_flag {
    pub const LATER: u32 = 1 << 0;
    pub const REPLACE: u32 = 1 << 1;
}

/// The status a call returns (abi.md §1, concept 13).
pub mod status {
    pub const OK: u32 = 1;
    pub const FAULT: u32 = 2;
    pub const RESHAPE: u32 = 3;
    pub const FINISH: u32 = 4;
}


// ─── Typed access ───────────────────────────────────────────────────────────

/// A type every byte pattern of whose size is a valid value, with no padding:
/// what typed access to a cell may return (abi.md §12 "Typed access in the
/// lens requires `Pod`").
///
/// # Safety
///
/// The implementor has no padding, no invalid bit pattern and no pointer;
/// it is `Copy` and its alignment divides its size.
pub unsafe trait Pod: Copy + 'static {}

// SAFETY: abi.md §12 "Typed access in the lens requires Pod": every bit
// pattern of a numeric leaf is a value and none has padding.
unsafe impl Pod for u8 {}
// SAFETY: as for `u8`.
unsafe impl Pod for u16 {}
// SAFETY: as for `u8`.
unsafe impl Pod for u32 {}
// SAFETY: as for `u8`.
unsafe impl Pod for u64 {}
// SAFETY: as for `u8`.
unsafe impl Pod for i8 {}
// SAFETY: as for `u8`.
unsafe impl Pod for i16 {}
// SAFETY: as for `u8`.
unsafe impl Pod for i32 {}
// SAFETY: as for `u8`.
unsafe impl Pod for i64 {}
// SAFETY: as for `u8`.
unsafe impl Pod for f32 {}
// SAFETY: as for `u8`.
unsafe impl Pod for f64 {}
// SAFETY: an array of `Pod` has no padding between its elements.
unsafe impl<T: Pod, const N: usize> Pod for [T; N] {}

/// `bytes` as a `T`: `None` when they are too few or misaligned for `T`.
pub fn pod_ref<T: Pod>(bytes: &[u8]) -> Option<&T> {
    if bytes.len() < size_of::<T>() || bytes.as_ptr().align_offset(align_of::<T>()) != 0 {
        return None;
    }
    // SAFETY: abi.md §12 "Typed access in the lens requires Pod": `T` is
    // `Pod`, and the bytes are enough and aligned (checked above).
    Some(unsafe { &*bytes.as_ptr().cast::<T>() })
}

/// `bytes` as a mutable `T`: `None` when they are too few or misaligned.
pub fn pod_mut<T: Pod>(bytes: &mut [u8]) -> Option<&mut T> {
    if bytes.len() < size_of::<T>() || bytes.as_ptr().align_offset(align_of::<T>()) != 0 {
        return None;
    }
    // SAFETY: as for `pod_ref`; the borrow is unique because `bytes` is.
    Some(unsafe { &mut *bytes.as_mut_ptr().cast::<T>() })
}

/// The bytes of a `Pod` value.
pub fn pod_bytes<T: Pod>(value: &T) -> &[u8] {
    // SAFETY: abi.md §12: a `Pod` has no padding, so all `size_of::<T>()`
    // bytes behind `value` are initialized.
    unsafe { core::slice::from_raw_parts((value as *const T).cast::<u8>(), size_of::<T>()) }
}
