//! Piperine codegen: lowers the POM (`piperine_lang::pom::Design`) straight
//! to native code — no separate IR crate. Verilog-AMS (the IR's other former
//! producer) is gone, so the resolved form lives here as a private module.
//!
//! ```text
//! pom::Design ─▶ lower (resolved bodies) ─▶ jit (Cranelift kernels) ─▶ device (solver Devices)
//! ```
//!
//! - [`lower`] (re-exported as [`ir`] for call-site continuity) — the
//!   resolved expression/statement form plus the POM→resolved pass
//!   (formerly the standalone `piperine-ir` crate + `piperine-lang`'s
//!   `lowering/`). Codegen-private: nothing outside this crate depends on
//!   its shape anymore.
//! - [`jit`] — Cranelift compilation: analog residual/Jacobian/charge/force/
//!   noise kernels and digital comb/seq/watch kernels. No interpreter.
//! - [`device`] — kernels wrapped as `piperine_solver` devices, plus the
//!   program-level [`device::CircuitCompiler`].

pub mod device;
pub mod jit;
pub mod lower;

pub use lower as ir;

pub use device::{BuiltInstanceInfo, CircuitBuildInfo, CircuitCompiler, CompiledModule, PiperineDevice};
pub use jit::analog::AnalogKernel;
pub use jit::digital::DigitalKernel;
pub use jit::{CodegenError, SimCtx};
