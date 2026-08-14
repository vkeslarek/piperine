//! The analog JIT ABI: live simulator state threaded through every
//! JIT-compiled analog function.

/// Live simulator state threaded through every JIT-compiled analog function.
///
/// `#[repr(C)]` keeps the layout stable across the JIT ABI; the emitter reads
/// fields by their byte offsets (see the `emit` module).
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
    /// The runtime I/O sink `pip_task_*` trampolines (`emit::runtime_io`)
    /// write through — a *borrowed* reference (`SimCtx` stays `Copy`,
    /// unlike an owned `Rc<dyn SimOutput>` would allow), valid for the
    /// duration of the call that provided this `SimCtx`. Defaults to a
    /// `'static` [`piperine_solver::prelude::StdoutSink`] singleton.
    /// Appended last, per this struct's own established convention, to
    /// preserve every prior field's byte offset — a mis-ordered field here
    /// silently corrupts every JIT-compiled device (see
    /// `runtime_io.rs::tests::io_field_is_appended_last`).
    pub io: *const dyn piperine_solver::prelude::SimOutput,
}

// SAFETY: `SimCtx` is `Copy`/`Send`/`Sync` in the same sense every other
// raw-pointer-carrying JIT-ABI struct in this codebase is — the pointer is
// never dereferenced except by the `pip_task_*` trampolines, which uphold
// their own `# Safety` contract (a live `SimCtx` whose `io` outlives the
// call). No thread-safety is claimed for `*const dyn SimOutput` itself.
unsafe impl Send for SimCtx {}
unsafe impl Sync for SimCtx {}

/// The default sink every `SimCtx::default()` (and `at_temperature`) points
/// `io` at — a zero-sized singleton, so no allocation backs the default.
static DEFAULT_SINK: piperine_solver::prelude::StdoutSink = piperine_solver::prelude::StdoutSink;

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
            io: &DEFAULT_SINK as *const dyn piperine_solver::prelude::SimOutput,
        }
    }
}
