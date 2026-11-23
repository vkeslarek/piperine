//! The solver side of a call (abi.md §2, §3, §7 "One path"): a `Handle` owns
//! one instance of a `Module` and calls it through the module's pointers,
//! building the frames of abi.md §3 from checked slices. Both sub-solvers call
//! every device through it, whatever produced the `Module`.

use std::any::Any;
use std::ffi::c_void;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

use crate::device::{Access, Flow, cell_lens};
use crate::raw::{self, Host, Instance, List, Module, NOTE_TEXT, Note, Shape, Value, status};

/// A call that did not return `OK` (or `FINISH` from a reaction).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallError {
    /// `FAULT` or `RESHAPE` (or the unknown status a device returned).
    pub status: u32,
    /// The note's code.
    pub code: u32,
    /// The note's text.
    pub message: String,
}

impl fmt::Display for CallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let what = match self.status {
            status::FAULT => "fault",
            status::RESHAPE => "reshape",
            _ => "unknown status",
        };
        write!(f, "{what} (status {}, code {}): {}", self.status, self.code, self.message)
    }
}

impl std::error::Error for CallError {}

impl CallError {
    fn from_note(s: u32, note: &Note) -> CallError {
        let len = (note.len as usize).min(NOTE_TEXT);
        CallError { status: s, code: note.code, message: String::from_utf8_lossy(&note.text[..len]).into_owned() }
    }

    fn frame(message: String) -> CallError {
        CallError { status: status::FAULT, code: 0, message }
    }
}

/// One commit a reaction made, as the solver receives it (abi.md §6).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Commit<'a> {
    pub statement: u32,
    pub offset: u32,
    pub bytes: &'a [u8],
    pub after: f64,
}

fn empty_note() -> Note {
    Note { code: 0, len: 0, text: [0; NOTE_TEXT] }
}

/// One live instance of a module.
///
/// `Handle` is `Send`: calls on different instances may run on different
/// threads (abi.md §12). No two calls on one instance may overlap (abi.md §12
/// "Threads"): [`share`](Handle::share)d handles of one instance take a lock
/// for the length of each call, so they are `Sync` too.
#[derive(Debug)]
pub struct Handle {
    inner: std::sync::Arc<Inner>,
}

/// The instance behind every share of one handle.
#[derive(Debug)]
struct Inner {
    module: &'static Module,
    instance: *mut Instance,
    shape: *const Shape<'static>,
    /// The cell lengths of each function, computed once at `create`.
    frames: Vec<FunctionFrame>,
    /// Calls on one instance never overlap (abi.md §12 "Threads"): shares of a
    /// handle take this for the length of each `eval` and `react`. It holds the pointer lists
    /// an `eval` builds, so a call after the first allocates nothing.
    calls: std::sync::Mutex<EvalBuffers>,
}

/// The byte length of every cell a function reads and writes, from the shape, computed once.
/// `None` when a cell breaks a rule `validate` checks; the first `eval` of the function
/// reports it, as it always has.
#[derive(Debug)]
struct FunctionFrame {
    cells: Option<(Box<[usize]>, Box<[usize]>)>,
}

impl FunctionFrame {
    /// The frame of function `f` of `shape`.
    fn of(shape: &Shape<'_>, f: &raw::Function<'_>) -> Self {
        let reads = cell_lens(shape, &f.reads, Access::FunctionRead);
        let writes = cell_lens(shape, &f.writes, Access::FunctionWrite);
        Self { cells: reads.zip(writes).map(|(reads, writes)| (reads.into(), writes.into())) }
    }
}

/// The pointer lists of one `eval` frame, kept between calls so that building them allocates
/// only while they grow.
#[derive(Debug, Default)]
struct EvalBuffers {
    input: Vec<*const u8>,
    output: Vec<*mut u8>,
}

// SAFETY: abi.md §12 "Calls on different instances may run on different
// threads": the instance is only called while `calls` is held, so no two calls
// on it overlap whichever share or thread makes them.
unsafe impl Send for Inner {}
// SAFETY: as for `Send`; `calls` serializes every call through a shared reference.
unsafe impl Sync for Inner {}

/// The data of the `Host` a `react` call passes: the solver's closure, and a
/// panic it raised, resumed once the device has returned.
struct Trampoline<'f> {
    commit: &'f mut dyn FnMut(Commit<'_>) -> u32,
    panic: Option<Box<dyn Any + Send>>,
}

impl Trampoline<'_> {
    /// Hands one commit to the solver's closure: a panic it raises is kept (resumed once the device returned) and
    /// the device is answered `FAULT` (abi.md §12).
    ///
    /// # Safety
    ///
    /// `data` is the `Trampoline` of the live `react` call and `bytes` holds `len` bytes.
    unsafe fn deliver(data: *mut c_void, statement: u32, offset: u32, len: u32, bytes: *const u8, after: f64) -> u32 {
        // SAFETY: abi.md §3 React: `host_data` is what `Handle::react` passed, a
        // live `Trampoline` for this call.
        let t = unsafe { &mut *data.cast::<Trampoline<'_>>() };
        if t.panic.is_some() {
            return status::FAULT;
        }
        let bytes: &[u8] = if len == 0 {
            &[]
        } else {
            // SAFETY: abi.md §2 entry 6: `bytes` holds `len` bytes for this call.
            unsafe { core::slice::from_raw_parts(bytes, len as usize) }
        };
        let commit = &mut t.commit;
        match catch_unwind(AssertUnwindSafe(|| commit(Commit { statement, offset, bytes, after }))) {
            Ok(s) => s,
            Err(payload) => {
                t.panic = Some(payload);
                status::FAULT
            }
        }
    }
}

/// `Host.commit`.
unsafe extern "C" fn trampoline(data: *mut c_void, statement: u32, offset: u32, len: u32, bytes: *const u8, after: f64) -> u32 {
    // SAFETY: as `deliver` requires; the device passes what `react` handed it (abi.md §2 entry 6).
    unsafe { Trampoline::deliver(data, statement, offset, len, bytes, after) }
}

impl Handle {
    /// Creates an instance with initial values `(PARAM pin, bytes)`.
    pub fn create(module: &'static Module, values: &[(u32, &[u8])]) -> Result<Handle, CallError> {
        let values: Vec<Value> = values
            .iter()
            .map(|(pin, b)| Value { pin: *pin, bytes: raw::Bytes::from_slice(b) })
            .collect();
        let mut note = empty_note();
        // SAFETY: abi.md §2 entry 2: `module` is a live module; `values` and
        // `note` live for the call.
        let made = unsafe { (module.create)(module, List::from_slice(&values), &mut note) };
        if made.status != status::OK || made.instance.is_null() || made.shape.is_null() {
            return Err(CallError::from_note(made.status, &note));
        }
        // SAFETY: abi.md §9: the shape lives until the last instance created with it is destroyed.
        let shape: &Shape<'_> = unsafe { &*made.shape };
        let frames = shape.functions.iter().map(|f| FunctionFrame::of(shape, f)).collect();
        Ok(Handle {
            inner: std::sync::Arc::new(Inner {
                module,
                instance: made.instance,
                shape: made.shape,
                frames,
                calls: std::sync::Mutex::new(EvalBuffers::default()),
            }),
        })
    }

    /// Another handle on the same instance (one `create`, several callers: the
    /// two sub-solvers of one placement). The instance is destroyed when the
    /// last share is dropped; calls through any share are serialized.
    pub fn share(&self) -> Handle {
        Handle { inner: std::sync::Arc::clone(&self.inner) }
    }

    /// The module this instance came from.
    pub fn module(&self) -> &'static Module {
        self.inner.module
    }

    /// The instance's shape.
    pub fn shape(&self) -> &Shape<'_> {
        // SAFETY: abi.md §9 "Any other shape lives until the last instance
        // created with it is destroyed": the instance lives as long as `self`.
        unsafe { &*self.inner.shape }
    }

    /// Runs function `k`. `input` holds the bytes of each read cell, `output`
    /// a buffer per written cell pre-filled with its current bytes, `seed` and
    /// `jac` one entry per declared partial (`jac` pre-zeroed). Each buffer is
    /// at least its cell's length (`device::cell_lens`).
    pub fn eval(
        &self,
        k: u32,
        input: &[&[u8]],
        output: &mut [&mut [u8]],
        seed: &[u8],
        jac: &mut [f64],
    ) -> Result<(), CallError> {
        let (reads, writes) = self.cells(k)?;
        check_cells("read", input.iter().map(|b| b.len()), reads)?;
        check_cells("write", output.iter().map(|b| b.len()), writes)?;
        let mut buffers = self.inner.calls.lock().unwrap_or_else(|e| e.into_inner());
        buffers.input.clear();
        buffers.input.extend(input.iter().map(|b| b.as_ptr()));
        buffers.output.clear();
        buffers.output.extend(output.iter_mut().map(|b| b.as_mut_ptr()));
        self.invoke(k, &buffers, seed, jac)
    }

    /// [`eval`](Self::eval) over cells kept the way a caller that calls the same function every
    /// iteration keeps them: `input` is the bytes of every read cell one after the other, each
    /// as long as its cell, and `output` is one buffer per written cell. Nothing is allocated
    /// once the handle has made a call.
    pub fn eval_packed(
        &self,
        k: u32,
        input: &[u8],
        output: &mut [Vec<u8>],
        seed: &[u8],
        jac: &mut [f64],
    ) -> Result<(), CallError> {
        let (reads, writes) = self.cells(k)?;
        let wanted: usize = reads.iter().sum();
        if input.len() < wanted {
            return Err(CallError::frame(format!("{} input bytes for {wanted} bytes of read cells", input.len())));
        }
        check_cells("write", output.iter().map(Vec::len), writes)?;
        let mut buffers = self.inner.calls.lock().unwrap_or_else(|e| e.into_inner());
        buffers.input.clear();
        let mut at = 0;
        for len in reads {
            buffers.input.push(input[at..].as_ptr());
            at += len;
        }
        buffers.output.clear();
        buffers.output.extend(output.iter_mut().map(|b| b.as_mut_ptr()));
        self.invoke(k, &buffers, seed, jac)
    }

    /// The lengths of the cells function `k` reads and writes.
    fn cells(&self, k: u32) -> Result<(&[usize], &[usize]), CallError> {
        let Some(frame) = self.inner.frames.get(k as usize) else {
            return Err(CallError::frame(format!("function {k} does not exist")));
        };
        match &frame.cells {
            Some((reads, writes)) => Ok((reads, writes)),
            None => Err(CallError::frame(format!("function {k} names a cell its shape does not have"))),
        }
    }

    /// Calls function `k` with the pointers in `buffers`, whose cells the caller checked.
    fn invoke(&self, k: u32, buffers: &EvalBuffers, seed: &[u8], jac: &mut [f64]) -> Result<(), CallError> {
        let n = self.shape().functions.get(k as usize).map_or(0, |f| f.partials.len());
        if seed.len() != n || jac.len() != n {
            return Err(CallError::frame(format!(
                "function {k} declares {n} partials; seed has {}, jac {}",
                seed.len(),
                jac.len()
            )));
        }
        let mut note = empty_note();
        let frame = raw::Eval {
            size: size_of::<raw::Eval>() as u32,
            input: buffers.input.as_ptr(),
            output: buffers.output.as_ptr(),
            seed: seed.as_ptr(),
            jac: jac.as_mut_ptr(),
            note: &mut note,
        };
        // SAFETY: abi.md §3 Eval "Every pointer is non-null and has the length
        // the shape says": the caller checked the cells against the function's, and the
        // buffers are locked for the call.
        match unsafe { (self.inner.module.eval)(self.inner.instance, k, &frame) } {
            status::OK => Ok(()),
            s => Err(CallError::from_note(s, &note)),
        }
    }

    /// Runs reaction `k`. `input` holds the bytes of each read cell; `commit`
    /// receives each commit the reaction makes and returns its status.
    pub fn react(
        &mut self,
        k: u32,
        input: &[&[u8]],
        commit: &mut dyn FnMut(Commit<'_>) -> u32,
    ) -> Result<Flow, CallError> {
        let shape = self.shape();
        let Some(r) = shape.reactions.get(k as usize) else {
            return Err(CallError::frame(format!("reaction {k} does not exist")));
        };
        let Some(reads) = cell_lens(shape, &r.reads, Access::ReactionRead) else {
            return Err(CallError::frame(format!("reaction {k} names a cell its shape does not have")));
        };
        check_cells("read", input.iter().map(|b| b.len()), &reads)?;
        let input: Vec<*const u8> = input.iter().map(|b| b.as_ptr()).collect();
        let host = Host { size: size_of::<Host>() as u32, commit: trampoline };
        let mut data = Trampoline { commit, panic: None };
        let mut note = empty_note();
        let frame = raw::React {
            size: size_of::<raw::React>() as u32,
            input: input.as_ptr(),
            host: &host,
            host_data: (&mut data as *mut Trampoline<'_>).cast::<c_void>(),
            note: &mut note,
        };
        // SAFETY: abi.md §3 React: `input` matches the reaction's reads
        // (checked above); `host` and `data` outlive the call.
        let s = {
            let _calls = self.inner.calls.lock().unwrap_or_else(|e| e.into_inner());
            unsafe { (self.inner.module.react)(self.inner.instance, k, &frame) }
        };
        if let Some(payload) = data.panic.take() {
            resume_unwind(payload);
        }
        match s {
            status::OK => Ok(Flow::Continue),
            status::FINISH => Ok(Flow::Finish),
            _ => Err(CallError::from_note(s, &note)),
        }
    }
}

/// Checks a frame's buffers against the cells' lengths.
fn check_cells(what: &str, got: impl ExactSizeIterator<Item = usize>, want: &[usize]) -> Result<(), CallError> {
    if got.len() != want.len() {
        return Err(CallError::frame(format!("{} {what} buffers for {} {what} cells", got.len(), want.len())));
    }
    for (i, (g, w)) in got.zip(want).enumerate() {
        if g < *w {
            return Err(CallError::frame(format!("{what} {i} has {g} bytes, its cell {w}")));
        }
    }
    Ok(())
}

impl Drop for Inner {
    fn drop(&mut self) {
        // SAFETY: abi.md §2 entry 3: the instance came from this module's
        // `create` and is destroyed once, here.
        unsafe { (self.module.destroy)(self.instance) }
    }
}
