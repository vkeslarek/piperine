//! JIT codegen: the IR is compiled — analog *and* digital — to native code
//! via Cranelift. There is no interpreted execution path.
//!
//! - [`analog`] compiles an [`crate::ir::AnalogBody`] into an
//!   [`analog::AnalogKernel`]: residual, Jacobian, charge, force, noise, and
//!   state-input functions over a fixed `f64` ABI.
//! - [`digital`] compiles an [`crate::ir::DigitalBody`] into a
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

pub use crate::error::CodegenError;

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
    /// Element multiplicity (`$mfactor`).
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
    /// AC analysis frequency in Hz (`$ac_frequency`); used by noise
    /// sources whose PSD depends on frequency (`flicker_noise`). Zero
    /// outside AC analyses. Appended last so existing field offsets for
    /// the JIT-ABI struct are preserved.
    pub frequency: f64,
    /// Independent-source scale factor for DC source stepping
    /// (`$simparam("sourceScaleFactor")`, ngspice `CKTsrcFact`). `1.0`
    /// everywhere except while the source-stepping homotopy ramps the DC
    /// operating point. Appended last to preserve JIT-ABI field offsets.
    pub srcfact: f64,
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
            frequency: 0.0,
            srcfact: 1.0,
        }
    }
}
