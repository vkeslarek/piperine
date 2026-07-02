//! Piperine codegen: the shared IR (`docs/SPEC.md`) and its all-JIT lowering
//! to the native solver.
//!
//! ```text
//! frontends ─▶ ir::IrProgram ─▶ jit (Cranelift kernels) ─▶ device (solver Devices)
//! ```
//!
//! - [`ir`] — the post-elaboration, resolved IR both frontends lower into.
//! - [`jit`] — Cranelift compilation: analog residual/Jacobian/charge/force/
//!   noise kernels and digital comb/seq/watch kernels. No interpreter.
//! - [`device`] — kernels wrapped as `piperine_solver` devices, plus the
//!   program-level [`device::CircuitCompiler`].

pub mod device;
pub mod ir;
pub mod jit;

pub use device::{CircuitCompiler, CompiledModule, PiperineDevice};
pub use jit::analog::AnalogKernel;
pub use jit::digital::DigitalKernel;
pub use jit::{CodegenError, SimCtx};
