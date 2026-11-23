//! The generic `extern "C"` entry points over a `Device` (abi.md §2, §7):
//! `module::<D>(name)` (and `module_with(name, factory)` for a `Factory`, the
//! factory then being the module's `data`) fills a `Module` with thunks that
//! cut the frame into the lenses' slices, call the device inside `catch_unwind`, and turn its
//! result into a status and a note. No function unwinds across the boundary.

use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::device::{Device, Eval, Factory, Flow, Fault, React, Values, cell_lens, cell_lens_of};
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

/// The `Module` of a `&'static` factory: its prototype, the factory as the
/// module's `data`, and four entry points that read the factory (`create`) or
/// run the instance it made (`eval`, `react`, `destroy`) with the same checks
/// and the same `catch_unwind` as `module::<D>`.
pub fn module_with<F: Factory>(name: &'static str, factory: &'static F) -> Module {
    Module {
        name: Str::from_static(name),
        prototype: factory.prototype(),
        data: (factory as *const F).cast::<c_void>(),
        create: create_with::<F>,
        destroy: destroy_with::<F>,
        eval: eval_with::<F>,
        react: react_with::<F>,
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
    // SAFETY: abi.md §12 "The library and its prototypes live until the
    // process ends": `module` and its prototype are valid.
    unsafe { create_from(module, values, note, D::create) }
}

unsafe extern "C" fn create_with<F: Factory>(module: *const Module, values: List<Value>, note: *mut Note) -> Created {
    // SAFETY: abi.md §12 "The library and its prototypes live until the
    // process ends": `module` is valid, and `module_with::<F>` set its `data`
    // to a `&'static F`.
    let factory: &'static F = unsafe { &*(*module).data.cast::<F>() };
    // SAFETY: as above, `module` and its prototype are valid.
    unsafe { create_from(module, values, note, |v| factory.create(v)) }
}

/// The body of every `create`: checks each value against the prototype's
/// `PARAM` pins, calls `make`, validates the shape it returns and boxes the
/// instance as an `Inst<T>`.
///
/// # Safety
///
/// `module` points to a live `Module` whose prototype is valid (abi.md §12).
unsafe fn create_from<T>(
    module: *const Module,
    values: List<Value>,
    note: *mut Note,
    make: impl FnOnce(&Values<'_>) -> Result<(T, ShapeRef), Fault>,
) -> Created {
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
        let (device, shape) = make(&Values::new(given))?;
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
    // SAFETY: abi.md §2 "destroy(instance) frees the instance": `instance`
    // came from `create::<D>`, an `Inst<D>`, and is destroyed once.
    unsafe { destroy_inst::<D>(instance) }
}

unsafe extern "C" fn destroy_with<F: Factory>(instance: *mut Instance) {
    // SAFETY: abi.md §2 "destroy(instance) frees the instance": `instance`
    // came from `create_with::<F>`, an `Inst<F::Instance>`, destroyed once.
    unsafe { destroy_inst::<F::Instance>(instance) }
}

/// Drops an `Inst<T>`; a panicking `Drop` does not unwind into C.
///
/// # Safety
///
/// `instance` is null or came from `Box::into_raw` of an `Inst<T>` in
/// `create_from`, and is destroyed once.
unsafe fn destroy_inst<T>(instance: *mut Instance) {
    if instance.is_null() {
        return;
    }
    // SAFETY: abi.md §2 "destroy(instance) frees the instance": the caller's
    // contract above.
    let inst = unsafe { Box::from_raw(instance.cast::<Inst<T>>()) };
    // A panicking Drop must not unwind into C; the instance is gone either way.
    let _ = catch_unwind(AssertUnwindSafe(move || drop(inst)));
}

unsafe extern "C" fn eval<D: Device>(instance: *mut Instance, k: u32, frame: *const raw::Eval) -> u32 {
    // SAFETY: abi.md §3 "Every pointer is non-null" and §12 "No two calls on
    // one instance overlap": `instance` is a live `Inst<D>` from `create::<D>`.
    unsafe { eval_on::<D>(instance, k, frame, D::eval) }
}

unsafe extern "C" fn eval_with<F: Factory>(instance: *mut Instance, k: u32, frame: *const raw::Eval) -> u32 {
    // SAFETY: as in `eval::<D>`; `instance` is a live `Inst<F::Instance>` from
    // `create_with::<F>`.
    unsafe { eval_on::<F::Instance>(instance, k, frame, F::eval) }
}

/// The body of every `eval`: cuts the frame into the lens's slices and runs
/// `run` on the instance inside `guarded`.
///
/// # Safety
///
/// `frame` is one valid `raw::Eval` (abi.md §3) and `instance` a live
/// `Inst<T>` from `create_from`, with no other call on it in flight (§12).
unsafe fn eval_on<T>(
    instance: *mut Instance,
    k: u32,
    frame: *const raw::Eval,
    run: impl FnOnce(&T, u32, &mut Eval<'_>) -> Result<(), Fault>,
) -> u32 {
    // SAFETY: abi.md §3 "Every pointer is non-null": `frame` is one `Eval`.
    let note = unsafe { (*frame).note };
    guarded(note, || {
        // SAFETY: abi.md §12 "No two calls on one instance overlap": `instance`
        // is a live `Inst<T>` from `create_from`, shared for this call only.
        let inst = unsafe { &*instance.cast::<Inst<T>>() };
        // SAFETY: as above, `frame` is one valid `Eval`.
        let frame = unsafe { &*frame };
        let shape = inst.shape.shape();
        let Some(f) = shape.functions.get(k as usize) else {
            return Err(Fault::new(0, format!("function {k} does not exist")));
        };
        let reads = cell_lens_of(shape, &f.reads, &f.elements, Access::FunctionRead);
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
        run(&inst.device, k, &mut cx)?;
        Ok(status::OK)
    })
}

unsafe extern "C" fn react<D: Device>(instance: *mut Instance, k: u32, frame: *const raw::React) -> u32 {
    // SAFETY: abi.md §3 "Every pointer is non-null" and §12 "No two calls on
    // one instance overlap": `instance` is a live `Inst<D>` from `create::<D>`.
    unsafe { react_on::<D>(instance, k, frame, D::react) }
}

unsafe extern "C" fn react_with<F: Factory>(instance: *mut Instance, k: u32, frame: *const raw::React) -> u32 {
    // SAFETY: as in `react::<D>`; `instance` is a live `Inst<F::Instance>`
    // from `create_with::<F>`.
    unsafe { react_on::<F::Instance>(instance, k, frame, F::react) }
}

/// The body of every `react`: cuts the frame, routes commits to the host and
/// runs `run` on the instance inside `guarded`.
///
/// # Safety
///
/// `frame` is one valid `raw::React` (abi.md §3) and `instance` a live
/// `Inst<T>` from `create_from`, with no other call on it in flight (§12).
unsafe fn react_on<T>(
    instance: *mut Instance,
    k: u32,
    frame: *const raw::React,
    run: impl FnOnce(&T, u32, &mut React<'_>) -> Result<Flow, Fault>,
) -> u32 {
    // SAFETY: abi.md §3 "Every pointer is non-null": `frame` is one `React`.
    let note = unsafe { (*frame).note };
    guarded(note, || {
        // SAFETY: as in `eval_on`.
        let inst = unsafe { &*instance.cast::<Inst<T>>() };
        // SAFETY: `frame` is one valid `React`.
        let frame = unsafe { &*frame };
        let shape = inst.shape.shape();
        let Some(r) = shape.reactions.get(k as usize) else {
            return Err(Fault::new(0, format!("reaction {k} does not exist")));
        };
        let Some(reads) = cell_lens_of(shape, &r.reads, &r.elements, Access::ReactionRead) else {
            return Err(Fault::new(0, format!("reaction {k} names a cell its shape does not have")));
        };
        // SAFETY: abi.md §3 React: `input` holds one pointer per read cell.
        let input = unsafe { cut(frame.input, &reads) };
        // SAFETY: abi.md §3 React: `host` is the solver's `Host` for this call.
        let commit = unsafe { (*frame.host).commit };
        // SAFETY: as above; a host whose `size` reaches `commit_element` has it (abi.md §3, `Host.size`).
        let commit_element = unsafe {
            let host = &*frame.host;
            (host.size as usize >= size_of::<raw::Host>()).then_some(host.commit_element)
        };
        let host_data = frame.host_data;
        let statements = r.writes;
        let mut sink = |statement: u32, offset: u32, bytes: &[u8], after: f64| -> u32 {
            let Some(declared) = statements.get(statement as usize) else { return status::FAULT };
            if offset as u64 + bytes.len() as u64 > declared.len as u64 {
                return status::FAULT;
            }
            // SAFETY: abi.md §2 entry 6 and §12 "`commit` is called only inside
            // the `react` that received the `Host`": this is that call.
            unsafe { commit(host_data, statement, declared.offset + offset, bytes.len() as u32, bytes.as_ptr(), after) }
        };
        let mut element_sink = |statement: u32, element: u64, offset: u32, bytes: &[u8], after: f64| -> u32 {
            let Some(declared) = statements.get(statement as usize) else { return status::FAULT };
            let Some(commit_element) = commit_element else { return status::FAULT };
            if offset as u64 + bytes.len() as u64 > declared.len as u64 {
                return status::FAULT;
            }
            // SAFETY: as for `commit` above.
            unsafe {
                commit_element(host_data, statement, element, declared.offset + offset, bytes.len() as u32, bytes.as_ptr(), after)
            }
        };
        let mut cx = React::with_elements(input, &mut sink, &mut element_sink);
        Ok(match run(&inst.device, k, &mut cx)? {
            Flow::Continue => status::OK,
            Flow::Finish => status::FINISH,
        })
    })
}
