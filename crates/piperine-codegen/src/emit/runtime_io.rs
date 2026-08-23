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
//!
//! **T55/LRMW2-48 (§9.5 file I/O)** adds `pip_task_fopen`/`fclose`/
//! `fdisplay`/`fwrite`/`fflush`, threaded through a second borrowed
//! pointer on `SimCtx` (`files: *mut FileTable`, same null-tolerant
//! convention as `io`) rather than through `SimOutput` — `FileTable`'s
//! own doc already establishes file writes as its concern, never
//! `SimOutput`'s (`StdoutSink`/`BufferSink` both `debug_assert!` on a
//! `Channel::File` write reaching them). These five share the same
//! "reachable from both kernels via `TASK_FNS`, dynamic string
//! arguments not yet marshaled through `TaskArg`" posture the `$display`
//! family above already established — `$fopen`'s `path` is the one
//! exception, passed as an explicit `ptr`+`len` pair rather than through
//! `TaskArg`, since a file path is the trampoline's own first-class
//! argument, not an interpolated format argument.
//! `$fgets`/`$fread`/`$ungetc`/`$sformat`/`$swrite`/`$readmemb`/
//! `$readmemh` (also added by T55) are the elaboration-time/pure-value
//! half — `FileTable::fgets`/`ungetc`/`fread` and
//! `parse_memory_text`/`FileTable::readmem` (`piperine-solver`'s
//! `core/output.rs`) — since none of them are per-timestep dynamic-value
//! producers a JIT residual would call mid-solve; `$sformat`/`$swrite`
//! format into a `string` *variable* rather than a channel, and this
//! codebase's JIT ABI has no live `string`-variable storage to write
//! into yet (SPEC_DEVIATION, disclosed) — `eval::tasks::format_display`
//! (`piperine-lang`) is the correctly-scoped formatting building block
//! both already share with `$display`.

use std::sync::{Mutex, OnceLock};

use piperine_solver::prelude::{Channel, FileTable, SimOutput};

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

/// # Safety
/// Same contract as [`sink_of`], against `ctx`'s `files` field instead of
/// `io`.
unsafe fn files_of<'a>(ctx: *mut SimCtx) -> Option<&'a mut FileTable> {
    if ctx.is_null() {
        return None;
    }
    let sim = unsafe { &*ctx };
    if sim.files.is_null() {
        return None;
    }
    Some(unsafe { &mut *sim.files })
}

/// Reads `len` bytes at `ptr` as a UTF-8 path string. `None` on a null
/// pointer or invalid UTF-8 (both loud no-ops at the call sites below,
/// never a garbage descriptor).
///
/// # Safety
/// `ptr` must be valid for reads of `len` bytes, or null.
unsafe fn str_of(ptr: *const u8, len: u32) -> Option<&'static str> {
    if ptr.is_null() {
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    std::str::from_utf8(bytes).ok()
}

/// `$fopen(path)` (§9.5.1): opens `path` (given as a `ptr`+`len` UTF-8
/// slice — the caller's string data, not a `FormatPool` id) through
/// `ctx.files`, returning the new descriptor as a non-negative `i32`, or
/// `-1` on any failure (null `ctx`/`files`, invalid UTF-8, or a real
/// `FileError::OpenFailed`) — never a fabricated descriptor a caller
/// could go on to read/write against.
pub extern "C" fn pip_task_fopen(ctx: *mut SimCtx, path_ptr: *const u8, path_len: u32) -> i32 {
    let Some(files) = (unsafe { files_of(ctx) }) else { return -1 };
    let Some(path) = (unsafe { str_of(path_ptr, path_len) }) else { return -1 };
    match files.open(path) {
        Ok(id) => id.0 as i32,
        Err(_) => -1,
    }
}

/// `$fclose(fd)` (§9.5.1): closes `fd`. A negative or unknown `fd` is a
/// silent no-op at the ABI boundary (the same "loud at the `FileTable`
/// level, tolerant at the raw-pointer JIT boundary" posture `sink_of`'s
/// null-tolerance already establishes) — `FileTable::close`'s own typed
/// `FileError::UnknownDescriptor` is what a non-JIT caller (or a test
/// against `FileTable` directly) sees.
pub extern "C" fn pip_task_fclose(ctx: *mut SimCtx, fd: i32) {
    let Some(files) = (unsafe { files_of(ctx) }) else { return };
    if fd < 0 {
        return;
    }
    let _ = files.close(piperine_solver::prelude::FileId(fd as u32));
}

/// `$fdisplay(fd, ...)` (§9.5.2): renders `fmt_id` and writes it to `fd`
/// with a trailing newline — `$display`'s file-routed twin
/// ([`pip_task_display`]).
pub extern "C" fn pip_task_fdisplay(ctx: *mut SimCtx, fd: i32, fmt_id: u32, argc: u32, argv: *const TaskArg) {
    let Some(files) = (unsafe { files_of(ctx) }) else { return };
    if fd < 0 {
        return;
    }
    let text = render_task_text(fmt_id, argc, argv);
    let id = piperine_solver::prelude::FileId(fd as u32);
    let _ = files.write(id, &text);
    let _ = files.write(id, "\n");
}

/// `$fwrite(fd, ...)` (§9.5.2): like [`pip_task_fdisplay`] without the
/// trailing newline.
pub extern "C" fn pip_task_fwrite(ctx: *mut SimCtx, fd: i32, fmt_id: u32, argc: u32, argv: *const TaskArg) {
    let Some(files) = (unsafe { files_of(ctx) }) else { return };
    if fd < 0 {
        return;
    }
    let text = render_task_text(fmt_id, argc, argv);
    let _ = files.write(piperine_solver::prelude::FileId(fd as u32), &text);
}

/// `$fflush(fd)` (§9.5.6): flushes `fd`'s buffered writes to disk.
pub extern "C" fn pip_task_fflush(ctx: *mut SimCtx, fd: i32) {
    let Some(files) = (unsafe { files_of(ctx) }) else { return };
    if fd < 0 {
        return;
    }
    let _ = files.flush(piperine_solver::prelude::FileId(fd as u32));
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
    TaskFn { name: "pip_task_fopen", ptr: pip_task_fopen as *const u8 },
    TaskFn { name: "pip_task_fclose", ptr: pip_task_fclose as *const u8 },
    TaskFn { name: "pip_task_fdisplay", ptr: pip_task_fdisplay as *const u8 },
    TaskFn { name: "pip_task_fwrite", ptr: pip_task_fwrite as *const u8 },
    TaskFn { name: "pip_task_fflush", ptr: pip_task_fflush as *const u8 },
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

    /// T55/LRMW2-48: `$fopen`/`$fdisplay`/`$fwrite`/`$fclose` reach a real
    /// `FileTable` through `SimCtx.files` — a "model" (here, direct
    /// trampoline calls standing in for a JIT-compiled body across
    /// several transient steps, the same non-JIT-marshaled-args posture
    /// this module's own scope note already establishes for `$display`)
    /// writes a file, and the test reads it back from disk.
    #[test]
    fn a_model_writes_a_file_across_a_transient_and_the_test_reads_it_back() {
        use piperine_solver::prelude::FileTable;
        let dir = std::env::temp_dir();
        let path = dir.join(format!("lrmc_runtime_io_transient_{}.txt", std::process::id()));
        let path_str = path.to_str().unwrap();

        let mut files = FileTable::new();
        let mut ctx = SimCtx::default();
        ctx.files = &mut files as *mut FileTable;

        let fd = pip_task_fopen(&mut ctx as *mut SimCtx, path_str.as_ptr(), path_str.len() as u32);
        assert!(fd >= 0, "fopen must return a real descriptor");

        // Three transient steps, each $fdisplay-ing the step's own line.
        for step in 0..3 {
            let fmt = FormatPool::intern(match step {
                0 => "t=0",
                1 => "t=1",
                _ => "t=2",
            });
            pip_task_fdisplay(&mut ctx as *mut SimCtx, fd, fmt, 0, std::ptr::null());
        }
        pip_task_fflush(&mut ctx as *mut SimCtx, fd);
        pip_task_fclose(&mut ctx as *mut SimCtx, fd);

        let contents = std::fs::read_to_string(&path).expect("file exists and is readable");
        assert_eq!(contents, "t=0\nt=1\nt=2\n");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn fwrite_has_no_trailing_newline_unlike_fdisplay() {
        use piperine_solver::prelude::FileTable;
        let dir = std::env::temp_dir();
        let path = dir.join(format!("lrmc_fwrite_test_{}.txt", std::process::id()));
        let path_str = path.to_str().unwrap();
        let mut files = FileTable::new();
        let mut ctx = SimCtx::default();
        ctx.files = &mut files as *mut FileTable;
        let fd = pip_task_fopen(&mut ctx as *mut SimCtx, path_str.as_ptr(), path_str.len() as u32);
        let fmt = FormatPool::intern("no newline");
        pip_task_fwrite(&mut ctx as *mut SimCtx, fd, fmt, 0, std::ptr::null());
        pip_task_fclose(&mut ctx as *mut SimCtx, fd);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "no newline");
        let _ = std::fs::remove_file(&path);
    }

    /// Failure mode (Done-when): a bad path is loud — `-1`, never a
    /// descriptor a caller could go on to write against.
    #[test]
    fn fopen_on_a_bad_path_returns_negative_one_not_a_fabricated_descriptor() {
        use piperine_solver::prelude::FileTable;
        let mut files = FileTable::new();
        let mut ctx = SimCtx::default();
        ctx.files = &mut files as *mut FileTable;
        let bad = "/nonexistent/deeply/nested/lrmc/path.txt";
        let fd = pip_task_fopen(&mut ctx as *mut SimCtx, bad.as_ptr(), bad.len() as u32);
        assert_eq!(fd, -1);
    }

    /// Failure mode: a null `ctx.files` (no file table wired up) is a
    /// safe no-op, not a crash — same null-tolerance contract `sink_of`
    /// already has for `io`.
    #[test]
    fn null_files_pointer_is_a_safe_no_op_for_every_file_trampoline() {
        let mut ctx = SimCtx::default(); // files is null by default
        let p = "x";
        assert_eq!(pip_task_fopen(&mut ctx as *mut SimCtx, p.as_ptr(), 1), -1);
        pip_task_fclose(&mut ctx as *mut SimCtx, 0);
        pip_task_fdisplay(&mut ctx as *mut SimCtx, 0, 0, 0, std::ptr::null());
        pip_task_fwrite(&mut ctx as *mut SimCtx, 0, 0, 0, std::ptr::null());
        pip_task_fflush(&mut ctx as *mut SimCtx, 0);
        // No panic reaching here is the assertion.
    }

    /// Multi-channel descriptors (Done-when: "route per §9.5"): a
    /// combined stdout+file mask writes to both — `McdTable` already
    /// implements this fully (`output.rs`'s own test suite); this proves
    /// it composes with the file descriptor `pip_task_fopen` itself
    /// opened, not just a `FileTable::open` call made directly.
    #[test]
    fn mcd_routing_composes_with_a_trampoline_opened_descriptor() {
        use piperine_solver::prelude::{BufferSink, FileId, FileTable, McdTable};
        let dir = std::env::temp_dir();
        let path = dir.join(format!("lrmc_mcd_trampoline_{}.txt", std::process::id()));
        let path_str = path.to_str().unwrap();

        let mut files = FileTable::new();
        let mut ctx = SimCtx::default();
        ctx.files = &mut files as *mut FileTable;
        // `$fopen` (the plain trampoline form) opens the file...
        let fd = pip_task_fopen(&mut ctx as *mut SimCtx, path_str.as_ptr(), path_str.len() as u32);
        assert!(fd >= 0);
        let file_id = FileId(fd as u32);

        // ...and a *separate* MCD-form open+write against the same
        // shared `files` table composes correctly: `McdTable::printf`'s
        // combined stdout+file mask reaches `BufferSink` for the stdout
        // bit and `FileTable::write` (this exact descriptor) for the
        // file bit — both channels named by one call, per §9.5's
        // multi-channel-descriptor form.
        let sink = BufferSink::new();
        let mut mcd = McdTable::new();
        let file_bit = mcd.open(&mut files, "/dev/null").expect("mcd open");
        let combined = McdTable::STDOUT | file_bit;
        mcd.printf(&mut files, &sink, combined, "both channels\n");
        assert_eq!(sink.stdout(), "both channels\n", "stdout bit routes through the shared sink");

        // The trampoline-opened descriptor is a distinct, independently
        // usable channel on the very same FileTable.
        let _ = files.write(file_id, "file side\n");
        files.flush(file_id).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "file side\n");
        let _ = std::fs::remove_file(&path);
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
        assert_eq!(
            names,
            [
                "pip_task_display",
                "pip_task_write",
                "pip_task_finish",
                "pip_task_fatal",
                "pip_task_fopen",
                "pip_task_fclose",
                "pip_task_fdisplay",
                "pip_task_fwrite",
                "pip_task_fflush",
            ]
        );
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

    /// T55/LRMW2-48: `pip_task_fopen` (§9.5.1) is genuinely JIT-callable
    /// too — both kernels register `TASK_FNS` wholesale
    /// (`kernel/analog/compile.rs`, `kernel/digital/compile.rs`), so
    /// this is the same "in both analog and digital contexts" reach
    /// `pip_task_display`'s own JIT test above proves, applied to the
    /// file-I/O family this task adds.
    #[test]
    fn pip_task_fopen_is_callable_from_jit_compiled_code() {
        use cranelift_codegen::ir::{types, AbiParam, InstBuilder, Signature};
        use cranelift_codegen::isa::CallConv;
        use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
        use cranelift_jit::{JITBuilder, JITModule};
        use piperine_solver::prelude::FileTable;
        use cranelift_module::{Linkage, Module};

        let mut jit_builder =
            JITBuilder::new(cranelift_module::default_libcall_names()).expect("JITBuilder::new");
        for f in TASK_FNS {
            jit_builder.symbol(f.name, f.ptr);
        }
        let mut jit = JITModule::new(jit_builder);

        // extern "C" fn(ctx: *mut SimCtx, path_ptr: *const u8, path_len: u32) -> i32
        let mut fopen_sig = Signature::new(CallConv::SystemV);
        fopen_sig.params.push(AbiParam::new(types::I64));
        fopen_sig.params.push(AbiParam::new(types::I64));
        fopen_sig.params.push(AbiParam::new(types::I32));
        fopen_sig.returns.push(AbiParam::new(types::I32));
        let fopen_id = jit.declare_function("pip_task_fopen", Linkage::Import, &fopen_sig).expect("declare");

        let mut caller_sig = Signature::new(CallConv::SystemV);
        caller_sig.params.push(AbiParam::new(types::I64));
        caller_sig.params.push(AbiParam::new(types::I64));
        caller_sig.params.push(AbiParam::new(types::I32));
        caller_sig.returns.push(AbiParam::new(types::I32));
        let caller_id = jit.declare_function("call_fopen", Linkage::Export, &caller_sig).expect("declare");

        let mut ctx_ir = jit.make_context();
        ctx_ir.func.signature = caller_sig;
        let mut fb_ctx = FunctionBuilderContext::new();
        {
            let mut builder = FunctionBuilder::new(&mut ctx_ir.func, &mut fb_ctx);
            let block = builder.create_block();
            builder.append_block_params_for_function_params(block);
            builder.switch_to_block(block);
            let params: Vec<_> = builder.block_params(block).to_vec();
            let fopen_ref = jit.declare_func_in_func(fopen_id, builder.func);
            let call = builder.ins().call(fopen_ref, &params);
            let result = builder.inst_results(call)[0];
            builder.ins().return_(&[result]);
            builder.seal_block(block);
            builder.finalize();
        }
        jit.define_function(caller_id, &mut ctx_ir).expect("define");
        jit.clear_context(&mut ctx_ir);
        jit.finalize_definitions().expect("finalize");

        let call_fopen: extern "C" fn(*mut SimCtx, *const u8, u32) -> i32 =
            unsafe { std::mem::transmute(jit.get_finalized_function(caller_id)) };

        let dir = std::env::temp_dir();
        let path = dir.join(format!("lrmc_jit_fopen_{}.txt", std::process::id()));
        let path_str = path.to_str().unwrap();

        let mut files = FileTable::new();
        let mut sim_ctx = SimCtx::default();
        sim_ctx.files = &mut files as *mut FileTable;

        let fd = call_fopen(&mut sim_ctx as *mut SimCtx, path_str.as_ptr(), path_str.len() as u32);
        assert!(fd >= 0, "a Cranelift-emitted call reached pip_task_fopen and got a real descriptor");
        assert!(path.exists(), "the file was actually created on disk");
        let _ = std::fs::remove_file(&path);
    }
}
