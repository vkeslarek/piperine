//! The C contract of abi.md §3: every struct `#[repr(C)]`
//! with its fields in the listed order, every enumeration a group of named
//! `u32` constants. Values start at 1 (flags at bit 0), so an all-zero entry
//! never carries a valid kind. Beside it, the one typed view of a cell's
//! bytes (`Pod`, `pod_ref`, `pod_mut`, `pod_bytes`) the lenses use.

use core::ffi::c_void;
use core::marker::PhantomData;
use core::mem::{align_of, size_of};

// ─── Strings, bytes, lists ──────────────────────────────────────────────────

// The three views below and `Module` keep their pointer fields private, and
// each view carries the lifetime `'a` of what it points to (a zero-sized
// `PhantomData`, so the C layout is unchanged): a view built in safe code
// (`from_static`, `from_slice`, `empty`, a `ShapeBuf`, `module::<D>`) borrows
// its memory for `'a` and cannot outlive it; one built from foreign memory
// goes through an `unsafe` constructor whose caller promises `'a`, or an
// `unsafe` dereference of the producer's pointer. That is the trust boundary,
// so the safe accessors that read through them are sound.

/// UTF-8 text owned by the producer.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Str<'a> {
    pub(crate) ptr: *const u8,
    pub(crate) len: usize,
    pub(crate) life: PhantomData<&'a [u8]>,
}

/// Raw bytes owned by the producer.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Bytes<'a> {
    pub(crate) ptr: *const u8,
    pub(crate) len: usize,
    pub(crate) life: PhantomData<&'a [u8]>,
}

/// Bytes the consumer owns and the producer writes: the output buffer of a call. A view built in safe code
/// (`from_slice`) borrows its memory exclusively for `'a`.
#[repr(C)]
#[derive(Debug)]
pub struct BytesMut<'a> {
    pub(crate) ptr: *mut u8,
    pub(crate) len: usize,
    pub(crate) life: PhantomData<&'a mut [u8]>,
}

impl<'a> BytesMut<'a> {
    /// A `BytesMut` borrowing `b` exclusively for `'a`.
    pub fn from_slice(b: &'a mut [u8]) -> BytesMut<'a> {
        BytesMut { ptr: b.as_mut_ptr(), len: b.len(), life: PhantomData }
    }

    /// The length in bytes.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether there are no bytes.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The bytes, to write.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        if self.len == 0 {
            return &mut [];
        }
        // SAFETY: a `BytesMut` points to `len` writable bytes its consumer lent exclusively for the call
        // (built by `from_slice`, which borrows them for `'a`).
        unsafe { core::slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

/// A list whose elements lie `stride` bytes apart: the element size as the
/// producer compiled it. A consumer reads the fields it knows (abi.md §3, §12
/// "Versioning").
#[repr(C)]
#[derive(Debug)]
pub struct List<'a, T> {
    pub(crate) ptr: *const T,
    pub(crate) len: usize,
    pub(crate) stride: usize,
    pub(crate) life: PhantomData<&'a T>,
}

impl<T> Clone for List<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for List<'_, T> {}

// SAFETY: abi.md §3 "The complete, immutable description": the text a `Str`
// points to is never written after it is produced.
unsafe impl Send for Str<'_> {}
// SAFETY: as for `Send` above; shared reads of immutable bytes do not race.
unsafe impl Sync for Str<'_> {}
// SAFETY: abi.md §3: the bytes of a `Value` or `init` are immutable once produced.
unsafe impl Send for Bytes<'_> {}
// SAFETY: as for `Send` above.
unsafe impl Sync for Bytes<'_> {}
// SAFETY: abi.md §3 "immutable description": a list's elements are never
// written after it is produced, so sharing it is sharing `&T`.
unsafe impl<T: Sync> Send for List<'_, T> {}
// SAFETY: as for `Send` above.
unsafe impl<T: Sync> Sync for List<'_, T> {}

impl Str<'static> {
    /// The empty string, with a non-null pointer.
    pub const EMPTY: Str<'static> =
        Str { ptr: core::ptr::NonNull::<u8>::dangling().as_ptr(), len: 0, life: PhantomData };

    /// A `Str` over a static string.
    pub const fn from_static(s: &'static str) -> Str<'static> {
        Str { ptr: s.as_ptr(), len: s.len(), life: PhantomData }
    }
}

impl<'a> Str<'a> {
    /// A `Str` borrowing `s` for `'a`.
    pub const fn from_str(s: &'a str) -> Str<'a> {
        Str { ptr: s.as_ptr(), len: s.len(), life: PhantomData }
    }

    /// A `Str` over foreign memory.
    ///
    /// # Safety
    ///
    /// `ptr` is non-null and points to `len` bytes of UTF-8 that stay valid
    /// and unchanged as long as the `Str` (or any copy) is read (abi.md §12).
    pub const unsafe fn from_raw_parts(ptr: *const u8, len: usize) -> Str<'a> {
        Str { ptr, len, life: PhantomData }
    }

    /// The pointer.
    pub fn as_ptr(&self) -> *const u8 {
        self.ptr
    }

    /// The length in bytes.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the string is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The bytes of the string.
    pub fn as_bytes(&self) -> &'a [u8] {
        if self.len == 0 {
            return &[];
        }
        // SAFETY: abi.md §12 "Ownership": a `Str` of a shape, library or frame
        // points to `len` readable bytes for as long as its owner lives.
        unsafe { core::slice::from_raw_parts(self.ptr, self.len) }
    }

    /// The text, or `"<invalid utf-8>"` when the producer broke abi.md §3 ("UTF-8").
    pub fn as_str(&self) -> &'a str {
        core::str::from_utf8(self.as_bytes()).unwrap_or("<invalid utf-8>")
    }
}

impl Bytes<'static> {
    /// No bytes, with a non-null pointer.
    pub const EMPTY: Bytes<'static> =
        Bytes { ptr: core::ptr::NonNull::<u8>::dangling().as_ptr(), len: 0, life: PhantomData };

    /// A `Bytes` over a static slice.
    pub const fn from_static(b: &'static [u8]) -> Bytes<'static> {
        Bytes { ptr: b.as_ptr(), len: b.len(), life: PhantomData }
    }
}

impl<'a> Bytes<'a> {
    /// A `Bytes` borrowing `b` for `'a`.
    pub const fn from_slice(b: &'a [u8]) -> Bytes<'a> {
        Bytes { ptr: b.as_ptr(), len: b.len(), life: PhantomData }
    }

    /// A `Bytes` over foreign memory.
    ///
    /// # Safety
    ///
    /// `ptr` is non-null and points to `len` bytes that stay valid and
    /// unchanged as long as the `Bytes` (or any copy) is read (abi.md §12).
    pub const unsafe fn from_raw_parts(ptr: *const u8, len: usize) -> Bytes<'a> {
        Bytes { ptr, len, life: PhantomData }
    }

    /// The pointer.
    pub fn as_ptr(&self) -> *const u8 {
        self.ptr
    }

    /// The length in bytes.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether there are no bytes.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The bytes.
    pub fn as_slice(&self) -> &'a [u8] {
        if self.len == 0 {
            return &[];
        }
        // SAFETY: abi.md §12 "Ownership": a `Bytes` points to `len` readable
        // bytes for as long as its owner lives.
        unsafe { core::slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl<'a, T> List<'a, T> {
    /// The empty list: a non-null, aligned pointer and no element.
    pub const fn empty() -> List<'a, T> {
        List { ptr: core::ptr::NonNull::<T>::dangling().as_ptr(), len: 0, stride: size_of::<T>(), life: PhantomData }
    }

    /// A list over foreign memory, `stride` bytes apart.
    ///
    /// # Safety
    ///
    /// `ptr` is non-null and `len` elements of at least `stride` bytes each
    /// lie at `ptr + i * stride`, valid and unchanged as long as the list (or
    /// any copy) is read (abi.md §3, §12). A stride below `size_of::<T>()` is
    /// allowed here and refused by `is_readable`.
    pub const unsafe fn from_raw_parts(ptr: *const T, len: usize, stride: usize) -> List<'a, T> {
        List { ptr, len, stride, life: PhantomData }
    }

    /// The pointer.
    pub fn as_ptr(&self) -> *const T {
        self.ptr
    }

    /// The producer's element size.
    pub fn stride(&self) -> usize {
        self.stride
    }

    /// A list borrowing `items` for `'a`, with this compilation's stride. It
    /// cannot outlive the slice:
    ///
    /// ```compile_fail
    /// use piperine_abi::raw::List;
    /// let list: List<u64> = {
    ///     let v = vec![1u64; 4];
    ///     List::from_slice(&v) // `v` does not live long enough
    /// };
    /// let _ = list.get(0);
    /// ```
    pub const fn from_slice(items: &'a [T]) -> List<'a, T> {
        if items.is_empty() {
            return List::empty();
        }
        List { ptr: items.as_ptr(), len: items.len(), stride: size_of::<T>(), life: PhantomData }
    }

    /// A list over storage the crate keeps alive by other means (a `ShapeBuf`
    /// owns its boxes until its last `ShapeRef` is dropped); the `'static` is
    /// that owner's promise, never handed out but as a borrow of it.
    pub(crate) fn detached(items: &[T]) -> List<'static, T> {
        if items.is_empty() {
            return List::empty();
        }
        List { ptr: items.as_ptr(), len: items.len(), stride: size_of::<T>(), life: PhantomData }
    }

    /// Whether this consumer can read the elements: the stride holds at least
    /// a `T` and keeps its alignment, and the pointer is aligned. A producer of
    /// an older minor (abi.md §12 "Versioning") may compile a smaller element;
    /// such a list is refused, never read past its elements.
    pub fn is_readable(&self) -> bool {
        self.stride >= size_of::<T>()
            && self.stride.is_multiple_of(align_of::<T>())
            && (self.ptr as usize).is_multiple_of(align_of::<T>())
    }

    /// Element `i`, read at `ptr + i * stride`; `None` past the end or when the
    /// list is not readable (`is_readable`).
    pub fn get(&self, i: usize) -> Option<&'a T> {
        if i >= self.len || !self.is_readable() {
            return None;
        }
        // SAFETY: abi.md §3 "stride = element size as the producer compiled it"
        // and §12 "Ownership": `len` elements lie `stride` bytes apart and live
        // as long as the list's owner; `is_readable` checked that each holds a
        // whole aligned `T`.
        unsafe { Some(&*self.ptr.cast::<u8>().add(i * self.stride).cast::<T>()) }
    }

    /// Every element in order, each read by `stride`. A list that is not
    /// readable yields nothing: check `is_readable` (or `validate` the shape)
    /// before trusting an empty iteration of foreign memory.
    pub fn iter(&self) -> impl Iterator<Item = &'a T> + use<'a, T> {
        let list = *self;
        (0..list.len).filter_map(move |i| list.get(i))
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
    pub name: Str<'static>,
    pub build: Str<'static>,
    pub modules: List<'static, Module>,
}

/// `create` of abi.md §2.
pub type CreateFn = for<'a> unsafe extern "C" fn(*const Module, List<'a, Value<'a>>, *mut Note) -> Created;
/// `destroy` of abi.md §2.
pub type DestroyFn = unsafe extern "C" fn(*mut Instance);
/// `eval` of abi.md §2.
pub type EvalFn = unsafe extern "C" fn(*mut Instance, u32, *const Eval) -> u32;
/// `react` of abi.md §2.
pub type ReactFn = unsafe extern "C" fn(*mut Instance, u32, *const React) -> u32;
/// `commit` of abi.md §2, the solver's entry.
pub type CommitFn =
    unsafe extern "C" fn(*mut c_void, statement: u32, offset: u32, len: u32, bytes: *const u8, after: f64) -> u32;
/// `commit_element`, the solver's entry for a statement on an array pin: it names the element at run time.
pub type CommitElementFn = unsafe extern "C" fn(
    *mut c_void,
    statement: u32,
    element: u64,
    offset: u32,
    len: u32,
    bytes: *const u8,
    after: f64,
) -> u32;
/// The reader of an [`ArrayView`]: the bytes of one element of the call's snapshot, for a `part::*`.
pub type ArrayReadFn = unsafe extern "C" fn(context: *const c_void, element: u64, part: u32) -> *const u8;

/// A named factory in a library.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Module {
    pub(crate) name: Str<'static>,
    /// The shape at default values: lists every `PARAM` pin, its layout, its default.
    pub(crate) prototype: *const Shape<'static>,
    /// The module's own data (an interpreter's IR, an OSDI descriptor).
    pub(crate) data: *const c_void,
    pub(crate) create: CreateFn,
    pub(crate) destroy: DestroyFn,
    pub(crate) eval: EvalFn,
    pub(crate) react: ReactFn,
}

impl Module {
    /// A module from foreign parts (a library loaded by its symbol).
    ///
    /// # Safety
    ///
    /// `prototype` points to a valid `Shape` and the four functions keep the
    /// contracts of abi.md §2 for as long as the module is used (§12: "The
    /// library and its prototypes live until the process ends").
    pub const unsafe fn from_raw_parts(
        name: Str<'static>,
        prototype: *const Shape<'static>,
        data: *const c_void,
        create: CreateFn,
        destroy: DestroyFn,
        eval: EvalFn,
        react: ReactFn,
    ) -> Module {
        Module { name, prototype, data, create, destroy, eval, react }
    }

    pub fn name(&self) -> &Str<'static> {
        &self.name
    }

    /// The prototype shape.
    pub fn prototype(&self) -> &Shape<'static> {
        // SAFETY: abi.md §12 "The library and its prototypes live until the
        // process ends"; a `Module` is built only by `module::<D>` or the
        // `unsafe` `from_raw_parts`, both of which keep `prototype` valid.
        unsafe { &*self.prototype }
    }

    /// The prototype pointer, as the C entry `create` receives it. Kept for a device plugin host that
    /// calls that entry.
    pub fn prototype_ptr(&self) -> *const Shape<'static> {
        self.prototype
    }

    pub fn data(&self) -> *const c_void {
        self.data
    }

    pub fn create(&self) -> CreateFn {
        self.create
    }

    pub fn destroy(&self) -> DestroyFn {
        self.destroy
    }

    pub fn eval(&self) -> EvalFn {
        self.eval
    }

    pub fn react(&self) -> ReactFn {
        self.react
    }
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
pub struct Value<'a> {
    pub pin: u32,
    pub bytes: Bytes<'a>,
}

/// The result of `create`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Created {
    pub status: u32,
    pub instance: *mut Instance,
    pub shape: *const Shape<'static>,
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
    /// A later minor may append fields: a host whose `size` does not reach a field does not have
    /// it.
    pub size: u32,
    pub commit: CommitFn,
    /// Appended by arrays of carriers; a host whose `size` does not reach it refuses element commits.
    pub commit_element: CommitElementFn,
}

/// What a `WHOLE` read cell of an array pin holds: the array behind the call's snapshot (R2), read one element
/// at a time, so a `1 << 32` element array is never copied into a frame (abi.md §5, §3.1).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ArrayView {
    /// Opaque to the device: handed back to `read`.
    pub context: *const c_void,
    /// The number of elements.
    pub count: u64,
    /// The bytes of one element.
    pub size: u32,
    /// The bytes of `element` for a `part::VALUE`, `PREVIOUS` or `CHANGED`.
    pub read: ArrayReadFn,
}

// ─── Constants ──────────────────────────────────────────────────────────────

/// `Function.elements`/`Reaction.elements`: a read of every element of an array pin, as an [`ArrayView`].
pub const WHOLE: u64 = u64::MAX;
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

impl ArrayView {
    /// The view stored in the bytes of a `WHOLE` read cell; `None` when they are too few.
    pub fn from_bytes(bytes: &[u8]) -> Option<ArrayView> {
        if bytes.len() < size_of::<ArrayView>() {
            return None;
        }
        // SAFETY: `ArrayView` is `repr(C)` plain data (pointers, integers); the bytes are enough (checked above)
        // and `read_unaligned` needs no alignment. The host wrote a real view into this cell (abi.md §3).
        Some(unsafe { core::ptr::read_unaligned(bytes.as_ptr().cast::<ArrayView>()) })
    }

    /// The bytes of this view, to hand as the content of a `WHOLE` read cell.
    pub fn as_bytes(&self) -> &[u8] {
        // SAFETY: a `repr(C)` struct is readable as its own `size_of` bytes for the length of the borrow.
        unsafe { core::slice::from_raw_parts((self as *const ArrayView).cast::<u8>(), size_of::<ArrayView>()) }
    }

    /// The bytes of `element` for `part` (`VALUE`, `PREVIOUS` or `CHANGED`); `None` at or past `count`.
    pub fn element(&self, element: u64, part: u32) -> Option<&[u8]> {
        if element >= self.count {
            return None;
        }
        let len = if part == self::part::CHANGED { 1 } else { self.size as usize };
        if len == 0 {
            return Some(&[]);
        }
        // SAFETY: abi.md §3.1: `read` is the host's reader for `data` and returns `len` bytes that stay valid and
        // unchanged until the call returns, which outlives this borrow of the view.
        Some(unsafe { core::slice::from_raw_parts((self.read)(self.context, element, part), len) })
    }
}

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
    /// Whether the run set its temperature, a `BIT`: `1` when it did, `0` when the `TEMPERATURE` pin holds the
    /// session's default. Appended in place (`MINOR` 0).
    pub const TEMPERATURE_GIVEN: u32 = 10;
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

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::offset_of;

    /// The comparison the layout tests share.
    struct LayoutChecks;

    impl LayoutChecks {
        /// One struct of the table: its measured size and alignment against the C
        /// layout, and each field's measured offset against the expected one.
        fn check(name: &str, size: (usize, usize), align: (usize, usize), fields: &[(&str, usize, usize)]) {
            assert_eq!(size.0, size.1, "size_of::<{name}>()");
            assert_eq!(align.0, align.1, "align_of::<{name}>()");
            for (field, got, want) in fields {
                assert_eq!(got, want, "offset of {name}.{field}");
            }
        }
    }
    
    #[test]
    fn struct_layouts_match_the_abi() {
        assert_eq!(size_of::<usize>(), 8, "the table is for a 64-bit target");
    
        LayoutChecks::check("Str", (size_of::<Str>(), 16), (align_of::<Str>(), 8), &[
            ("ptr", offset_of!(Str, ptr), 0),
            ("len", offset_of!(Str, len), 8),
        ]);
        LayoutChecks::check("Bytes", (size_of::<Bytes>(), 16), (align_of::<Bytes>(), 8), &[
            ("ptr", offset_of!(Bytes, ptr), 0),
            ("len", offset_of!(Bytes, len), 8),
        ]);
        LayoutChecks::check("List", (size_of::<List<Pin>>(), 24), (align_of::<List<Pin>>(), 8), &[
            ("ptr", offset_of!(List<Pin>, ptr), 0),
            ("len", offset_of!(List<Pin>, len), 8),
            ("stride", offset_of!(List<Pin>, stride), 16),
        ]);
        LayoutChecks::check("Library", (size_of::<Library>(), 64), (align_of::<Library>(), 8), &[
            ("size", offset_of!(Library, size), 0),
            ("minor", offset_of!(Library, minor), 4),
            ("name", offset_of!(Library, name), 8),
            ("build", offset_of!(Library, build), 24),
            ("modules", offset_of!(Library, modules), 40),
        ]);
        LayoutChecks::check("Module", (size_of::<Module>(), 64), (align_of::<Module>(), 8), &[
            ("name", offset_of!(Module, name), 0),
            ("prototype", offset_of!(Module, prototype), 16),
            ("data", offset_of!(Module, data), 24),
            ("create", offset_of!(Module, create), 32),
            ("destroy", offset_of!(Module, destroy), 40),
            ("eval", offset_of!(Module, eval), 48),
            ("react", offset_of!(Module, react), 56),
        ]);
        LayoutChecks::check("Value", (size_of::<Value>(), 24), (align_of::<Value>(), 8), &[
            ("pin", offset_of!(Value, pin), 0),
            ("bytes", offset_of!(Value, bytes), 8),
        ]);
        LayoutChecks::check("Created", (size_of::<Created>(), 24), (align_of::<Created>(), 8), &[
            ("status", offset_of!(Created, status), 0),
            ("instance", offset_of!(Created, instance), 8),
            ("shape", offset_of!(Created, shape), 16),
        ]);
        LayoutChecks::check("Instance", (size_of::<Instance>(), 0), (align_of::<Instance>(), 1), &[]);
        LayoutChecks::check("Note", (size_of::<Note>(), 256), (align_of::<Note>(), 4), &[
            ("code", offset_of!(Note, code), 0),
            ("len", offset_of!(Note, len), 4),
            ("text", offset_of!(Note, text), 8),
        ]);
        LayoutChecks::check("Shape", (size_of::<Shape>(), 152), (align_of::<Shape>(), 8), &[
            ("size", offset_of!(Shape, size), 0),
            ("pins", offset_of!(Shape, pins), 8),
            ("terms", offset_of!(Shape, terms), 32),
            ("inputs", offset_of!(Shape, inputs), 56),
            ("watches", offset_of!(Shape, watches), 80),
            ("functions", offset_of!(Shape, functions), 104),
            ("reactions", offset_of!(Shape, reactions), 128),
        ]);
        LayoutChecks::check("Pin", (size_of::<Pin>(), 144), (align_of::<Pin>(), 8), &[
            ("name", offset_of!(Pin, name), 0),
            ("origin", offset_of!(Pin, origin), 16),
            ("kind", offset_of!(Pin, kind), 32),
            ("role", offset_of!(Pin, role), 36),
            ("global", offset_of!(Pin, global), 40),
            ("flags", offset_of!(Pin, flags), 44),
            ("layout", offset_of!(Pin, layout), 48),
            ("unit", offset_of!(Pin, unit), 80),
            ("init", offset_of!(Pin, init), 96),
            ("check", offset_of!(Pin, check), 112),
            ("abstol", offset_of!(Pin, abstol), 120),
            ("count", offset_of!(Pin, count), 136),
        ]);
        LayoutChecks::check("Layout", (size_of::<Layout>(), 32), (align_of::<Layout>(), 8), &[
            ("size", offset_of!(Layout, size), 0),
            ("align", offset_of!(Layout, align), 4),
            ("fields", offset_of!(Layout, fields), 8),
        ]);
        LayoutChecks::check("Field", (size_of::<Field>(), 32), (align_of::<Field>(), 8), &[
            ("name", offset_of!(Field, name), 0),
            ("leaf", offset_of!(Field, leaf), 16),
            ("offset", offset_of!(Field, offset), 20),
            ("count", offset_of!(Field, count), 24),
        ]);
        LayoutChecks::check("Term", (size_of::<Term>(), 56), (align_of::<Term>(), 8), &[
            ("name", offset_of!(Term, name), 0),
            ("origin", offset_of!(Term, origin), 16),
            ("op", offset_of!(Term, op), 32),
            ("plus", offset_of!(Term, plus), 36),
            ("minus", offset_of!(Term, minus), 40),
            ("abstol", offset_of!(Term, abstol), 44),
            ("max", offset_of!(Term, max), 48),
        ]);
        LayoutChecks::check("Input", (size_of::<Input>(), 40), (align_of::<Input>(), 8), &[
            ("name", offset_of!(Input, name), 0),
            ("origin", offset_of!(Input, origin), 16),
            ("kind", offset_of!(Input, kind), 32),
        ]);
        LayoutChecks::check("Watch", (size_of::<Watch>(), 88), (align_of::<Watch>(), 8), &[
            ("name", offset_of!(Watch, name), 0),
            ("origin", offset_of!(Watch, origin), 16),
            ("kind", offset_of!(Watch, kind), 32),
            ("layout", offset_of!(Watch, layout), 40),
            ("event", offset_of!(Watch, event), 72),
            ("ttol", offset_of!(Watch, ttol), 76),
            ("etol", offset_of!(Watch, etol), 80),
        ]);
        LayoutChecks::check("Cell", (size_of::<Cell>(), 12), (align_of::<Cell>(), 4), &[
            ("kind", offset_of!(Cell, kind), 0),
            ("index", offset_of!(Cell, index), 4),
            ("part", offset_of!(Cell, part), 8),
        ]);
        LayoutChecks::check("Partial", (size_of::<Partial>(), 16), (align_of::<Partial>(), 4), &[
            ("write", offset_of!(Partial, write), 0),
            ("to", offset_of!(Partial, to), 4),
            ("read", offset_of!(Partial, read), 8),
            ("at", offset_of!(Partial, at), 12),
        ]);
        LayoutChecks::check("Function", (size_of::<Function>(), 136), (align_of::<Function>(), 8), &[
            ("name", offset_of!(Function, name), 0),
            ("origin", offset_of!(Function, origin), 16),
            ("flags", offset_of!(Function, flags), 32),
            ("reads", offset_of!(Function, reads), 40),
            ("writes", offset_of!(Function, writes), 64),
            ("partials", offset_of!(Function, partials), 88),
            ("elements", offset_of!(Function, elements), 112),
        ]);
        LayoutChecks::check("Reaction", (size_of::<Reaction>(), 136), (align_of::<Reaction>(), 8), &[
            ("name", offset_of!(Reaction, name), 0),
            ("origin", offset_of!(Reaction, origin), 16),
            ("flags", offset_of!(Reaction, flags), 32),
            ("on", offset_of!(Reaction, on), 40),
            ("reads", offset_of!(Reaction, reads), 64),
            ("writes", offset_of!(Reaction, writes), 88),
            ("elements", offset_of!(Reaction, elements), 112),
        ]);
        LayoutChecks::check("Statement", (size_of::<Statement>(), 24), (align_of::<Statement>(), 8), &[
            ("pin", offset_of!(Statement, pin), 0),
            ("offset", offset_of!(Statement, offset), 4),
            ("len", offset_of!(Statement, len), 8),
            ("flags", offset_of!(Statement, flags), 12),
            ("min_after", offset_of!(Statement, min_after), 16),
        ]);
        LayoutChecks::check("Eval", (size_of::<Eval>(), 48), (align_of::<Eval>(), 8), &[
            ("size", offset_of!(Eval, size), 0),
            ("input", offset_of!(Eval, input), 8),
            ("output", offset_of!(Eval, output), 16),
            ("seed", offset_of!(Eval, seed), 24),
            ("jac", offset_of!(Eval, jac), 32),
            ("note", offset_of!(Eval, note), 40),
        ]);
        LayoutChecks::check("React", (size_of::<React>(), 40), (align_of::<React>(), 8), &[
            ("size", offset_of!(React, size), 0),
            ("input", offset_of!(React, input), 8),
            ("host", offset_of!(React, host), 16),
            ("host_data", offset_of!(React, host_data), 24),
            ("note", offset_of!(React, note), 32),
        ]);
        LayoutChecks::check("Host", (size_of::<Host>(), 24), (align_of::<Host>(), 8), &[
            ("size", offset_of!(Host, size), 0),
            ("commit", offset_of!(Host, commit), 8),
            ("commit_element", offset_of!(Host, commit_element), 16),
        ]);
        LayoutChecks::check("ArrayView", (size_of::<ArrayView>(), 32), (align_of::<ArrayView>(), 8), &[
            ("context", offset_of!(ArrayView, context), 0),
            ("count", offset_of!(ArrayView, count), 8),
            ("size", offset_of!(ArrayView, size), 16),
            ("read", offset_of!(ArrayView, read), 24),
        ]);
    }
}
