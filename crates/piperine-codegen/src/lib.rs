//! Piperine codegen: lowers the POM (`piperine_lang::pom::Design`) straight
//! to native code — no separate IR crate. Verilog-AMS (the IR's other former
//! producer) is gone, so the resolved form lives here as a codegen-private
//! stage.
//!
//! ```text
//! pom::Design ─▶ resolve ─▶ flatten ─▶ emit ─▶ kernel ─▶ device ─▶ solver Element
//! ```
//!
//! One module per pipeline stage:
//!
//! - [`resolve`] — the resolved expression/statement form plus the
//!   POM→resolved pass (formerly the standalone `piperine-ir` crate +
//!   `piperine-lang`'s `lowering/`). Codegen-private: nothing outside this
//!   crate depends on its shape anymore.
//! - `flatten` (crate-private) — resolved analog body → `FlatAnalog`
//!   (contributions/forces/events/noise hoisted for emission).
//! - `emit` (crate-private) — the reusable Cranelift emission machinery
//!   (`Builder`, the `Codegen` trait, CSE, the [`SimCtx`] ABI struct).
//! - [`kernel`] — the compiled products: analog residual/Jacobian/charge/
//!   force/noise kernels and digital comb/seq/watch kernels. No interpreter.
//! - [`device`] — kernels wrapped as `piperine_solver` devices, plus the
//!   program-level [`device::CircuitCompiler`].
//!
//! Public surface: a single façade below — `resolve`/`kernel`/`device` stay
//! `pub` because hosts and tests address them by deep path (e.g.
//! `kernel::digital::network`, `resolve::pom`); `emit`/`flatten`/`error` are
//! crate-private, with their host-facing items re-exported here instead —
//! this crate has one deliverable, not a two-tier host/plugin split.

mod emit;
mod error;
mod flatten;
pub mod device;
pub mod kernel;
pub mod resolve;

pub use device::{BuiltInstanceInfo, CircuitBuildInfo, CircuitCompiler, CompiledModule, PiperineDevice};
pub use emit::SimCtx;
pub use error::CodegenError;
pub use kernel::analog::AnalogKernel;
pub use kernel::digital::DigitalKernel;
