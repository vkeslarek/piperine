//! Piperine codegen: the shared IR (`docs/SPEC.md`) and its all-JIT lowering
//! to the native solver.
//!
//! ```text
//! frontends ─▶ ir::IrProgram ─▶ jit (Cranelift kernels) ─▶ device (solver Devices)
//! ```
//!
//! - [`ir`] — re-export of [`piperine_ir`], the post-elaboration resolved IR
//!   (its own crate: the frontend/backend contract).
//! - [`jit`] — Cranelift compilation: analog residual/Jacobian/charge/force/
//!   noise kernels and digital comb/seq/watch kernels. No interpreter.
//! - [`device`] — kernels wrapped as `piperine_solver` devices, plus the
//!   program-level [`device::CircuitCompiler`].

pub mod device;
pub mod jit;

pub use piperine_ir as ir;

pub use device::{BuiltInstanceInfo, CircuitBuildInfo, CircuitCompiler, CompiledModule, PiperineDevice};
pub use jit::analog::AnalogKernel;
pub use jit::digital::DigitalKernel;
pub use jit::{CodegenError, SimCtx};
