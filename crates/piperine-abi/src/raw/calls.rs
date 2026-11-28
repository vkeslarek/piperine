//! The library, module and instance of abi.md §2 and the frames of a call (`eval`, `react`, the host).

use core::ffi::c_void;

use super::{Bytes, List, Shape, Str, part};
use core::mem::size_of;

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
    /// The module's name, the key a host looks it up by.
    pub(crate) name: Str<'static>,
    /// The shape at default values: lists every `PARAM` pin, its layout, its default.
    pub(crate) prototype: *const Shape<'static>,
    /// The module's own data (an interpreter's IR, an OSDI descriptor).
    pub(crate) data: *const c_void,
    /// The `create` entry: makes an instance of this module.
    pub(crate) create: CreateFn,
    /// The `destroy` entry: frees an instance.
    pub(crate) destroy: DestroyFn,
    /// The `eval` entry: the analog evaluation of an instance.
    pub(crate) eval: EvalFn,
    /// The `react` entry: the digital reaction of an instance.
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

    /// The module's name.
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

    /// The module's own data pointer.
    pub fn data(&self) -> *const c_void {
        self.data
    }

    /// The `create` entry.
    pub fn create(&self) -> CreateFn {
        self.create
    }

    /// The `destroy` entry.
    pub fn destroy(&self) -> DestroyFn {
        self.destroy
    }

    /// The `eval` entry.
    pub fn eval(&self) -> EvalFn {
        self.eval
    }

    /// The `react` entry.
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

impl Note {
    /// A note with no code and no text: what a caller hands a call before it runs.
    pub const EMPTY: Note = Note { code: 0, len: 0, text: [0; NOTE_TEXT] };
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
        let len = if part == part::CHANGED { 1 } else { self.size as usize };
        if len == 0 {
            return Some(&[]);
        }
        // SAFETY: abi.md §3.1: `read` is the host's reader for `data` and returns `len` bytes that stay valid and
        // unchanged until the call returns, which outlives this borrow of the view.
        Some(unsafe { core::slice::from_raw_parts((self.read)(self.context, element, part), len) })
    }
}
