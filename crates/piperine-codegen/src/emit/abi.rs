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
    /// Bitmask of ports connected at this instance's own instantiation
    /// (`$port_connected`, §9.19, LRMC-C7-09, T50), one bit per port in
    /// declared order, 64 ports max (the same convention/cap
    /// `param_given_mask` already uses). Appended after `io` — that
    /// field's own doc comment explains why append-only ordering
    /// matters for this struct.
    pub port_connected_mask: u64,
    /// The `$fopen`/`$fclose`/`$fdisplay`/`$fwrite`/... descriptor table
    /// (§9.5, T55/LRMW2-48) the `pip_task_f*` trampolines
    /// (`emit::runtime_io`) read/write through — a *borrowed* mutable
    /// pointer (same convention as `io` above: `SimCtx` stays `Copy`),
    /// valid for the duration of the call that provided this `SimCtx`.
    /// Null by default (`SimCtx::default()`) — a null `files` is a safe
    /// no-op for every `pip_task_f*` trampoline, same contract as `io`.
    /// Appended last, per this struct's established convention.
    pub files: *mut piperine_solver::prelude::FileTable,
    /// The `$strobe`/`$monitor` Monitor-region deferral queue (§8.5.1,
    /// T62/LRMW2-59) — `pip_task_defer_monitor` (`emit::runtime_io`)
    /// pushes a format-string id here instead of printing immediately;
    /// the digital device (`device/digital.rs`) drains it (in FIFO order,
    /// calling `pip_task_display` directly) only after the enclosing
    /// `seq`/`comb` call *and* its NBA commit have both finished — the
    /// Monitor region's "after every earlier region at this time has
    /// drained" ordering, reusing `Region::Monitor` conceptually without
    /// a literal solver-scheduler event (the compiled kernel is one
    /// synchronous call; there is no mid-call yield point to schedule
    /// into). A *borrowed* mutable pointer, same convention as `files`/
    /// `io` above (`SimCtx` stays `Copy`) — null by default, a safe no-op
    /// for `pip_task_defer_monitor`. Appended last, per this struct's
    /// established convention.
    pub monitor_queue: *mut Vec<u32>,
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
            // Every port connected by default — the common case; a real
            // instance's own mask (computed from its actual connections,
            // `device/builder.rs`) overrides this.
            port_connected_mask: u64::MAX,
            files: std::ptr::null_mut(),
            monitor_queue: std::ptr::null_mut(),
        }
    }
}
