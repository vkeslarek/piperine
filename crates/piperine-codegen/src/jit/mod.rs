//! JIT codegen: the IR is compiled — analog *and* digital — to native code
//! via Cranelift. There is no interpreted execution path.
//!
//! - [`analog`] compiles an [`crate::resolve::AnalogBody`] into an
//!   [`analog::AnalogKernel`]: residual, Jacobian, charge, force, noise, and
//!   state-input functions over a fixed `f64` ABI.
//! - [`digital`] compiles an [`crate::resolve::DigitalBody`] into a
//!   [`digital::DigitalKernel`]: combinational, sequential (register update),
//!   and event-watch functions over quad-coded `i64` signals.
//!
//! Kernels are per *module* and shared across instances; per-instance state
//! (parameter values, operator history, register banks) lives in
//! [`crate::device`].

pub mod analog;
pub mod digital;
pub mod flatten;
pub use piperine_lang::math;

pub use crate::emit::abi::SimCtx;
pub use crate::error::CodegenError;
