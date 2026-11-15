//! The generic `extern "C"` entry points over a `Device` (abi.md §2, §7):
//! `module::<D>(name)` fills a `Module` with thunks that cut the frame into
//! the lenses' slices, call the device inside `catch_unwind`, and turn its
//! result into a status and a note. No function unwinds across the boundary.

use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::device::{Device, Eval, Flow, Fault, React, Values, cell_lens};
use crate::raw::{
    self, Created, Instance, Library, List, MINOR, Module, NOTE_TEXT, Note, Shape, Str, Value, role, status,
};
use crate::shape::ShapeRef;
use crate::validate::{Access, validate};

/// An instance as the thunks see it: the device and the hold on its shape.
struct Inst<D> {
    device: D,
    shape: ShapeRef,
}

/// The `Module` of device `D`: its prototype and its four entry points.
///
/// A plain `fn`, not `const fn`: it calls `D::prototype()`.
pub fn module<D: Device>(name: &'static str) -> Module {
    Module {
        name: Str::from_static(name),
        prototype: D::prototype(),
        data: core::ptr::null(),
        create: create::<D>,
        destroy: destroy::<D>,
        eval: eval::<D>,
        react: react::<D>,
    }
}

/// The value `piperine_abi_1()` returns, over modules built once (for example
/// behind a `OnceLock`).
pub fn library(name: &'static str, build: &'static str, modules: &'static [Module]) -> Library {
    Library {
        size: size_of::<Library>() as u32,
        minor: MINOR,
        name: Str::from_static(name),
        build: Str::from_static(build),
        modules: List::from_slice(modules),
    }
}

/// Writes `fault` into `note`, the message cut to the note's capacity at a
/// character boundary.
fn write_note(note: *mut Note, fault: &Fault) {
    let text = fault.message.as_str();
    let mut cut = text.len().min(NOTE_TEXT);
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    let mut filled = Note { code: fault.code, len: cut as u32, text: [0; NOTE_TEXT] };
    filled.text[..cut].copy_from_slice(&text.as_bytes()[..cut]);
    if !note.is_null() {
        // SAFETY: abi.md §3 "Every pointer is non-null and has the length the
        // shape says": a non-null `note` is one writable `Note` for this call.
        unsafe { note.write(filled) };
    }
}

/// The message of a panic payload.
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        format!("panic: {s}")
    } else if let Some(s) = payload.downcast_ref::<String>() {
        format!("panic: {s}")
    } else {
        "panic with a non-text payload".to_string()
    }
}

/// Runs `body` with no unwinding: a panic becomes a `FAULT` with its message.
fn guarded(note: *mut Note, body: impl FnOnce() -> Result<u32, Fault>) -> u32 {
    let fault = match catch_unwind(AssertUnwindSafe(body)) {
        Ok(Ok(s)) => return s,
        Ok(Err(f)) => f,
        Err(payload) => Fault::new(0, panic_message(payload.as_ref())),
    };
    write_note(note, &fault);
    if fault.reshape { status::RESHAPE } else { status::FAULT }
}

/// `count` slices of `lens[i]` bytes from an array of pointers.
///
/// # Safety
///
/// `ptrs` points to `lens.len()` pointers, each to `lens[i]` bytes valid for `'a`.
unsafe fn cut<'a>(ptrs: *const *const u8, lens: &[usize]) -> Vec<&'a [u8]> {
    if lens.is_empty() {
        return Vec::new();
    }
    // SAFETY: abi.md §3 Eval/React: `input` is `[reads.len]` pointers, each to
    // the bytes of its read cell (the caller's contract above).
    let ptrs = unsafe { core::slice::from_raw_parts(ptrs, lens.len()) };
    ptrs.iter()
        .zip(lens)
        .map(|(p, n)| {
            if *n == 0 {
                &[][..]
            } else {
                // SAFETY: abi.md §3 Eval: "the bytes of each read cell", `n` of them.
                unsafe { core::slice::from_raw_parts(*p, *n) }
            }
        })
        .collect()
}

/// As `cut`, for the mutable output buffers.
///
/// # Safety
///
/// `ptrs` points to `lens.len()` pointers, each to `lens[i]` writable bytes
/// valid for `'a`, no two overlapping.
unsafe fn cut_mut<'a>(ptrs: *const *mut u8, lens: &[usize]) -> Vec<&'a mut [u8]> {
    if lens.is_empty() {
        return Vec::new();
    }
    // SAFETY: abi.md §3 Eval: `output` is `[writes.len]` pointers.
    let ptrs = unsafe { core::slice::from_raw_parts(ptrs, lens.len()) };
    ptrs.iter()
        .zip(lens)
        .map(|(p, n)| {
            if *n == 0 {
                &mut [][..]
            } else {
                // SAFETY: abi.md §3 Eval: "a buffer per written cell", one per
                // write, so no two alias.
                unsafe { core::slice::from_raw_parts_mut(*p, *n) }
            }
        })
        .collect()
}

unsafe extern "C" fn create<D: Device>(module: *const Module, values: List<Value>, note: *mut Note) -> Created {
    let failed = Created { status: status::FAULT, instance: core::ptr::null_mut(), shape: core::ptr::null() };
    let mut made = None;
    let s = guarded(note, || {
        // SAFETY: abi.md §12 "The library and its prototypes live until the
        // process ends": `module` and its prototype are valid.
        let proto: &Shape = unsafe { &*(*module).prototype };
        if !values.is_readable() {
            return Err(Fault::new(0, format!(
                "the value list is unreadable: stride {} for an element of {} bytes",
                values.stride(),
                size_of::<Value>()
            )));
        }
        let mut given = Vec::with_capacity(values.len());
        for v in values.iter() {
            let pin = proto.pins.get(v.pin as usize).filter(|p| p.role == role::PARAM);
            let Some(pin) = pin else {
                return Err(Fault::new(0, format!("value for pin {}, which is not a PARAM pin of the prototype", v.pin)));
            };
            let bytes = v.bytes.as_slice();
            if bytes.len() != pin.layout.size as usize {
                return Err(Fault::new(0, format!(
                    "value for pin {} ({}) has {} bytes, its layout {}",
                    v.pin,
                    pin.name.as_str(),
                    bytes.len(),
                    pin.layout.size
                )));
            }
            given.push((v.pin, bytes));
        }
        let (device, shape) = D::create(&Values::new(given))?;
        validate(shape.shape()).map_err(|e| Fault::new(0, format!("the created shape is invalid: {e}")))?;
        made = Some(Box::new(Inst { device, shape }));
        Ok(status::OK)
    });
    match made {
        Some(inst) if s == status::OK => {
            let shape = inst.shape.as_raw();
            Created { status: status::OK, instance: Box::into_raw(inst).cast::<Instance>(), shape }
        }
        _ => Created { status: s, ..failed },
    }
}

unsafe extern "C" fn destroy<D: Device>(instance: *mut Instance) {
    if instance.is_null() {
        return;
    }
    // SAFETY: abi.md §2 "destroy(instance) frees the instance": `instance`
    // came from `Box::into_raw` in `create::<D>` and is destroyed once.
    let inst = unsafe { Box::from_raw(instance.cast::<Inst<D>>()) };
    // A panicking Drop must not unwind into C; the instance is gone either way.
    let _ = catch_unwind(AssertUnwindSafe(move || drop(inst)));
}

unsafe extern "C" fn eval<D: Device>(instance: *mut Instance, k: u32, frame: *const raw::Eval) -> u32 {
    // SAFETY: abi.md §3 "Every pointer is non-null": `frame` is one `Eval`.
    let note = unsafe { (*frame).note };
    guarded(note, || {
        // SAFETY: abi.md §12 "No two calls on one instance overlap": `instance`
        // is a live `Inst<D>` from `create::<D>`, shared for this call only.
        let inst = unsafe { &*instance.cast::<Inst<D>>() };
        // SAFETY: as above, `frame` is one valid `Eval`.
        let frame = unsafe { &*frame };
        let shape = inst.shape.shape();
        let Some(f) = shape.functions.get(k as usize) else {
            return Err(Fault::new(0, format!("function {k} does not exist")));
        };
        let reads = cell_lens(shape, &f.reads, Access::FunctionRead);
        let writes = cell_lens(shape, &f.writes, Access::FunctionWrite);
        let (Some(reads), Some(writes)) = (reads, writes) else {
            return Err(Fault::new(0, format!("function {k} names a cell its shape does not have")));
        };
        let n = f.partials.len();
        // SAFETY: abi.md §3 Eval: `input`/`output` hold one pointer per read
        // and write cell, each with the cell's length (computed from the shape).
        let (input, output) = unsafe { (cut(frame.input, &reads), cut_mut(frame.output, &writes)) };
        let (seed, jac): (&[u8], &mut [f64]) = if n == 0 {
            (&[], &mut [])
        } else {
            // SAFETY: abi.md §3 Eval: `seed` and `jac` hold `[partials.len]`.
            unsafe { (core::slice::from_raw_parts(frame.seed, n), core::slice::from_raw_parts_mut(frame.jac, n)) }
        };
        let mut cx = Eval::new(input, output, seed, jac);
        inst.device.eval(k, &mut cx)?;
        Ok(status::OK)
    })
}

unsafe extern "C" fn react<D: Device>(instance: *mut Instance, k: u32, frame: *const raw::React) -> u32 {
    // SAFETY: abi.md §3 "Every pointer is non-null": `frame` is one `React`.
    let note = unsafe { (*frame).note };
    guarded(note, || {
        // SAFETY: as in `eval::<D>`.
        let inst = unsafe { &*instance.cast::<Inst<D>>() };
        // SAFETY: `frame` is one valid `React`.
        let frame = unsafe { &*frame };
        let shape = inst.shape.shape();
        let Some(r) = shape.reactions.get(k as usize) else {
            return Err(Fault::new(0, format!("reaction {k} does not exist")));
        };
        let Some(reads) = cell_lens(shape, &r.reads, Access::ReactionRead) else {
            return Err(Fault::new(0, format!("reaction {k} names a cell its shape does not have")));
        };
        // SAFETY: abi.md §3 React: `input` holds one pointer per read cell.
        let input = unsafe { cut(frame.input, &reads) };
        // SAFETY: abi.md §3 React: `host` is the solver's `Host` for this call.
        let host = unsafe { &*frame.host };
        let host_data = frame.host_data;
        let statements = r.writes;
        let mut sink = |statement: u32, bytes: &[u8], after: f64| -> u32 {
            let declared = statements.get(statement as usize);
            if declared.is_none_or(|s| bytes.len() > s.len as usize) {
                return status::FAULT;
            }
            // SAFETY: abi.md §2 entry 6 and §12 "`commit` is called only inside
            // the `react` that received the `Host`": this is that call.
            unsafe { (host.commit)(host_data, statement, 0, bytes.len() as u32, bytes.as_ptr(), after) }
        };
        let mut cx = React::new(input, &mut sink);
        Ok(match inst.device.react(k, &mut cx)? {
            Flow::Continue => status::OK,
            Flow::Finish => status::FINISH,
        })
    })
}
