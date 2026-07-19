//! Transient (`.tran`) — the element-facing state/options, the UIC hold
//! clamps (ngspice `CKTsetIC`), and the adaptive TR-BDF2 driver: unified
//! breakpoint table, Milne-LTE accept gate, PI timestep control, and live
//! scheduled parameter sets.
#![allow(dead_code)]
use crate::analog::AnalogReference;
use crate::analyses::Context;
use crate::analyses::convergence::StepperStrategy;
use crate::analyses::dc::DcSolver;
use crate::core::circuit::CircuitInstance;
use crate::digital::LogicValue;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::faer::FaerSparseLinearSystem;
use crate::math::integration::{TrBdf2, TrBdf2Phase};
use crate::math::iv::InitialValue;
use crate::math::linear::{AsIndex, Stamp};
use crate::math::newton_raphson::{NewtonRaphsonSolver, NonLinearSystem};
use crate::prelude::{TransientAnalysisResult, TransientStep};

use log::debug;
use ndarray::ArrayView1;
use std::collections::HashMap;
use std::ops::Deref;

// ── request/state ────────────────────────────────────────────────────────

/// The read-only state an element sees while stamping the transient system: the
/// analog solution history **and** the digital net snapshot it may read (D2A,
/// no device-side cache). Derefs to the analog history buffer.
pub struct TransientAnalysisState<'a> {
    history: &'a CircularArrayBuffer2<f64>,
    /// Every digital net's logic value for this step, indexed by `DigitalNet`.
    pub digital: &'a [LogicValue],
}

impl<'a> TransientAnalysisState<'a> {
    pub fn new(history: &'a CircularArrayBuffer2<f64>, digital: &'a [LogicValue]) -> Self {
        Self { history, digital }
    }

    /// The analog solution history buffer.
    pub fn history(&self) -> &CircularArrayBuffer2<f64> {
        self.history
    }
}

impl Deref for TransientAnalysisState<'_> {
    type Target = CircularArrayBuffer2<f64>;
    fn deref(&self) -> &Self::Target {
        self.history
    }
}

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
    /// keyed by device label on each [`TransientStep`]. Off by default —
    /// enabling it clones the stateful devices' banks per accepted step.
    /// With it on, `Trace.i` recomputes currents of state-reading devices
    /// (`delay`/`transition`/`idt`); with it off, that read stays a loud
    /// error.
    pub record_device_state: bool,
}

impl TransientAnalysisOptions {
    /// Create transient options. The integrator is always adaptive (PI
    /// controller); `dt` is the initial step size, grown/shrunk from there.
    pub fn new(stop_time: f64, dt: f64) -> Self {
        Self {
            stop_time,
            dt,
            dt_min: 1e-15,
            dt_max: (stop_time / 100.0),
            record_from: 0.0,
            start_time: 0.0,
            record_device_state: false,
        }
    }

    /// Set the simulation start time (restart-from-`t` semantics).
    pub fn with_start(mut self, start_time: f64) -> Self {
        self.start_time = start_time;
        self
    }

    /// Set minimum timestep
    pub fn with_dt_min(mut self, dt_min: f64) -> Self {
        self.dt_min = dt_min;
        self
    }

    /// Set maximum timestep
    pub fn with_dt_max(mut self, dt_max: f64) -> Self {
        self.dt_max = dt_max;
        self
    }

    /// Set the earliest recorded time (`TranConfig.start`).
    pub fn with_record_from(mut self, record_from: f64) -> Self {
        self.record_from = record_from;
        self
    }
}

/// Per-analysis config for transient. Built from
/// [`TransientAnalysisOptions`] via `From`. Carries the tunables that
/// used to be on the global `Context` (MD-03).
#[derive(Debug, Clone)]
pub struct TransientContext {
    pub dt: f64,
    pub dt_min: f64,
    pub dt_max: f64,
    pub record_from: f64,
    pub stop_time: f64,
}

impl From<TransientAnalysisOptions> for TransientContext {
    fn from(opts: TransientAnalysisOptions) -> Self {
        Self {
            dt: opts.dt,
            dt_min: opts.dt_min,
            dt_max: opts.dt_max,
            record_from: opts.record_from,
            stop_time: opts.stop_time,
        }
    }
}

/// Per-step transient context handed to the kernel. Carries the TR-BDF2
/// phase being stamped and the step sizes; the kernel calls
/// `TrBdf2::phase_coeffs(phase, h)` for the reactive companion — there is no
/// method-selection surface (TR-BDF2 is the sole integration scheme).
#[derive(Clone, Copy)]
pub struct TransientAnalysisContext {
    pub time: f64,
    pub tfinal: f64,
    /// Which sub-step the kernel is stamping: [`Trapezoidal`][TrBdf2Phase::Trapezoidal]
    /// over `γh` (solving for `x_{n+γ}`) or [`Bdf2`][TrBdf2Phase::Bdf2] over
    /// `(1−γ)h` (solving for `x_{n+1}` from `x_{n+γ}` and `x_n`).
    pub phase: crate::math::integration::TrBdf2Phase,
    /// The full step size `h = t_{n+1} − t_n`. The companion sub-step (`γh` or
    /// `(1−γ)h`) is derived from `phase` inside `TrBdf2::phase_coeffs`.
    pub h: f64,
    /// The previous accepted step size. The TR stage's trapezoidal companion
    /// needs the capacitor current at `t_n`, which the kernel re-derives from
    /// the prior step's BDF2 formula using this. Zero on the first step (no
    /// history → no current, matching the DC operating point).
    pub prev_h: f64,
}

pub trait TransientAnalysis {
    fn load_transient(
        &mut self,
        circuit_states: &TransientAnalysisState<'_>,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>>;

    fn load_transient_dynamic(
        &mut self,
        _circuit_states: &TransientAnalysisState<'_>,
        _transient_analysis_context: &TransientAnalysisContext,
        _context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        vec![]
    }

    fn initial_transient_values(
        &mut self,
        _context: &Context,
    ) -> Vec<InitialValue<AnalogReference, f64>> {
        Vec::new()
    }
}



#[cfg(test)]
mod step_tests {
    #[allow(unused_imports)]
    use super::*;
    use crate::prelude::TransientStep;
    use crate::core::net::Net;
    use crate::digital::LogicValue;
    use std::collections::HashMap;
    use crate::analog::{AnalogReference, AnalogVariable, NodeIdentifier};
    use std::sync::Arc;

    #[test]
    fn transient_step_lookup_by_net_returns_analog_and_digital_values() {
        let var: Arc<AnalogVariable> = Arc::new(AnalogVariable::Node(NodeIdentifier::Anonymous(0)));
        let mut values = HashMap::new();
        values.insert(var.clone(), 1.25);
        let step = TransientStep::new(0.0, values).with_digital(vec![LogicValue::One, LogicValue::Zero]);

        let analog_net: Net = (&AnalogReference::new(var.clone(), 0)).into();
        assert_eq!(step.get_net(&analog_net), Some(1.25));

        let digital_net = Net::digital(1, "top.clk");
        assert_eq!(step.digital_net(&digital_net), Some(LogicValue::Zero));
        assert_eq!(step.digital_net(&Net::digital(0, "d0")), Some(LogicValue::One));

        // Wrong kind returns None — analog_net is not a digital net.
        assert_eq!(step.digital_net(&analog_net), None);

        // Digital net past the recorded snapshot returns None.
        assert_eq!(step.digital_net(&Net::digital(99, "x")), None);
    }
}

// ── driver ───────────────────────────────────────────────────────────────

// UIC hold clamps (ngspice `CKTsetIC` analog): an `@initial` branch seed
// is enforced during the t=0 solve and held through the first accepted
// step by a large conductance across the seeded branch carrying `G·ic` —
// the seed value becomes the *consistent* t=0 solution (the rest of the
// circuit solves against it), not just a Newton guess overlaid on an
// inconsistent operating point.

/// One seeded branch clamp: hold `V(plus) − V(minus) ≈ ic`.
#[derive(Debug, Clone)]
pub struct UicClamp {
    pub plus: AnalogReference,
    pub minus: Option<AnalogReference>,
    pub ic: f64,
}

impl UicClamp {
    /// Clamp conductance: large enough to pin the branch against any circuit
    /// admittance, small enough to keep the matrix conditioned.
    pub const G: f64 = 1.0e12;

    /// Stamp `G·(v − ic)`: conductance across the branch plus the `G·ic`
    /// offset current that pins the branch voltage to `ic`.
    pub fn stamp(&self, stamps: &mut Vec<Stamp<AnalogReference, f64>>) {
        stamps.push(Stamp::Matrix(self.plus.clone(), self.plus.clone(), Self::G));
        stamps.push(Stamp::Rhs(self.plus.clone(), Self::G * self.ic));
        if let Some(minus) = &self.minus {
            stamps.push(Stamp::Matrix(minus.clone(), minus.clone(), Self::G));
            stamps.push(Stamp::Matrix(self.plus.clone(), minus.clone(), -Self::G));
            stamps.push(Stamp::Matrix(minus.clone(), self.plus.clone(), -Self::G));
            stamps.push(Stamp::Rhs(minus.clone(), -Self::G * self.ic));
        }
    }
}

pub struct TransientSystem<'a> {
    pub circuit: &'a mut CircuitInstance,
    pub context: Context,
    /// Absolute time at the point this phase solves (t_{n+γ} for the TR stage,
    /// t_{n+1} for the BDF2 stage).
    pub time: f64,
    /// Which TR-BDF2 sub-step the next assemble stamps.
    pub phase: TrBdf2Phase,
    /// Full step size `h = t_{n+1} − t_n`.
    pub h: f64,
    /// Last accepted step size, so the TR stage can re-derive the previous
    /// capacitor current. Updated after each accepted BDF2 phase.
    pub prev_h: f64,
    pub tfinal: f64,
    /// UIC hold clamps (ngspice `CKTsetIC`): `@initial` branch seeds pinned
    /// through the t=0 solve and the first accepted step.
    pub uic_clamps: Vec<UicClamp>,
    /// While true, the clamps stamp — released after the first accepted step.
    pub uic_hold: bool,
}

impl<'a> NonLinearSystem<AnalogReference, f64> for TransientSystem<'a> {
    fn assemble(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
    ) -> crate::result::Result<Vec<Stamp<AnalogReference, f64>>> {
        // TR-BDF2 is the sole integration scheme — no Gear order ramp. The
        // kernel derives the companion from `phase` + `h` via the centralised
        // `TrBdf2::phase_coeffs` (MD-07); the TR stage's previous-current
        // term is re-derived from `prev_h` inside the kernel.
        let tran_ctx = TransientAnalysisContext {
            time: self.time,
            tfinal: self.tfinal,
            phase: self.phase,
            h: self.h,
            prev_h: self.prev_h,
        };

        let mut all_stamps = Vec::new();

        self.circuit.update_all(state, &self.context);
        let CircuitInstance { devices, digital_state, .. } = &mut *self.circuit;
        let tran_state = TransientAnalysisState::new(state, &digital_state.nets);
        for tran in devices.iter_mut() {
            all_stamps.extend(tran.load_transient(&tran_state, &tran_ctx, &self.context));
        }

        // gshunt: user-set circuit-wide diagonal conductance on every node.
        let gshunt = self.context.tolerances.gshunt;
        if gshunt > 0.0 {
            for r in self.circuit.netlist().all_references() {
                if r.variable().is_node() && !r.variable().is_ground() {
                    all_stamps.push(Stamp::Matrix(r.clone(), r.clone(), gshunt));
                }
            }
        }

        // UIC hold clamps: pinned through the first accepted step.
        if self.uic_hold {
            for clamp in &self.uic_clamps {
                clamp.stamp(&mut all_stamps);
            }
        }

        Ok(all_stamps)
    }

    fn netlist(&self) -> &crate::analog::Netlist {
        self.circuit.netlist()
    }

    fn any_limiting(&self) -> bool {
        self.circuit.devices.iter().any(|d| d.limiting_active())
    }

    fn apply_convergence_hints(&self, guess: ndarray::ArrayViewMut1<f64>) {
        self.circuit.apply_convergence_hints(guess);
    }

    fn update_sources(&mut self, _state: &mut CircularArrayBuffer2<f64>) {}

    fn convergence_success_callback(
        &mut self,
        _state: &CircularArrayBuffer2<f64>,
        _: &ArrayView1<f64>,
    ) {
    }
}

/// One host-scheduled live parameter write: at simulation time `t`, set
/// `param` on the element labeled `label` to `value`.
#[derive(Debug, Clone)]
pub struct ScheduledSet {
    pub t: f64,
    pub label: String,
    pub param: String,
    pub value: crate::core::introspect::Value,
}

/// Pending live sets for a running transient (LIVE-06). Entries keep their
/// scheduling order, so applying a drained batch in order gives
/// last-write-wins per param; every entry's `t` feeds the unified
/// breakpoint table (TRB-11), so the integrator lands exactly on each set
/// time with the discontinuity edge rules (skip LTE, reset `prev_h`).
#[derive(Debug, Default)]
struct SetQueue {
    entries: Vec<ScheduledSet>,
}

impl SetQueue {
    fn push(&mut self, set: ScheduledSet) {
        self.entries.push(set);
    }

    /// The earliest pending set time strictly after `from` — the next
    /// landing point this queue asks of the breakpoint table. Absolute
    /// time, so it survives step rollback.
    fn next_breakpoint(&self, from: f64) -> Option<f64> {
        self.entries
            .iter()
            .map(|s| s.t)
            .filter(|&t| t > from)
            .min_by(f64::total_cmp)
    }

    /// Remove and return every entry due at or before `now`, preserving
    /// scheduling order (application order = last-write-wins per param).
    fn drain_due(&mut self, now: f64) -> Vec<ScheduledSet> {
        let (due, pending): (Vec<_>, Vec<_>) =
            std::mem::take(&mut self.entries).into_iter().partition(|s| s.t <= now);
        self.entries = pending;
        due
    }
}

pub struct TransientSolver<'a> {
    pub system: TransientSystem<'a>,
    pub solver: NewtonRaphsonSolver<AnalogReference, f64, FaerSparseLinearSystem<f64>>,
    pub options: TransientAnalysisOptions,
    /// User-supplied initial node voltages (host `run_tran` `ic`
    /// `TranConfig.ic`), pushed after the DC operating point so the t=0
    /// state reflects them. Milestone-1: a seed (the companion model's
    /// first step may show a transient); full enforced-hold is deferred.
    initial_conditions: Vec<InitialValue<AnalogReference, f64>>,
    /// Stateful PI timestep controller (TRB-07).
    stepper: crate::analyses::convergence::PiController,
    /// Convergence tunables for this analysis (MD-04). Defaults on
    /// construction; hosts override before [`solve`](Self::solve).
    pub policy: crate::analyses::Policy,
    /// Host-scheduled live parameter writes (LIVE-06/09).
    sets: SetQueue,
    /// Full-state re-entry point (PSS shooting enabler): when set, the run
    /// starts from this captured step instead of a DC operating point.
    reentry_state: Option<TransientStep>,
}

impl<'a> TransientSolver<'a> {
    pub fn new(
        circuit: &'a mut CircuitInstance,
        options: TransientAnalysisOptions,
        context: Context,
    ) -> crate::result::Result<Self> {
        Context::init_global();
        circuit.setup_all(&context)?;

        // Build DAG topology once before simulation begins
        circuit.rebuild_digital_topology();
        circuit.init_digital()?;

        let size = circuit.netlist().max_index().map(|i| i + 1).unwrap_or(0);

        let mut system = TransientSystem {
            circuit,
            context,
            time: options.start_time,
            phase: TrBdf2Phase::Trapezoidal,
            h: options.dt,
            prev_h: 0.0,
            tfinal: options.stop_time,
            uic_clamps: Vec::new(),
            uic_hold: false,
        };

        let solver = NewtonRaphsonSolver::new(&mut system, size, 4)?;

        Ok(Self {
            system,
            solver,
            options,
            initial_conditions: Vec::new(),
            stepper: crate::analyses::convergence::PiController::default(),
            policy: crate::analyses::Policy::default(),
            sets: SetQueue::default(),
            reentry_state: None,
        })
    }

    /// Start the integration from a previously captured step — analog
    /// solution and digital snapshot — instead of a DC operating point
    /// (full-state re-entry, the PSS shooting seam). Pair with
    /// [`TransientAnalysisOptions::with_start`] so the clock continues from
    /// the capture time; user/device initial conditions are ignored (the
    /// captured state *is* the initial condition).
    pub fn with_initial_state(&mut self, state: &TransientStep) {
        self.reentry_state = Some(state.clone());
    }

    /// Schedule a live parameter write for simulation time `t` (LIVE-06):
    /// the integrator lands exactly on `t` (unified breakpoint table), the
    /// write applies there, and the new value takes effect from the next
    /// accepted step. Writes reporting ≥
    /// [`Invalidation::Restamp`](crate::core::introspect::Invalidation)
    /// restamp naturally; ≥ `OperatingPoint` triggers a consistent re-solve
    /// at `t` before the run continues (LIVE-09). Several sets on the same
    /// param apply in scheduling order — last write wins.
    pub fn schedule_set(
        &mut self,
        t: f64,
        label: impl Into<String>,
        param: impl Into<String>,
        value: crate::core::introspect::Value,
    ) {
        self.sets.push(ScheduledSet { t, label: label.into(), param: param.into(), value });
    }

    /// Seed the transient's t=0 state with user initial node voltages
    /// (the host session's `ic`). Applied after the DC operating
    /// point in `compute_initial_conditions`.
    pub fn apply_initial_conditions(&mut self, ivs: Vec<InitialValue<AnalogReference, f64>>) {
        self.initial_conditions = ivs;
    }

    fn compute_initial_conditions(&mut self) -> crate::result::Result<TransientStep> {
        // Full-state re-entry: seed both companion-history rows from the
        // captured solution and restore the digital snapshot — no DC solve,
        // no device/user seeds (the captured state is the whole story).
        if let Some(state) = self.reentry_state.take() {
            let ivs: Vec<InitialValue<AnalogReference, f64>> = {
                let netlist = self.system.circuit.netlist();
                netlist
                    .all_references()
                    .into_iter()
                    .filter_map(|reference| {
                        state
                            .get(reference.variable().clone())
                            .map(|value| InitialValue { reference: reference.clone(), value })
                    })
                    .collect()
            };
            for idx in 0..self.system.circuit.digital_state.nets.len() {
                if let Some(lv) = state.digital(idx) {
                    self.system.circuit.digital_state.nets[idx] = lv;
                }
            }
            // Hidden register state (module vars, edge memory) round-trips
            // with the nets — the full-state shot contract (PSS). Restored
            // after `init_digital` (constructor) so the restore wins over the
            // power-on reset; unknown labels are skipped (a structurally
            // rebuilt circuit starts its new devices fresh).
            for dev in &mut self.system.circuit.devices {
                if let Some(hidden) = state.digital_hidden(dev.name()) {
                    let hidden = hidden.clone();
                    dev.digital_hidden_restore(&hidden);
                }
            }
            self.solver.push_initial_conditions(ivs.clone());
            self.solver.push_initial_conditions(ivs);
            return Ok(self.snapshot(self.options.start_time));
        }

        debug!("Calculating DC Operating Point...");
        // UIC hold clamps (ngspice `CKTsetIC`): the `@initial` branch seeds
        // pin the t=0 solve so the seed is the *consistent* operating point,
        // and stay stamped through the first accepted step.
        let uic_clamps: Vec<UicClamp> = self
            .system
            .circuit
            .devices
            .iter()
            .flat_map(|dev| dev.initial_conditions())
            .filter_map(|(plus, minus, ic)| {
                plus.map(|plus| UicClamp { plus, minus, ic })
            })
            .collect();
        let mut dc_solver = DcSolver::new(self.system.circuit, Context::default())?;
        dc_solver.system.uic_clamps = uic_clamps.clone();
        let dc_result = dc_solver.solve()?;
        self.system.uic_clamps = uic_clamps;
        self.system.uic_hold = !self.system.uic_clamps.is_empty();

        let _netlist = self.system.circuit.netlist();
        let iv_dc = self.system.circuit.netlist().initial_values(dc_result.values());

        // Element `@initial { V(p,n) <- ic }` UIC seeds: set the t=0 branch
        // voltage `v(plus) = v(minus) + ic` (cap/ind/dio initial condition,
        // SPICE `.ic`). Overlaid on the DC point so unconstrained nodes keep
        // their operating-point values.
        let mut device_ic: Vec<InitialValue<AnalogReference, f64>> = Vec::new();
        for dev in &self.system.circuit.devices {
            for (plus, minus, ic) in dev.initial_conditions() {
                let Some(plus_ref) = plus else { continue };
                let v_minus = minus
                    .as_ref()
                    .and_then(|m| m.as_index())
                    .map_or(0.0, |i| iv_dc.iter().find(|iv| iv.reference.as_index() == Some(i)).map_or(0.0, |iv| iv.value));
                device_ic.push(InitialValue { reference: plus_ref, value: v_minus + ic });
            }
        }

        self.solver.push_initial_conditions(iv_dc.clone());
        self.solver.push_initial_conditions(iv_dc);
        if !device_ic.is_empty() {
            self.solver.push_initial_conditions(device_ic.clone());
            self.solver.push_initial_conditions(device_ic);
        }
        // User `ic` seeds the t=0 state. Pushed twice so both rows of the
        // companion's history buffer see the ic values (avoids a
        // discontinuity that would spike the first transient step) —
        // milestone-1 seed; full enforced-hold is deferred.
        if !self.initial_conditions.is_empty() {
            self.solver.push_initial_conditions(self.initial_conditions.clone());
            self.solver.push_initial_conditions(self.initial_conditions.clone());
        }

        Ok(self.snapshot(self.options.start_time))
    }

    /// Advance one TR-BDF2 step over `[t_n, t_n + dt]`. Two Newton solves:
    /// phase 1 (Trapezoidal over `γ·dt`) → `x_{n+γ}`; phase 2 (BDF2 over
    /// `(1−γ)·dt`) → `x_{n+1}`, warm-started from `x_{n+γ}`. Either phase
    /// failing rejects the whole step (TRB-05). The Newton buffer's push
    /// semantics line the history up naturally: phase 1 reads `x_n` at
    /// `view(1)`; phase 2 reads `x_{n+γ}` at `view(1)` and `x_n` at `view(2)`.
    fn execute_timestep(
        &mut self,
        t_n: f64,
        dt: f64,
        use_predictor: bool,
    ) -> crate::result::Result<Option<TransientStep>> {
        let strategy = crate::analyses::convergence::DampedNewton;
        let policy = self.policy.clone();
        let tolerances = self.system.context.tolerances;
        let t_next = t_n + dt;

        // Phase 1 — Trapezoidal over γ·dt → intermediate point x_{n+γ}.
        self.system.phase = TrBdf2Phase::Trapezoidal;
        self.system.h = dt;
        self.system.time = t_n + TrBdf2::GAMMA * dt;
        // First-order predictor seed (CP-16): extrapolate the two newest
        // accepted rows — x_n at view(0), x_{n−1+γ} at view(1), separated by
        // (1−γ)·prev_h — forward γ·dt to the TR-stage target. Only when the
        // previous step was accepted and didn't land on a breakpoint
        // (prev_h > 0); phase 2 warm-starts from x_{n+γ} and needs none.
        if use_predictor && self.system.prev_h > 0.0 {
            let r = TrBdf2::GAMMA * dt / ((1.0 - TrBdf2::GAMMA) * self.system.prev_h);
            self.solver.set_predictor_ratio(r);
        }
        self.solver.solve_with_strategy(
            &mut self.system, &strategy, &tolerances, &policy,
        )?;

        // Phase 2 — BDF2 over (1−γ)·dt → final point x_{n+1} (warm-start x_{n+γ}).
        self.system.phase = TrBdf2Phase::Bdf2;
        self.system.h = dt;
        self.system.time = t_next;
        self.solver.solve_with_strategy(
            &mut self.system, &strategy, &tolerances, &policy,
        )?;

        // `prev_h` is set by the caller once the global-LTE accept gate passes.
        Ok(Some(self.snapshot(t_next)))
    }

    pub fn solve(&mut self) -> crate::result::Result<TransientAnalysisResult> {
        let stop_time: f64 = self.options.stop_time;
        let start_time: f64 = self.options.start_time;
        let record_from: f64 = self.options.record_from;
        let mut dt: f64 = self.options.dt;
        let dt_min = self.options.dt_min;

        // Sets scheduled at or before the start time apply before the
        // initial operating point — equivalent to an idle set before the
        // run (LIVE-08); the whole run sees the new values, no breakpoint.
        for set in self.sets.drain_due(start_time) {
            self.system.circuit.set_element_param(&set.label, &set.param, set.value)?;
        }

        let initial_snapshot = self.compute_initial_conditions()?;
        let mut steps = Vec::new();
        // The start-time operating point is only part of the recorded output
        // when recording starts at (or before) it; a delayed `record_from`
        // drops it but still computes it — the initial state seeds the
        // integration.
        if start_time >= record_from {
            steps.push(initial_snapshot);
        }

        // Seed runtime operators at the operating point (t = start_time) so
        // history-based operators anchor on the quiescent solution. Without
        // this a `delay(x, td)` returns the first *stepped* sample for
        // `t < td` instead of the op value — a spurious pre-arrival leak on a
        // transmission line. The op point is not a stepped advance, so this
        // only records history; it does not integrate anything.
        if let Some(op) = self.solver.current_guess().map(|g| g.to_owned()) {
            self.system
                .circuit
                .accept_and_run_digital(op.as_slice().unwrap(), start_time)?;
            self.system.circuit.digital_state.commit();
        }

        let mut current_time = start_time;
        self.solver.reset_iteration_counter();
        let mut steps_accepted: usize = 0;
        let mut steps_rejected: usize = 0;
        let mut dt_min_floor_hits: usize = 0;
        // Predictor gate: extrapolation history is only valid coming off an
        // accepted step (a rejection leaves rejected rows in the buffer).
        let mut last_step_accepted = false;
        // Live-set edge rule (LIVE-06/07): the first step after a scheduled
        // set integrates the value jump, so its Milne window spans the
        // discontinuity — the LTE there is not error but the jump itself.
        // That one step is exempt from the accept gate and from the PI
        // update (dt held), exactly like landing on a declared breakpoint.
        let mut sets_just_applied = false;
        let mut dt_min_seen = f64::INFINITY;
        let mut dt_max_seen = 0.0_f64;

        // The Milne accept gate reads only node-voltage unknowns; the netlist
        // is structurally stable for the whole run, so build the index list
        // once instead of per accepted step.
        let node_indices: Vec<usize> = self
            .system
            .circuit
            .netlist()
            .all_references()
            .iter()
            .filter(|r| !r.is_branch())
            .filter_map(|r| r.idx())
            .collect();

        while current_time < stop_time {
            let dt_proposed = dt;

            // Unified breakpoint table (TRB-11): the integrator lands on the
            // nearest of (a) the PI-proposed step, (b) the next DIGITAL event
            // — digital-var/enum `if`s in analog bodies switch at these times,
            // so landing here covers them — and (c) ANALOG `@timer` fires /
            // source edges declared via `Element::next_breakpoints`. Absolute
            // times → survive rollback.
            //
            // A step that lands on a declared discontinuity is ACCEPTED without
            // the Milne-LTE gate: the LTE would otherwise see the intentional
            // source jump (e.g. V(in) 0→5 at a pulse edge) as a huge error and
            // reject, thrashing the integrator against the edge it already hit.
            let t_next_event = self.system.circuit.digital_state.peek_next_event_time();
            let pi_target = current_time + dt_proposed;
            let mut t_next = pi_target.min(stop_time);
            let mut landed_on_breakpoint = false;
            if t_next_event < t_next {
                t_next = t_next_event;
                landed_on_breakpoint = true;
            }
            for dev in self.system.circuit.devices.iter() {
                for bp in dev.next_breakpoints(current_time, dt_proposed) {
                    if bp > current_time && bp < t_next {
                        t_next = bp;
                        landed_on_breakpoint = true;
                    }
                }
            }
            // Scheduled live sets (LIVE-06): each pending set time is a
            // declared discontinuity — land exactly on it so the write
            // applies at its scheduled time with the edge rules (skip LTE,
            // reset prev_h). The relative-epsilon snap absorbs float
            // accumulation: a proposal one ulp shy of the set time
            // stretches onto it instead of leaving a ~1e-22 s sliver step.
            if let Some(ts) = self.sets.next_breakpoint(current_time) {
                let snap = 1e-9 * ts.abs().max(f64::MIN_POSITIVE);
                if ts <= t_next + snap {
                    t_next = ts;
                    landed_on_breakpoint = true;
                }
            }

            let dt_actual = t_next - current_time;

            // Checkpoint digital state
            self.system.circuit.digital_state.checkpoint();

            // Checkpoint the analog history: a rejected attempt leaves its
            // rows in the Newton buffer, and the retry's charge-history
            // views would integrate off the rejected trajectory.
            let analog_history = self.solver.state_snapshot();

            // Process digital events EXACTLY at t_next BEFORE analog solve.
            self.system.circuit.run_digital_at(t_next)?;

            // Solve the TR-BDF2 step [current_time, t_next] (two phases).
            let analog_result =
                self.execute_timestep(current_time, dt_actual, last_step_accepted);

            // Whether this step's Milne window spans a live-set value jump
            // (consumed here; re-armed below when new sets apply).
            let post_set_step = sets_just_applied;

            if let Ok(Some(snapshot)) = analog_result {                // Both Newton phases converged. Global Milne-LTE accept gate
                // (TRB-05/06): the two-phase buffer holds x_{n+1} (view 0),
                // x_{n+γ} (view 1), x_n (view 2). The Milne predictor is
                // evaluated only over **node-voltage** unknowns — branch
                // currents are KCL-derived (their accuracy follows the node
                // voltages) and the `/γ` extrapolation falsely amplifies a
                // source branch's startup jump.
                let tolerances = self.system.context.tolerances;
                let milne = match (self.solver.state().view(0), self.solver.state().view(1), self.solver.state().view(2)) {
                    (Some(a), Some(b), Some(c)) => TrBdf2::milne_lte_indexed(
                        c.as_slice().unwrap(),
                        b.as_slice().unwrap(),
                        a.as_slice().unwrap(),
                        &node_indices,
                        tolerances.reltol,
                        tolerances.vntol,
                    ),
                    _ => 0.0,
                };
                if !landed_on_breakpoint && !post_set_step && milne > tolerances.trtol {
                    // LTE too large: reject, halve dt, reset the PI memory.
                    if self.policy.trace.transient {
                        eprintln!("REJECT t={current_time:.3e} dt={dt_actual:.3e} milne={milne:.3e} (trtol={})", tolerances.trtol);
                    }
                    self.system.circuit.digital_state.rollback();
                    dt = self.stepper.reject_dt(dt_proposed, &self.options);
                    if dt <= self.options.dt_min {
                        // Can't shrink further — accept the step as-is rather
                        // than stall. Surface the accuracy concession (audit C2).
                        dt_min_floor_hits += 1;
                        tracing::warn!(
                            "transient LTE exceeded trtol at dt_min ({:.3e}); \
                             accepting step at t={:.3e} with reduced accuracy",
                            dt, current_time
                        );
                    } else {
                        steps_rejected += 1;
                        last_step_accepted = false;
                        self.solver.restore_state(analog_history);
                        continue;
                    }
                }
                // Accept.
                steps_accepted += 1;
                last_step_accepted = true;
                // UIC clamps release after the first accepted step (CKTsetIC).
                self.system.uic_hold = false;
                dt_min_seen = dt_min_seen.min(dt_actual);
                dt_max_seen = dt_max_seen.max(dt_actual);
                let solution = self.solver.current_guess().unwrap().to_owned();
                let _changed = self
                    .system
                    .circuit
                    .accept_and_run_digital(solution.as_slice().unwrap(), t_next)?;
                self.system.circuit.digital_state.commit();
                if t_next >= record_from {
                    // Runtime banks committed by `accept_and_run_digital`
                    // (idt/operator state) post-date the in-step snapshot —
                    // re-attach so the recorded state matches this point.
                    let snapshot = if self.options.record_device_state {
                        snapshot.with_device_state(self.collect_device_banks())
                    } else {
                        snapshot
                    };
                    steps.push(snapshot);
                }
                current_time = t_next;
                // Apply scheduled live sets due at this accepted point
                // (LIVE-06/09): scheduling order = last-write-wins per
                // param. The new values take effect from the next accepted
                // step; a write of ≥ OperatingPoint strength additionally
                // re-solves the just-closed step with the new values so the
                // point at t is the post-set consistent solution. Rebuild
                // is beyond the solver (no POM here) — fail loud.
                let due = self.sets.drain_due(current_time);
                sets_just_applied = !due.is_empty();
                if !due.is_empty() {
                    use crate::core::introspect::Invalidation;
                    let mut strongest = Invalidation::None;
                    for set in due {
                        let inv = self.system.circuit.set_element_param(
                            &set.label, &set.param, set.value,
                        )?;
                        strongest = strongest.max(inv);
                    }
                    if strongest >= Invalidation::Rebuild {
                        return Err(crate::error::Error::simple(
                            crate::error::SolverDomain::Transient,
                            format!(
                                "scheduled set at t={current_time:.3e} needs a structural \
                                 rebuild — re-elaborate at the host layer (MD-18)"
                            ),
                        ));
                    }
                    if strongest >= Invalidation::OperatingPoint
                        && let Some(re) =
                            self.execute_timestep(current_time - dt_actual, dt_actual, false)?
                        && current_time >= record_from
                        && let Some(last) = steps.last_mut()
                    {
                        *last = re;
                    }
                    // The value jump is a discontinuity: the next TR stage
                    // must not re-derive a previous current across it.
                    landed_on_breakpoint = true;
                }
                // This step's size seeds the next step's TR-stage
                // previous-current re-derivation — UNLESS the step landed on a
                // declared discontinuity, in which case the history spans a
                // jump (e.g. a source edge) and must not feed the next TR
                // stage; reset so the next step starts clean (i_{C,n} = 0).
                self.system.prev_h = if landed_on_breakpoint { 0.0 } else { dt_actual };
                // Timestep policy: the PI controller grows / shrinks `dt`
                // from the global Milne error (always adaptive — SPICE has
                // been adaptive since v2). Output interpolation onto a fixed
                // print grid is a follow-up (ROADMAP); the recorded waveform
                // is the adaptive time grid for now, and statistics
                // weight by `dt` so they stay correct.
                dt = if post_set_step {
                    // The Milne value measures the jump, not integration
                    // error — hold dt instead of feeding the PI garbage.
                    dt_proposed
                } else {
                    self.stepper.propose_dt(milne, dt_actual, &self.options)
                };
                if sets_just_applied {
                    // Discontinuity restart (SPICE breakpoint convention):
                    // the step after a live-set jump starts first-order
                    // (prev_h = 0 discards the pre-jump current), so resume
                    // with a small step and let the PI regrow from clean
                    // LTE readings.
                    dt = (1e-3 * dt_actual).max(self.options.dt_min);
                }

                // Per-device LTE floor: reactive devices can cap dt tighter
                // than the global Milne LTE (audit P5 — this was never called).
                let tran_state = TransientAnalysisState::new(
                    self.solver.state(),
                    &self.system.circuit.digital_state.nets,
                );
                let time_history = [self.system.prev_h, dt_actual];
                for dev in &self.system.circuit.devices {
                    if let Some(dt_floor) = dev.suggest_transient_step(
                        &tran_state,
                        &time_history,
                        &self.system.context,
                    ) {
                        dt = dt.min(dt_floor);
                    }
                }
            } else {
                // Either phase failed to converge — reject the whole step,
                // halve dt, reset the PI memory (TRB-05/09).
                steps_rejected += 1;
                last_step_accepted = false;
                self.system.circuit.digital_state.rollback();
                self.solver.restore_state(analog_history);
                dt = self.stepper.reject_dt(dt_proposed, &self.options);

                if dt <= dt_min && let Err(e) = analog_result {
                    return Err(e);
                }
                continue;
            }
        }

        let mut result = TransientAnalysisResult::new(steps);
        result.stats.newton_iterations = self.solver.total_iterations();
        result.stats.converged = true;
        result.stats.steps_accepted = steps_accepted;
        result.stats.steps_rejected = steps_rejected;
        result.stats.dt_min_floor_hits = dt_min_floor_hits;
        result.stats.dt_min = if dt_min_seen.is_finite() { dt_min_seen } else { 0.0 };
        result.stats.dt_max = dt_max_seen;
        result.stats.assembly_time_ns = self.solver.assembly_time_ns();
        result.stats.solve_time_ns = self.solver.solve_time_ns();
        Ok(result)
    }

    fn snapshot(&self, time: f64) -> TransientStep {
        let mut values = HashMap::new();
        let netlist = self.system.circuit.netlist();
        let latest_state = self.solver.current_guess().unwrap();

        for reference in netlist.all_references() {
            if let Some(idx) = reference.idx() {
                values.insert(reference.variable().clone(), latest_state[idx]);
            }
        }

        let step = TransientStep::new(time, values)
            .with_digital(self.system.circuit.digital_state.nets.clone())
            .with_digital_hidden(self.collect_digital_hidden());
        if !self.options.record_device_state {
            return step;
        }
        step.with_device_state(self.collect_device_banks())
    }

    /// Snapshot each digital device's hidden state (module vars + edge
    /// memory) — the register half of the full-state contract, always
    /// recorded so any step can seed a re-entry.
    fn collect_digital_hidden(&self) -> HashMap<String, (Vec<i64>, Vec<f64>)> {
        let mut hidden = HashMap::new();
        for dev in &self.system.circuit.devices {
            if let Some(state) = dev.digital_hidden_snapshot() {
                hidden.insert(dev.name().to_string(), state);
            }
        }
        hidden
    }

    /// Clone each stateful device's runtime banks (opt-in recording; see
    /// `TransientAnalysisOptions::record_device_state`).
    fn collect_device_banks(&self) -> HashMap<String, (Vec<f64>, Vec<f64>)> {
        let mut device_state = HashMap::new();
        for dev in &self.system.circuit.devices {
            let (state, vars) = dev.runtime_banks();
            if !state.is_empty() || !vars.is_empty() {
                device_state.insert(dev.name().to_string(), (state.to_vec(), vars.to_vec()));
            }
        }
        device_state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::introspect::Value;

    fn set(t: f64, param: &str, v: f64) -> ScheduledSet {
        ScheduledSet { t, label: "r1".into(), param: param.into(), value: Value::Real(v) }
    }

    #[test]
    fn next_breakpoint_is_the_earliest_pending_time_strictly_after_from() {
        let mut q = SetQueue::default();
        q.push(set(5e-6, "r", 1.0));
        q.push(set(3e-6, "r", 2.0));
        q.push(set(8e-6, "r", 3.0));
        assert_eq!(q.next_breakpoint(0.0), Some(3e-6));
        assert_eq!(q.next_breakpoint(3e-6), Some(5e-6), "strictly after `from`");
        assert_eq!(q.next_breakpoint(8e-6), None);
    }

    #[test]
    fn drain_preserves_scheduling_order_for_last_write_wins() {
        let mut q = SetQueue::default();
        q.push(set(5e-6, "r", 3000.0));
        q.push(set(5e-6, "r", 1000.0));
        let due = q.drain_due(5e-6);
        // Application order is scheduling order: the later push lands last,
        // so the element ends at 1000 — last write wins.
        assert_eq!(due.len(), 2);
        assert_eq!(due[0].value, Value::Real(3000.0));
        assert_eq!(due[1].value, Value::Real(1000.0));
        assert!(q.next_breakpoint(0.0).is_none(), "queue drained");
    }

    #[test]
    fn drain_takes_only_due_entries_and_keeps_the_rest_pending() {
        let mut q = SetQueue::default();
        q.push(set(5e-6, "r", 1.0));
        q.push(set(2e-6, "c", 2.0));
        q.push(set(9e-6, "r", 3.0));
        let due = q.drain_due(5e-6);
        assert_eq!(due.iter().map(|s| s.t).collect::<Vec<_>>(), vec![5e-6, 2e-6]);
        assert_eq!(q.next_breakpoint(0.0), Some(9e-6), "later entry stays pending");
    }
}
