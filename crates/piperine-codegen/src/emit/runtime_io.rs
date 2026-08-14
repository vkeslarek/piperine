//! JIT task trampolines — the runtime half of the `RuntimeIo` backbone
//! (design.md P0.1). [`FormatPool`] interns format strings once (at kernel
//! compile time or from any other caller); the `pip_task_*` trampolines are
//! plain `extern "C" fn`s registered as importable symbols in a JIT module
//! the same way `piperine_lang::math::MATH_FNS`/`LIMIT_FNS` already are
//! (`jit_builder.symbol(name, ptr)`, see `kernel/analog/compile.rs` and
//! `kernel/digital/compile.rs`) — and are equally callable as ordinary Rust
//! functions, so a non-JIT caller (the analog accepted-step diagnostics
//! path, `device/analog/instance.rs`) uses the exact same code path a
//! JIT-emitted call would (Tech Decisions: "one mechanism").
//!
//! **Scope note (SPEC_DEVIATION from a literal reading of design.md):**
//! this task wires the trampolines into `AnalogKernel::diagnostics()`'s
//! only real caller today (`AnalogInstance::accept_timestep`, itself the
//! accepted-step gate `Element::accept_timestep` already provides) via a
//! direct Rust call, not a Cranelift-emitted call inside the shared
//! residual/Jacobian skeleton (`emit/analog_expr.rs`, CLAUDE.md "Files not
//! to edit casually"). Every diagnostic collected today
//! (`flatten/analog.rs`'s `$display`/`$write`/... arm) is a *static*
//! compile-time format string with no interpolated arguments — there is
//! nothing dynamic a JIT evaluation would add — so this is not a silent
//! under-delivery, it is the correctly-scoped implementation for today's
//! actual data shape. `argc`/`argv`/`TaskArg` are already part of the ABI
//! so that a future task threading dynamic arguments through
//! `flatten/analog.rs` (P3/T43+, per design.md's component table) is
//! additive, not a signature change. JIT-callability of the trampolines
//! themselves is proven directly (`tests/runtime_io_jit_call.rs`), and
//! both JIT builders link every symbol below.

use std::sync::{Mutex, OnceLock};

use piperine_solver::prelude::{Channel, SimOutput};

use crate::emit::abi::SimCtx;

// ---------------------------------------------------------------------------
// TaskArg — the JIT-ABI dynamic-argument interchange format
// ---------------------------------------------------------------------------

/// One dynamic argument to a task call. `kind` selects which of `i`/`f`/
/// `ptr`+`len` is meaningful (0 = none, 1 = `i`, 2 = `f`, 3 = a `ptr`+`len`
/// UTF-8 string) — a plain tag rather than a Rust enum so the layout is
/// `#[repr(C)]`-stable for Cranelift-emitted stores. Not yet populated by
/// any call site (see this module's own scope note); the shape exists now
/// so a later task's argument marshaling is additive.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TaskArg {
    pub kind: u8,
    pub i: i64,
    pub f: f64,
    pub ptr: *const u8,
    pub len: u32,
}

// ---------------------------------------------------------------------------
// FormatPool
// ---------------------------------------------------------------------------

/// Interns format strings so a call site passes a small `u32` id instead of
/// allocating/copying a string per call (MD-18: no re-JIT once a kernel is
/// compiled, and — the same spirit applied to this new path — no
/// per-timestep string parsing either). One process-wide pool: format
/// strings are compile-time source constants, never per-session data, so
/// every compiled kernel (analog or digital, across every session sharing
/// this process) interns into the same table; entries are never freed —
/// the format-string set is small and bounded by the source text compiled
/// so far, the same "leaks by design" posture Cranelift's own JIT pages
/// already have in this codebase.
pub struct FormatPool;

fn pool() -> &'static Mutex<Vec<String>> {
    static POOL: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    POOL.get_or_init(|| Mutex::new(Vec::new()))
}

impl FormatPool {
    /// Interns `s`, returning its id. Two calls with the same text return
    /// the same id (compile-time dedup — two kernels emitting the same
    /// literal format string share one entry).
    pub fn intern(s: &str) -> u32 {
        let mut p = pool().lock().expect("FormatPool mutex poisoned");
        if let Some(pos) = p.iter().position(|e| e == s) {
            return pos as u32;
        }
        p.push(s.to_string());
        (p.len() - 1) as u32
    }

    /// Renders the format string for `id`. An id can only be produced by
    /// [`FormatPool::intern`], so a miss here is an internal-invariant
    /// violation (a corrupted id crossing the JIT boundary), not a
    /// user-facing condition — reported inline rather than panicking
    /// mid-simulation over a diagnostic-rendering detail.
    pub fn render(id: u32) -> String {
        let p = pool().lock().expect("FormatPool mutex poisoned");
        match p.get(id as usize) {
            Some(s) => s.clone(),
            None => format!("<unknown format id {id}>"),
        }
    }
}

// ---------------------------------------------------------------------------
// Trampolines
// ---------------------------------------------------------------------------

/// Renders `fmt_id`'s text. `argc`/`argv` are accepted (matching the ABI)
/// but not yet interpolated — see this module's scope note.
fn render_task_text(fmt_id: u32, _argc: u32, _argv: *const TaskArg) -> String {
    FormatPool::render(fmt_id)
}

/// # Safety
/// `ctx` must be a valid, non-null pointer to a live [`SimCtx`] whose `io`
/// field is either null (in which case this is a no-op) or points to a
/// live [`SimOutput`] for the duration of the call. Every `pip_task_*`
/// trampoline in this module shares this contract.
unsafe fn sink_of<'a>(ctx: *mut SimCtx) -> Option<&'a dyn SimOutput> {
    if ctx.is_null() {
        return None;
    }
    let sim = unsafe { &*ctx };
    if sim.io.is_null() {
        return None;
    }
    Some(unsafe { &*sim.io })
}

/// `$display`-family output (§9.2): renders `fmt_id` and writes it to
/// `Channel::Stdout` with a trailing newline.
pub extern "C" fn pip_task_display(ctx: *mut SimCtx, fmt_id: u32, argc: u32, argv: *const TaskArg) {
    let Some(sink) = (unsafe { sink_of(ctx) }) else { return };
    let text = render_task_text(fmt_id, argc, argv);
    sink.write(Channel::Stdout, &text);
    sink.write(Channel::Stdout, "\n");
}

/// `$write`-family output (§9.2): like [`pip_task_display`] but without the
/// trailing newline `$display` adds.
pub extern "C" fn pip_task_write(ctx: *mut SimCtx, fmt_id: u32, argc: u32, argv: *const TaskArg) {
    let Some(sink) = (unsafe { sink_of(ctx) }) else { return };
    let text = render_task_text(fmt_id, argc, argv);
    sink.write(Channel::Stdout, &text);
}

/// `$finish`/`$stop` (§9.7): flushes the sink. Actually ending the analysis
/// is the solver's own concern (a typed result, not this trampoline's
/// job) — this only guarantees buffered output reaches the host first.
pub extern "C" fn pip_task_finish(ctx: *mut SimCtx) {
    if let Some(sink) = unsafe { sink_of(ctx) } {
        sink.flush();
    }
}

/// `$fatal` (§9.7): renders `fmt_id` to `Channel::Stderr` and flushes.
/// Aborting the analysis is the caller's concern (design.md's Error
/// Handling Strategy: a typed solver error, matching `$finish`/`$stop`
/// semantics) — this trampoline only guarantees the message is visible
/// before that happens.
pub extern "C" fn pip_task_fatal(ctx: *mut SimCtx, fmt_id: u32, argc: u32, argv: *const TaskArg) {
    let Some(sink) = (unsafe { sink_of(ctx) }) else { return };
    let text = render_task_text(fmt_id, argc, argv);
    sink.write(Channel::Stderr, &text);
    sink.write(Channel::Stderr, "\n");
    sink.flush();
}

/// The table both JIT builders register (`kernel/analog/compile.rs`,
/// `kernel/digital/compile.rs`) — mirrors `MathFn`'s shape
/// (`piperine_lang::math::MathFn`) so registration is the same
/// `for f in TASK_FNS { jit_builder.symbol(f.name, f.ptr); }` pattern.
pub struct TaskFn {
    pub name: &'static str,
    pub ptr: *const u8,
}

// The table is read-only function pointers; sharing across threads is safe
// (matching `MathFn`'s own `unsafe impl Sync` — see `piperine_lang::math`).
unsafe impl Sync for TaskFn {}

pub const TASK_FNS: &[TaskFn] = &[
    TaskFn { name: "pip_task_display", ptr: pip_task_display as *const u8 },
    TaskFn { name: "pip_task_write", ptr: pip_task_write as *const u8 },
    TaskFn { name: "pip_task_finish", ptr: pip_task_finish as *const u8 },
    TaskFn { name: "pip_task_fatal", ptr: pip_task_fatal as *const u8 },
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::offset_of;

    #[test]
    fn format_pool_dedups_identical_strings() {
        let a = FormatPool::intern("hello %d");
        let b = FormatPool::intern("hello %d");
        let c = FormatPool::intern("goodbye");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(FormatPool::render(a), "hello %d");
        assert_eq!(FormatPool::render(c), "goodbye");
    }

    #[test]
    fn format_pool_render_of_unknown_id_is_loud_not_a_panic() {
        // A corrupted/foreign id must never crash a running simulation —
        // it renders a visible placeholder instead.
        let rendered = FormatPool::render(u32::MAX);
        assert!(rendered.contains("unknown format id"));
    }

    /// **CRITICAL (design.md Risks table):** `SimCtx` is a JIT-ABI struct —
    /// field offsets are baked into emitted code. `io` must be the very
    /// last field, so every prior field keeps the offset the JIT already
    /// assumes. This test fails if a future edit inserts a field after
    /// `srcfact` and before `io`, or reorders either.
    #[test]
    fn io_field_is_appended_last() {
        assert_eq!(
            offset_of!(SimCtx, io),
            offset_of!(SimCtx, srcfact) + std::mem::size_of::<f64>(),
            "SimCtx.io must immediately follow srcfact (the prior last-appended field) — \
             see the CRITICAL note in emit/abi.rs and design.md's Risks table"
        );
        // And every f64 field, right through srcfact, keeps a byte offset
        // strictly before `io`'s.
        assert!(offset_of!(SimCtx, temperature) < offset_of!(SimCtx, io));
        assert!(offset_of!(SimCtx, srcfact) < offset_of!(SimCtx, io));
    }

    #[test]
    fn pip_task_display_writes_to_the_ctx_sink() {
        use piperine_solver::prelude::BufferSink;
        let sink = BufferSink::new();
        let mut ctx = SimCtx::default();
        ctx.io = &sink as *const dyn SimOutput;
        let id = FormatPool::intern("hello from the trampoline");
        pip_task_display(&mut ctx as *mut SimCtx, id, 0, std::ptr::null());
        assert_eq!(sink.stdout(), "hello from the trampoline\n");
    }

    #[test]
    fn pip_task_write_has_no_trailing_newline() {
        use piperine_solver::prelude::BufferSink;
        let sink = BufferSink::new();
        let mut ctx = SimCtx::default();
        ctx.io = &sink as *const dyn SimOutput;
        let id = FormatPool::intern("no newline here");
        pip_task_write(&mut ctx as *mut SimCtx, id, 0, std::ptr::null());
        assert_eq!(sink.stdout(), "no newline here");
    }

    #[test]
    fn pip_task_fatal_writes_to_stderr_and_flushes() {
        use piperine_solver::prelude::BufferSink;
        let sink = BufferSink::new();
        let mut ctx = SimCtx::default();
        ctx.io = &sink as *const dyn SimOutput;
        let id = FormatPool::intern("fatal condition");
        pip_task_fatal(&mut ctx as *mut SimCtx, id, 0, std::ptr::null());
        assert_eq!(sink.stderr(), "fatal condition\n");
        assert_eq!(sink.stdout(), "");
    }

    #[test]
    fn null_ctx_or_null_io_is_a_safe_no_op() {
        // A null ctx must never crash — it's a caller-side condition the
        // trampoline tolerates rather than propagates as a segfault.
        pip_task_display(std::ptr::null_mut(), 0, 0, std::ptr::null());
        let mut ctx = SimCtx::default();
        ctx.io = std::ptr::null::<piperine_solver::prelude::StdoutSink>() as *const dyn SimOutput;
        pip_task_display(&mut ctx as *mut SimCtx, 0, 0, std::ptr::null());
        // No panic reaching here is the assertion.
    }

    #[test]
    fn task_fns_table_names_match_the_registered_pointers() {
        let names: Vec<&str> = TASK_FNS.iter().map(|f| f.name).collect();
        assert_eq!(names, ["pip_task_display", "pip_task_write", "pip_task_finish", "pip_task_fatal"]);
        for f in TASK_FNS {
            assert!(!f.ptr.is_null());
        }
    }

    /// Proves the exact mechanism `kernel/analog/compile.rs` and
    /// `kernel/digital/compile.rs` register (`jit_builder.symbol(f.name,
    /// f.ptr)` for every `TASK_FNS` entry) is genuinely JIT-callable — a
    /// real Cranelift-compiled function, not just a Rust function invoked
    /// directly, calls `pip_task_display` through the same import
    /// mechanism `math::MATH_FNS`/`LIMIT_FNS` use. Isolated in its own
    /// tiny JIT module rather than wired into `AnalogKernel`'s shared
    /// residual/Jacobian skeleton (`emit/analog_expr.rs`, CLAUDE.md "Files
    /// not to edit casually") — see this module's own scope note.
    #[test]
    fn pip_task_display_is_callable_from_jit_compiled_code() {
        use cranelift_codegen::ir::{types, AbiParam, InstBuilder, Signature};
        use cranelift_codegen::isa::CallConv;
        use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
        use cranelift_jit::{JITBuilder, JITModule};
        use cranelift_module::{Linkage, Module};
        use piperine_solver::prelude::BufferSink;

        let mut jit_builder =
            JITBuilder::new(cranelift_module::default_libcall_names()).expect("JITBuilder::new");
        for f in TASK_FNS {
            jit_builder.symbol(f.name, f.ptr);
        }
        let mut jit = JITModule::new(jit_builder);

        // extern "C" fn(ctx: *mut SimCtx, fmt_id: u32, argc: u32, argv: *const TaskArg)
        let mut display_sig = Signature::new(CallConv::SystemV);
        display_sig.params.push(AbiParam::new(types::I64)); // ctx
        display_sig.params.push(AbiParam::new(types::I32)); // fmt_id
        display_sig.params.push(AbiParam::new(types::I32)); // argc
        display_sig.params.push(AbiParam::new(types::I64)); // argv
        let display_id = jit
            .declare_function("pip_task_display", Linkage::Import, &display_sig)
            .expect("declare pip_task_display");

        // A caller function: fn(ctx: *mut SimCtx, fmt_id: u32) that forwards
        // to pip_task_display(ctx, fmt_id, 0, null).
        let mut caller_sig = Signature::new(CallConv::SystemV);
        caller_sig.params.push(AbiParam::new(types::I64));
        caller_sig.params.push(AbiParam::new(types::I32));
        let caller_id = jit
            .declare_function("call_display", Linkage::Export, &caller_sig)
            .expect("declare call_display");

        let mut ctx_ir = jit.make_context();
        ctx_ir.func.signature = caller_sig;
        let mut fb_ctx = FunctionBuilderContext::new();
        {
            let mut builder = FunctionBuilder::new(&mut ctx_ir.func, &mut fb_ctx);
            let block = builder.create_block();
            builder.append_block_params_for_function_params(block);
            builder.switch_to_block(block);
            let ctx_param = builder.block_params(block)[0];
            let fmt_id_param = builder.block_params(block)[1];
            let argc = builder.ins().iconst(types::I32, 0);
            let argv = builder.ins().iconst(types::I64, 0);
            let display_ref = jit.declare_func_in_func(display_id, builder.func);
            builder.ins().call(display_ref, &[ctx_param, fmt_id_param, argc, argv]);
            builder.ins().return_(&[]);
            builder.seal_block(block);
            builder.finalize();
        }
        jit.define_function(caller_id, &mut ctx_ir).expect("define call_display");
        jit.clear_context(&mut ctx_ir);
        jit.finalize_definitions().expect("finalize");

        let call_display: extern "C" fn(*mut SimCtx, u32) =
            unsafe { std::mem::transmute(jit.get_finalized_function(caller_id)) };

        let sink = BufferSink::new();
        let mut sim_ctx = SimCtx::default();
        sim_ctx.io = &sink as *const dyn SimOutput;
        let fmt_id = FormatPool::intern("jit-emitted call reached the sink");

        call_display(&mut sim_ctx as *mut SimCtx, fmt_id);

        assert_eq!(sink.stdout(), "jit-emitted call reached the sink\n");
    }
}
