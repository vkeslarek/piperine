//! `piperine-codegen::backend` — IR to Cranelift (MD-41's `backend` stage).
//!
//! [`analog`] (T18-T20) and [`digital`] (T21-T22) each JIT-compile one
//! `piperine_ir::core::Block`'s op sequence into a native function through
//! the shared engine below. Both domains share the same [`Op`] set and the
//! same environment ABI ([`BlockEnv`]) — a raw-pointer struct mirroring the
//! reference interpreter's `State`/`Interpreter::set_node_values`/
//! `set_signal`/`seed_value` (`piperine_ir::interp`), so the JIT backend and
//! the reference interpreter are handed EXACTLY the same external inputs.
//! That equality of inputs is what makes the differential harness (LOV-11)
//! a real cross-check rather than two paths that could never disagree.
//!
//! ## Scope note vs. `piperine_ir::interp::Interpreter`
//!
//! Like the reference interpreter (see `interp.rs`'s module doc), this
//! backend does not solve KCL and does not fire its own triggers — it
//! compiles and runs ONE block's op sequence given externally-supplied net/
//! seed/cell values, exactly mirroring the interpreter's scope. Trigger
//! firing (`Always`/`Edge`/`Cross`/`Timer`/`Initial`) is evaluated by the
//! caller (the differential-test `Evaluator` wrappers in
//! `piperine-codegen/tests/`), the same as `Interpreter::step` does for the
//! reference path.
//!
//! ## Deviation from design.md's literal reuse note
//!
//! design.md's Data Models/Component sections say `backend::{analog,digital}`
//! "Reuses: `emit/` (`Builder`, CSE, `SimCtx`) unchanged". `emit::Builder` is
//! hard-coupled to `resolve::LoweredBody`/`NodeId`/`ParamId`/`VarId` (the OLD
//! pipeline's resolved form) — there is no POM/resolve form for IR fixtures
//! at this phase (POM→IR lowering is Phase 6, T26+), so `Builder` cannot
//! literally accept an IR `Module`/`Op` without first inventing a fake
//! `LoweredBody`, which would be pretend-reuse, not real reuse. This module
//! instead reuses the same MACHINERY-LEVEL idioms `emit/` established: libm
//! imported as JIT symbols via `piperine_lang::math::MATH_FNS` (the exact
//! table `kernel/analog/compile.rs` already imports the same way, byte for
//! byte — `JITBuilder::symbol` + one `declare_function(Linkage::Import)`
//! per entry), and a raw-pointer runtime-state ABI in the spirit of
//! `emit::SimCtx`. `Engine` holds its `JITModule` directly rather than
//! wrapping it in `crate::emit::OwnedJit` (the code-page-ownership guard
//! `kernel/analog` uses once a kernel is done building): `OwnedJit` exists
//! to bound a long-running *process*'s cumulative JIT-page growth across
//! many re-elaborations, which does not apply here — every `Engine` in
//! this phase is scoped to one differential-test fixture's lifetime, and
//! `cranelift_jit`'s default (leak on drop, per `OwnedJit`'s own module
//! doc) is memory-SAFE, only less page-frugal. Recorded here rather than
//! silently claimed as "unchanged" reuse — flagged in the batch report as
//! a SPEC_DEVIATION.

pub mod analog;
pub mod digital;
mod engine;

pub use engine::{BackendError, BlockEnv, CellLayout, CompiledFn, Engine, PortMap};
