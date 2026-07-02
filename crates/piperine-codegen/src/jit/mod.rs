//! JIT codegen: the IR is compiled — analog *and* digital — to native code
//! via Cranelift. There is no interpreted execution path.
//!
//! - [`analog`] compiles an [`crate::ir::IrAnalogBody`] into an
//!   [`analog::AnalogKernel`]: residual, Jacobian, charge, force, noise, and
//!   state-input functions over a fixed `f64` ABI.
//! - [`digital`] compiles an [`crate::ir::IrDigitalBody`] into a
//!   [`digital::DigitalKernel`]: combinational, sequential (register update),
//!   and event-watch functions over quad-coded `i64` signals.
//!
//! Kernels are per *module* and shared across instances; per-instance state
//! (parameter values, operator history, register banks) lives in
//! [`crate::device`].

pub mod analog;
pub mod digital;
pub mod flatten;
pub mod math;

mod diff;
mod emit;

use thiserror::Error;

/// Errors from IR validation and JIT compilation. Every unimplemented
/// lowering is a *named* error — nothing ever silently compiles to `0.0`.
#[derive(Debug, Error)]
pub enum CodegenError {
    #[error("module `{0}` not found in IrProgram")]
    ModuleNotFound(String),
    #[error("IR validation failed: {0}")]
    Invalid(String),
    #[error("Cranelift module error: {0}")]
    Module(String),
    #[error("unsupported construct: {0}")]
    Unsupported(String),
    #[error("constant evaluation failed: {0}")]
    ConstEval(String),
    #[error("function lowering failed: {0}")]
    Function(String),
}

impl CodegenError {
    pub fn unsupported(what: impl Into<String>) -> Self {
        Self::Unsupported(what.into())
    }
}

/// Live simulator state threaded through every JIT-compiled analog function.
///
/// `#[repr(C)]` keeps the layout stable across the JIT ABI; the emitter reads
/// fields by their byte offsets (see [`emit`]).
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SimCtx {
    /// Simulation temperature in Kelvin.
    pub temperature: f64,
    /// Absolute simulation time in seconds (`$abstime`).
    pub abstime: f64,
    /// Device multiplicity (`$mfactor`).
    pub mfactor: f64,
    /// Matrix-regularisation conductance (`$simparam("gmin")`).
    pub gmin: f64,
    /// Current solver time step.
    pub step: f64,
    /// Solver stop time.
    pub tfinal: f64,
    /// Bitmask of user-provided parameters (`$param_given`), 64 params max.
    pub param_given_mask: u64,
    /// The running analysis, as `Analysis as u64` (`$analysis`).
    pub current_analysis: u64,
}

impl SimCtx {
    /// Boltzmann constant over elementary charge in V/K, so
    /// `vt = K_B_OVER_Q * T`. CODATA 2018.
    pub const K_B_OVER_Q: f64 = 8.617_333_262e-5;

    pub fn at_temperature(temperature: f64) -> Self {
        Self { temperature, ..Self::default() }
    }
}

impl Default for SimCtx {
    /// T = 300 K, t = 0, mfactor = 1, gmin = 1e-12.
    fn default() -> Self {
        Self {
            temperature: 300.0,
            abstime: 0.0,
            mfactor: 1.0,
            gmin: 1.0e-12,
            step: 0.0,
            tfinal: 0.0,
            param_given_mask: 0,
            current_analysis: 0,
        }
    }
}
