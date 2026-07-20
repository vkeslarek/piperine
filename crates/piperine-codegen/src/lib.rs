//! Piperine codegen: lowers the POM (`piperine_lang::pom::Design`) straight
//! to native code — no separate IR crate. Verilog-AMS (the IR's other former
//! producer) is gone, so the resolved form lives here as a private module.
//!
//! ```text
//! pom::Design ─▶ resolve (resolved bodies) ─▶ kernel (Cranelift kernels) ─▶ device (solver Devices)
//! ```
//!
//! - [`resolve`] — the resolved expression/statement form plus the
//!   POM→resolved pass (formerly the standalone `piperine-ir` crate +
//!   `piperine-lang`'s `lowering/`). Codegen-private: nothing outside this
//!   crate depends on its shape anymore.
//! - [`kernel`] — Cranelift compilation: analog residual/Jacobian/charge/force/
//!   noise kernels and digital comb/seq/watch kernels. No interpreter.
//! - [`device`] — kernels wrapped as `piperine_solver` devices, plus the
//!   program-level [`device::CircuitCompiler`].

pub mod device;
pub mod emit;
pub mod error;
pub mod flatten;
pub mod kernel;
pub mod resolve;

pub use device::{BuiltInstanceInfo, CircuitBuildInfo, CircuitCompiler, CompiledModule, PiperineDevice};
pub use emit::SimCtx;
pub use error::CodegenError;
pub use kernel::analog::AnalogKernel;
pub use kernel::digital::DigitalKernel;
