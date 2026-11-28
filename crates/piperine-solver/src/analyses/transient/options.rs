//! The options of a transient: its time span, its step bounds, what it records and keeps.

/// What a transient keeps of the steps it takes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Keep {
    /// One `TransientStep` per accepted step (the default).
    #[default]
    Every,
    /// The last accepted step only, with its re-entry state: a run whose host reads the steps
    /// through a `StepObserver` holds nothing per step.
    Last,
}

/// Which steps of a transient carry the state a later run re-enters from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Reentry {
    /// Every recorded step (the default): any of them can be re-entered.
    #[default]
    EveryStep,
    /// The last step only: the other steps carry none, and a run that records many steps holds
    /// one state instead of one per step.
    LastStep,
}

/// What a transient runs: its time span, its step bounds, what it records and which steps it keeps.
#[derive(Clone)]
pub struct TransientAnalysisOptions {
    /// Simulation stop time
    pub stop_time: f64,

    /// Initial timestep for the adaptive stepper (SPICE has been adaptive
    /// since v2; the integrator varies `dt` from here via the PI controller).
    /// A user-supplied `.step` becomes this initial value.
    pub dt: f64,

    /// Minimum allowed timestep (default: 1e-15 seconds)
    pub dt_min: f64,

    /// Hard budget on time-loop iterations (accepted + rejected attempts).
    /// Default 2_000_000 — far above any healthy run, low enough that a
    /// stepper that collapses onto `dt_min` fails loud in seconds instead of
    /// grinding for hours with unbounded memory growth.
    ///
    /// Without this cap `solve()`'s `while current_time < stop_time` has no
    /// termination guarantee against a stepper that keeps retrying: a Newton failure of
    /// a step already at the `dt_min` floor fails the run (`E0508`), so this is
    /// the backstop for a controller that never reaches it.
    pub max_steps: usize,

    /// Maximum allowed timestep (default: stop_time / 100)
    pub dt_max: f64,

    /// Earliest time at which a step is *recorded* (host `run_tran` `start`
    /// `TranConfig.start`). The solver still integrates from t=0 — the state
    /// evolution matters — but steps with `t < record_from` are dropped from
    /// the result (ngspice `.tran tstart tstop` semantics). Defaults to 0
    /// (record everything, the pre-existing behavior).
    pub record_from: f64,

    /// Simulation start time (default 0). The integrator's clock starts
    /// here — `$abstime`, breakpoints, and scheduled sets are all absolute
    /// times. Used by a host restarting a transient from `t` after a
    /// structural rebuild (LIVE-16); the starting state comes from the
    /// initial operating point overlaid with `apply_initial_conditions`.
    pub start_time: f64,

    /// Opt-in per-step recording of device runtime banks (state/vars),
    /// keyed by device label on each [`TransientStep`](crate::prelude::TransientStep). Off by default —
    /// enabling it clones the stateful devices' banks per accepted step.
    /// With it on, `Trace.i` recomputes currents of state-reading devices
    /// (`delay`/`transition`/`idt`); with it off, that read stays a loud
    /// error. When on, this is the shorthand for "record every observable
    /// on every device" — equivalent to a `probe_selection` enumerating
    /// every device's full observable catalog (ABI-34).
    pub record_device_state: bool,

    /// Per-device observable requests for selective recording (ABI-33/34).
    /// Empty by default (no selective recording — the prior all-or-nothing
    /// behavior stays controlled by `record_device_state`). When non-empty,
    /// `collect_device_banks` records only the requested observables for
    /// each device — a host running a long transient with one probe pays
    /// `O(requested)` memory, not `O(devices × steps)`. Validation
    /// (`list_observables` membership for every request) runs once at
    /// solver setup (ABI-35).
    pub probe_selection: crate::core::introspect::ProbeSelection,

    /// The default `ttol` of a crossing (docs/spec §17.3 rule 2, Appendix C.2.1): a `CROSS`
    /// watch with no `ttol` pin is located until the landing exceeds the estimated crossing by
    /// at most this, or its value is under its `etol`. Default 1 ps.
    pub ttol: f64,

    /// Which steps the result holds. `Keep::Last` takes neither `record_device_state` nor a
    /// `probe_selection` (a transient of devices refuses them at setup): with one step kept,
    /// the recording they ask for has nothing to attach to. `record_from` has nothing to act on.
    pub keep: Keep,

    /// Which steps carry a re-entry state (a transient of devices). `Reentry::LastStep` is for a
    /// run that continues from its last step only: a re-entry from any other step fails.
    pub reentry: Reentry,
}

impl TransientAnalysisOptions {
    /// The `dt` bounds the stepper strategy clamps into.
    pub fn step_limits(&self) -> piperine_analog_solver::convergence::StepLimits {
        piperine_analog_solver::convergence::StepLimits { dt_min: self.dt_min, dt_max: self.dt_max }
    }

    /// Create transient options. The integrator is always adaptive (PI
    /// controller); `dt` is the initial step size, grown/shrunk from there.
    pub fn new(stop_time: f64, dt: f64) -> Self {
        Self {
            stop_time,
            dt,
            dt_min: 1e-15,
            max_steps: 2_000_000,
            dt_max: (stop_time / 100.0),
            record_from: 0.0,
            start_time: 0.0,
            record_device_state: false,
            probe_selection: crate::core::introspect::ProbeSelection::new(),
            ttol: 1e-12,
            keep: Keep::Every,
            reentry: Reentry::EveryStep,
        }
    }

    /// Set which steps carry a re-entry state.
    /// Kept for a host that re-enters a transient from a state (`tests/transient_reentry.rs`).
    pub fn with_reentry(mut self, reentry: Reentry) -> Self {
        self.reentry = reentry;
        self
    }

    /// Whether every recorded step is to carry a re-entry state.
    pub(super) fn reenters_every_step(&self) -> bool {
        self.keep == Keep::Every && self.reentry == Reentry::EveryStep
    }

    /// Set which steps the result holds.
    pub fn with_keep(mut self, keep: Keep) -> Self {
        self.keep = keep;
        self
    }

    /// Set the simulation start time (restart-from-`t` semantics).
    pub fn with_start(mut self, start_time: f64) -> Self {
        self.start_time = start_time;
        self
    }

    /// Set minimum timestep
    /// Kept for a host that bounds the smallest transient step (`tests/failure_rules.rs`).
    pub fn with_dt_min(mut self, dt_min: f64) -> Self {
        self.dt_min = dt_min;
        self
    }

    /// Set the time-loop iteration budget (see [`Self::max_steps`]).
    pub fn with_max_steps(mut self, max_steps: usize) -> Self {
        self.max_steps = max_steps;
        self
    }

    /// Set maximum timestep
    pub fn with_dt_max(mut self, dt_max: f64) -> Self {
        self.dt_max = dt_max;
        self
    }

    /// Set the earliest recorded time (`TranConfig.start`).
    /// Kept for a host that records from a time on (`piperine-api` `tests/grid.rs`).
    pub fn with_record_from(mut self, record_from: f64) -> Self {
        self.record_from = record_from;
        self
    }

    /// Set the per-device observable selection (ABI-33). When non-empty,
    /// `collect_device_banks` records only the requested observables
    /// (instead of the full `(state, vars)` banks every step). An empty
    /// selection leaves the prior behavior in place: nothing recorded
    /// unless `record_device_state = true`. Unknown device/observable
    /// requests fail loud at solver setup (ABI-35).
    /// Kept for a host that selects the probes a transient records (`tests/probe_selection.rs`).
    pub fn with_probe_selection(
        mut self,
        selection: crate::core::introspect::ProbeSelection,
    ) -> Self {
        self.probe_selection = selection;
        self
    }
}
