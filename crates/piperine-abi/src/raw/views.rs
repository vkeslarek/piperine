//! The views: strings, bytes and lists a producer owns, and the one typed view (`Pod`) of a cell's bytes.

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

// ─── Typed access ───────────────────────────────────────────────────────────

/// A type every byte pattern of whose size is a valid value, with no padding:
/// what typed access to a cell may return (abi.md §12 "Typed access in the
/// lens requires `Pod`").
///
/// # Safety
///
/// The implementor has no padding, no invalid bit pattern and no pointer;
/// it is `Copy` and its alignment divides its size.
pub unsafe trait Pod: Copy + 'static {
    /// `bytes` as a `Self`: `None` when they are too few or misaligned for `Self`.
    fn from_bytes(bytes: &[u8]) -> Option<&Self> {
        if bytes.len() < size_of::<Self>() || bytes.as_ptr().align_offset(align_of::<Self>()) != 0 {
            return None;
        }
        // SAFETY: abi.md §12 "Typed access in the lens requires Pod": `Self` is
        // `Pod`, and the bytes are enough and aligned (checked above).
        Some(unsafe { &*bytes.as_ptr().cast::<Self>() })
    }

    /// `bytes` as a mutable `Self`: `None` when they are too few or misaligned.
    fn from_bytes_mut(bytes: &mut [u8]) -> Option<&mut Self> {
        if bytes.len() < size_of::<Self>() || bytes.as_ptr().align_offset(align_of::<Self>()) != 0 {
            return None;
        }
        // SAFETY: as for `from_bytes`; the borrow is unique because `bytes` is.
        Some(unsafe { &mut *bytes.as_mut_ptr().cast::<Self>() })
    }

    /// The bytes of a `Pod` value.
    fn as_bytes(&self) -> &[u8] {
        // SAFETY: abi.md §12: a `Pod` has no padding, so all `size_of::<Self>()`
        // bytes behind `self` are initialized.
        unsafe { core::slice::from_raw_parts((self as *const Self).cast::<u8>(), size_of::<Self>()) }
    }
}

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
