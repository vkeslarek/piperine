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
/// threads (abi.md §12). It is not `Sync`: no two calls on one instance may
/// overlap (abi.md §12 "Threads"), so a handle is never shared between threads.
#[derive(Debug)]
pub struct Handle {
    module: &'static Module,
    instance: *mut Instance,
    shape: *const Shape,
}

// SAFETY: abi.md §12 "Calls on different instances may run on different
// threads": a `Handle` owns its instance, and `Handle` is not `Sync`, so
// moving it moves the only caller.
unsafe impl Send for Handle {}

/// The data of the `Host` a `react` call passes: the solver's closure, and a
/// panic it raised, resumed once the device has returned.
struct Trampoline<'f> {
    commit: &'f mut dyn FnMut(Commit<'_>) -> u32,
    panic: Option<Box<dyn Any + Send>>,
}

unsafe extern "C" fn trampoline(
    data: *mut c_void,
    statement: u32,
    offset: u32,
    len: u32,
    bytes: *const u8,
    after: f64,
) -> u32 {
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

impl Handle {
    /// Creates an instance with initial values `(PARAM pin, bytes)`.
    pub fn create(module: &'static Module, values: &[(u32, &[u8])]) -> Result<Handle, CallError> {
        let values: Vec<Value> = values
            .iter()
            .map(|(pin, b)| Value { pin: *pin, bytes: raw::Bytes { ptr: b.as_ptr(), len: b.len() } })
            .collect();
        let mut note = empty_note();
        // SAFETY: abi.md §2 entry 2: `module` is a live module; `values` and
        // `note` live for the call.
        let made = unsafe { (module.create)(module, List::from_slice(&values), &mut note) };
        if made.status != status::OK || made.instance.is_null() || made.shape.is_null() {
            return Err(CallError::from_note(made.status, &note));
        }
        Ok(Handle { module, instance: made.instance, shape: made.shape })
    }

    /// The module this instance came from.
    pub fn module(&self) -> &'static Module {
        self.module
    }

    /// The instance's shape.
    pub fn shape(&self) -> &Shape {
        // SAFETY: abi.md §9 "Any other shape lives until the last instance
        // created with it is destroyed": the instance lives as long as `self`.
        unsafe { &*self.shape }
    }

    /// Runs function `k`. `input` holds the bytes of each read cell, `output`
    /// a buffer per written cell pre-filled with its current bytes, `seed` and
    /// `jac` one entry per declared partial (`jac` pre-zeroed). Each buffer is
    /// at least its cell's length (`device::cell_lens`).
    // SPEC_DEVIATION: design `caller` gives `input: &[*const u8]`, `output: &[*mut u8]`.
    // Reason: a safe fn dereferencing caller-supplied raw pointers is unsound; slices are
    // checked against the cells' lengths and need no `unsafe` in a sub-solver.
    pub fn eval(
        &self,
        k: u32,
        input: &[&[u8]],
        output: &mut [&mut [u8]],
        seed: &[u8],
        jac: &mut [f64],
    ) -> Result<(), CallError> {
        let shape = self.shape();
        let Some(f) = shape.functions.get(k as usize) else {
            return Err(CallError::frame(format!("function {k} does not exist")));
        };
        let reads = cell_lens(shape, &f.reads, Access::FunctionRead);
        let writes = cell_lens(shape, &f.writes, Access::FunctionWrite);
        let (Some(reads), Some(writes)) = (reads, writes) else {
            return Err(CallError::frame(format!("function {k} names a cell its shape does not have")));
        };
        check_cells("read", input.iter().map(|b| b.len()), &reads)?;
        check_cells("write", output.iter().map(|b| b.len()), &writes)?;
        let n = f.partials.len();
        if seed.len() != n || jac.len() != n {
            return Err(CallError::frame(format!(
                "function {k} declares {n} partials; seed has {}, jac {}",
                seed.len(),
                jac.len()
            )));
        }
        let input: Vec<*const u8> = input.iter().map(|b| b.as_ptr()).collect();
        let output: Vec<*mut u8> = output.iter_mut().map(|b| b.as_mut_ptr()).collect();
        let mut note = empty_note();
        let frame = raw::Eval {
            size: size_of::<raw::Eval>() as u32,
            input: input.as_ptr(),
            output: output.as_ptr(),
            seed: seed.as_ptr(),
            jac: jac.as_mut_ptr(),
            note: &mut note,
        };
        // SAFETY: abi.md §3 Eval "Every pointer is non-null and has the length
        // the shape says": checked above against the function's cells.
        let s = unsafe { (self.module.eval)(self.instance, k, &frame) };
        match s {
            status::OK => Ok(()),
            _ => Err(CallError::from_note(s, &note)),
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
        let s = unsafe { (self.module.react)(self.instance, k, &frame) };
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

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: abi.md §2 entry 3: the instance came from this module's
        // `create` and is destroyed once, here.
        unsafe { (self.module.destroy)(self.instance) }
    }
}
