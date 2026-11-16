//! Transient (`.tran`) — the options, the UIC hold clamps (ngspice `CKTsetIC`),
//! and the adaptive TR-BDF2 driver over the device assembly: Milne-LTE accept
//! gate, PI timestep control, and live scheduled parameter sets, the time loop
//! itself run by the owner protocol.
//!
//! There is no per-analysis `TransientContext`: MD-03 is superseded
//! (p6-cleanup-architecture D2). The step tunables stay on
//! [`TransientAnalysisOptions`], and the shared
//! [`Context`](crate::analyses::Context) carries what every analysis shares.
use piperine_analog_solver::crossing::{CrossDir, predict_crossing};
use piperine_analog_solver::device::{Ambient, AssemblySnapshot, Stage};
use piperine_analog_solver::netlist::AnalogReference;
use crate::analyses::dc::DcSolver;
use crate::analyses::Context;
use crate::core::circuit::CircuitInstance;
use piperine_analog_solver::math::circular_array::CircularArrayBuffer2;
use piperine_analog_solver::math::faer::FaerSparseLinearSystem;
use piperine_analog_solver::math::integration::{TrBdf2, TrBdf2Phase};
use piperine_analog_solver::math::iv::InitialValue;
use piperine_analog_solver::math::linear::{AsIndex, Stamp};
use piperine_analog_solver::math::newton_raphson::{NewtonRaphsonSolver, NonLinearSystem};
use crate::prelude::{TransientAnalysisResult, TransientStep};

use ndarray::ArrayView1;
use std::collections::HashMap;

// ── request/state ────────────────────────────────────────────────────────

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
    /// termination guarantee: an LTE rejection at the `dt_min` floor is
    /// *accepted* (Part VII §16 §10), so a circuit that pins the floor keeps
    /// advancing by `dt_min` per iteration — 1e-15 s at a time — recording a
    /// snapshot for each. That is a hang, not an analysis.
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
    /// keyed by device label on each [`TransientStep`]. Off by default —
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
    pub fn with_probe_selection(
        mut self,
        selection: crate::core::introspect::ProbeSelection,
    ) -> Self {
        self.probe_selection = selection;
        self
    }
}

#[cfg(test)]
mod step_tests {
    use super::*;
    use crate::core::net::Net;
    use piperine_analog_solver::netlist::{AnalogReference, AnalogVariable, NodeIdentifier};
    use std::sync::Arc;

    #[test]
    fn transient_step_lookup_by_net_returns_analog_and_digital_values() {
        let var: Arc<AnalogVariable> = Arc::new(AnalogVariable::Node(NodeIdentifier::Anonymous(0)));
        let mut values = HashMap::new();
        values.insert(var.clone(), 1.25);
        let names: Arc<[String]> = vec!["d0".to_string(), "top.clk".to_string()].into();
        let digital = crate::core::result::DigitalNets::new(names, vec![vec![1, 1], vec![0, 1]]);
        let step = TransientStep::new(0.0, values).with_digital(digital);

        let analog_net: Net = (&AnalogReference::new(var.clone(), 0)).into();
        assert_eq!(step.get_net(&analog_net), Some(1.25));

        let digital_net = Net::digital(1, "top.clk");
        assert_eq!(step.digital_net(&digital_net), Some(&[0u8, 1][..]));
        assert_eq!(step.digital_net(&Net::digital(0, "d0")), Some(&[1u8, 1][..]));
        assert_eq!(step.digital_named("top.clk"), Some(&[0u8, 1][..]));
        // A digital net carries no analog value.
        assert_eq!(step.get_net(&digital_net), None);

        // Wrong kind returns None — analog_net is not a digital net.
        assert_eq!(step.digital_net(&analog_net), None);

        // Digital net past the recorded snapshot returns None.
        assert_eq!(step.digital_net(&Net::digital(99, "x")), None);
        assert_eq!(step.digital_named("x"), None);
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
    /// An error a Newton callback met (they return nothing): the driver
    /// takes it after the solve that raised it.
    callback_error: Option<piperine_analog_solver::error::Error>,
}

impl<'a> NonLinearSystem<AnalogReference, f64> for TransientSystem<'a> {
    fn assemble(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
    ) -> crate::core::result::Result<Vec<Stamp<AnalogReference, f64>>> {
        self.assemble_device(state)
    }

    fn netlist(&self) -> &piperine_analog_solver::netlist::Netlist {
        self.circuit.netlist()
    }

    fn any_limiting_report(&self) -> bool {
        self.circuit.abi.as_ref().is_some_and(|bound| !bound.analog.limiting_active().is_empty())
    }

    /// Every Newton iteration but the first ends here (abi.md A2).
    fn before_iter_callback(&mut self, _state: &CircularArrayBuffer2<f64>, iteration_number: usize) {
        if iteration_number > 0
            && let Some(bound) = self.circuit.abi.as_mut()
        {
            bound.analog.end_iteration();
        }
    }

    fn update_sources(&mut self, _state: &mut CircularArrayBuffer2<f64>) {}

    /// A converged stage gains a row of the Newton buffer: the assembly's term
    /// histories gain theirs, at the same point and in the same call.
    fn convergence_success_callback(
        &mut self,
        _state: &CircularArrayBuffer2<f64>,
        converged: &ArrayView1<f64>,
    ) {
        let amb = self.ambient();
        if let Some(bound) = self.circuit.abi.as_mut() {
            bound.analog.end_iteration();
            if let Err(e) = bound.analog.push_stage(&converged.to_vec(), amb) {
                self.callback_error.get_or_insert(e);
            }
        }
    }
}

impl TransientSystem<'_> {
    /// The coordinates of the stage being solved (abi.md §4.2).
    fn ambient(&self) -> Ambient {
        Ambient {
            time: self.time,
            freq: 0.0,
            is_static: false,
            gmin: self.context.tolerances.gmin,
            source_scale: 1.0,
        }
    }

    /// The stamps of one stage of a circuit of devices: `FLOW` terms in their
    /// Norton form, `DDT` terms through the TR-BDF2 companion over their own
    /// history, `DELAY` terms from theirs, then the solver's shunts and clamps.
    fn assemble_device(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
    ) -> crate::core::result::Result<Vec<Stamp<AnalogReference, f64>>> {
        let x: Vec<f64> = match state.latest() {
            Some(row) => row.to_vec(),
            None => vec![0.0; state.size()],
        };
        let stage = Stage { phase: self.phase, h: self.h, prev_h: self.prev_h };
        let amb = self.ambient();
        let bound = self.circuit.abi.as_mut().expect("a circuit of devices");
        let mut all_stamps = bound.analog.assemble_tran(&x, amb, stage)?;
        if let Some(err) = piperine_analog_solver::stamp_validation::non_finite_stamp_error("devices", &all_stamps) {
            return Err(err);
        }
        let gshunt = self.context.tolerances.gshunt;
        if gshunt > 0.0 {
            for r in self.circuit.netlist().all_references() {
                if r.variable().is_node() && !r.variable().is_ground() {
                    all_stamps.push(Stamp::Matrix(r.clone(), r.clone(), gshunt));
                }
            }
        }
        if self.uic_hold {
            for clamp in &self.uic_clamps {
                clamp.stamp(&mut all_stamps);
            }
        }
        Ok(all_stamps)
    }

    /// The error a callback met during the solve just run, if any.
    fn take_callback_error(&mut self) -> crate::core::result::Result<()> {
        self.callback_error.take().map_or(Ok(()), Err)
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

/// Phase 1 output — the candidate landing point chosen over the unified
/// breakpoint table from the PI-proposed step size.
struct StepPrediction {
    /// Absolute landing time.
    t_next: f64,
    /// The step size actually taken: `t_next − t_n`.
    dt_actual: f64,
    /// The step size the PI controller proposed (before breakpoint clamping).
    dt_proposed: f64,
    /// The step lands on a declared discontinuity → exempt from the
    /// Milne-LTE accept gate.
    landed_on_breakpoint: bool,
}

/// Phase 3 output — the Milne-LTE accept-gate verdict for a converged
/// candidate step.
struct StepAssessment {
    /// The global Milne LTE over node-voltage unknowns (feeds the PI
    /// controller on accept).
    milne: f64,
    /// Gate verdict: accept the candidate step.
    accept: bool,
}

/// The adaptive time loop's mutable state: the clock, the current step
/// size, and the acceptance bookkeeping the phase methods read and
/// advance. Owned by [`TransientSolver::solve`]; the phase methods take it
/// as an explicit parameter.
struct TimeLoop {
    current_time: f64,
    dt: f64,
    steps_accepted: usize,
    steps_rejected: usize,
    dt_min_floor_hits: usize,
    /// Predictor gate: extrapolation history is only valid coming off an
    /// accepted step (a rejection leaves rejected rows in the buffer).
    last_step_accepted: bool,
    /// Live-set edge rule (LIVE-06/07): the first step after a scheduled
    /// set integrates the value jump, so its Milne window spans the
    /// discontinuity — the LTE there is not error but the jump itself.
    /// That one step is exempt from the accept gate and from the PI
    /// update (dt held), exactly like landing on a declared breakpoint.
    sets_just_applied: bool,
    dt_min_seen: f64,
    dt_max_seen: f64,
}

impl TimeLoop {
    fn new(start_time: f64, dt: f64) -> Self {
        Self {
            current_time: start_time,
            dt,
            steps_accepted: 0,
            steps_rejected: 0,
            dt_min_floor_hits: 0,
            last_step_accepted: false,
            sets_just_applied: false,
            dt_min_seen: f64::INFINITY,
            dt_max_seen: 0.0,
        }
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
    /// Convergence plan owning the timestep strategy (ABI-42). The transient
    /// driver delegates `propose_dt`/`reject_dt` to `plan.stepper()` — the
    /// plan is the single strategy owner across analyses (Newton +
    /// Homotopy + Stepper).
    plan: piperine_analog_solver::convergence::ConvergencePlan,
    /// Convergence tunables for this analysis (MD-04). Defaults on
    /// construction; hosts override before [`solve`](Self::solve).
    pub policy: piperine_analog_solver::tolerances::Policy,
    /// Host-scheduled live parameter writes (LIVE-06/09), in scheduling order:
    /// last write wins per param.
    sets: Vec<ScheduledSet>,
    /// Full-state re-entry point (PSS shooting enabler): when set, the run
    /// starts from this captured step instead of a DC operating point.
    reentry_state: Option<TransientStep>,
    /// The host's sink and waiters, for a circuit of devices.
    pub(crate) host: crate::analyses::adapters::HostHooks,
}

impl<'a> TransientSolver<'a> {
    pub fn new(
        circuit: &'a mut CircuitInstance,
        options: TransientAnalysisOptions,
        context: Context,
    ) -> crate::core::result::Result<Self> {
        Self::with_plan(circuit, options, context, piperine_analog_solver::convergence::ConvergencePlan::default())
    }

    /// Build a transient solver with an explicit convergence plan (ABI-42):
    /// the plan's stepper owns the timestep strategy; the driver delegates
    /// `propose_dt`/`reject_dt` to it.
    pub fn with_plan(
        circuit: &'a mut CircuitInstance,
        options: TransientAnalysisOptions,
        context: Context,
        plan: piperine_analog_solver::convergence::ConvergencePlan,
    ) -> crate::core::result::Result<Self> {
        Context::init_global();
        circuit.setup_all(&context)?;
        Self::validate_probe_selection(circuit, &options.probe_selection)?;

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
            callback_error: None,
        };
        if let Some(bound) = &system.circuit.abi {
            let mut tolerances = system.context.tolerances.clone();
            bound.tighten(&mut tolerances);
            system.context.tolerances = tolerances;
        }

        let solver = NewtonRaphsonSolver::new(&mut system, size, 4)?;

        Ok(Self {
            system,
            solver,
            options,
            initial_conditions: Vec::new(),
            plan,
            policy: piperine_analog_solver::tolerances::Policy::default(),
            sets: Vec::new(),
            reentry_state: None,
            host: Default::default(),
        })
    }

    /// Replace the convergence plan (ABI-42). The host plugs in a plan with a
    /// custom stepper; the driver delegates rejection/proposal to it.
    pub fn set_plan(&mut self, plan: piperine_analog_solver::convergence::ConvergencePlan) {
        self.plan = plan;
    }

    /// Validate the `ProbeSelection` against the assembled circuit at setup
    /// time (ABI-35). Each `(instance path, observable name)` request must
    /// name an instance that exists and a term or `VAR` watch of its shape —
    /// otherwise fail loud with a named error before the run starts, not
    /// silently record nothing.
    fn validate_probe_selection(
        circuit: &CircuitInstance,
        selection: &crate::core::introspect::ProbeSelection,
    ) -> crate::core::result::Result<()> {
        let bound = circuit.abi.as_ref().expect("a circuit of devices");
        for (label, observable) in &selection.requests {
            let Some((terms, vars)) = bound.analog.observable_names(label) else {
                return Err(piperine_analog_solver::error::Error::simple(
                    piperine_analog_solver::error::SolverDomain::Device,
                    format!("device `{label}` not found"),
                ));
            };
            if !terms.contains(&observable.as_str()) && !vars.contains(&observable.as_str()) {
                return Err(piperine_analog_solver::error::Error::simple(
                    piperine_analog_solver::error::SolverDomain::Device,
                    format!("device `{label}` has no observable `{observable}`"),
                ));
            }
        }
        Ok(())
    }

    /// Read-only access to the convergence plan (e.g., for tests asserting
    /// which strategy is wired).
    pub fn plan(&self) -> &piperine_analog_solver::convergence::ConvergencePlan {
        &self.plan
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
    /// accepted step (the landing is solved on the value before the write).
    /// Several sets on the same param apply in scheduling order — last write
    /// wins.
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

    /// Pushes one row of initial values onto the Newton history and, for a
    /// circuit of devices, the same row onto every `DDT` term's history, so
    /// the two stay aligned (the equilibrium rows count as `STATIC`).
    fn push_initial(&mut self, ivs: Vec<InitialValue<AnalogReference, f64>>) -> crate::core::result::Result<()> {
        self.solver.push_initial_conditions(ivs);
        if self.system.circuit.abi.is_none() {
            return Ok(());
        }
        let x = self.solver.current_guess().map(|g| g.to_vec()).unwrap_or_default();
        let amb = Ambient { time: self.options.start_time, is_static: true, ..self.system.ambient() };
        if let Some(bound) = self.system.circuit.abi.as_mut() {
            bound.analog.push_stage(&x, amb)?;
        }
        Ok(())
    }

    /// The `(plus, minus, ic)` seeds of the circuit: a continuous pin's `init`
    /// (abi.md §3.1).
    fn initial_seeds(&self) -> Vec<(Option<AnalogReference>, Option<AnalogReference>, f64)> {
        let bound = self.system.circuit.abi.as_ref().expect("a circuit of devices");
        bound.holds.iter().map(|h| (Some(h.reference.clone()), None, h.value)).collect()
    }

    /// `@initial { V(p,n) <- ic }` UIC seeds: the t=0 branch
    /// voltage `v(plus) = v(minus) + ic` (cap/ind/dio initial condition,
    /// SPICE `.ic`), overlaid on the DC point so unconstrained nodes keep
    /// their operating-point values.
    fn device_initial_conditions(
        &self,
        iv_dc: &[InitialValue<AnalogReference, f64>],
    ) -> Vec<InitialValue<AnalogReference, f64>> {
        let mut device_ic: Vec<InitialValue<AnalogReference, f64>> = Vec::new();
        for (plus, minus, ic) in self.initial_seeds() {
            let Some(plus_ref) = plus else { continue };
            let v_minus = minus
                .as_ref()
                .and_then(|m| m.as_index())
                .map_or(0.0, |i| iv_dc.iter().find(|iv| iv.reference.as_index() == Some(i)).map_or(0.0, |iv| iv.value));
            device_ic.push(InitialValue { reference: plus_ref, value: v_minus + ic });
        }
        device_ic
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
    ) -> crate::core::result::Result<Option<TransientStep>> {
        let strategy = piperine_analog_solver::convergence::DampedNewton;
        let policy = self.policy.clone();
        let tolerances = self.system.context.tolerances.clone();
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
        self.system.take_callback_error()?;

        // Phase 2 — BDF2 over (1−γ)·dt → final point x_{n+1} (warm-start x_{n+γ}).
        self.system.phase = TrBdf2Phase::Bdf2;
        self.system.h = dt;
        self.system.time = t_next;
        self.solver.solve_with_strategy(
            &mut self.system, &strategy, &tolerances, &policy,
        )?;
        self.system.take_callback_error()?;

        // `prev_h` is set by the caller once the global-LTE accept gate passes.
        Ok(Some(self.snapshot(t_next)))
    }

    /// Phase 3 — assess: the global Milne-LTE accept gate (TRB-05/06). The
    /// two-phase buffer holds x_{n+1} (view 0), x_{n+γ} (view 1), x_n
    /// (view 2). The Milne predictor is evaluated only over
    /// **node-voltage** unknowns — branch currents are KCL-derived (their
    /// accuracy follows the node voltages) and the `/γ` extrapolation
    /// falsely amplifies a source branch's startup jump. Steps landing on
    /// a declared discontinuity (breakpoint or live-set edge) are exempt.
    fn assess_step(
        &self,
        prediction: &StepPrediction,
        post_set_step: bool,
        node_indices: &[usize],
    ) -> StepAssessment {
        let tolerances = self.system.context.tolerances.clone();
        let milne = match (self.solver.state().view(0), self.solver.state().view(1), self.solver.state().view(2)) {
            (Some(a), Some(b), Some(c)) => TrBdf2::milne_lte_indexed(
                c.as_slice().unwrap(),
                b.as_slice().unwrap(),
                a.as_slice().unwrap(),
                node_indices,
                tolerances.reltol,
                tolerances.vntol,
            ),
            _ => 0.0,
        };
        let reject =
            !prediction.landed_on_breakpoint && !post_set_step && milne > tolerances.trtol;
        StepAssessment { milne, accept: !reject }
    }

    /// Record an accepted step's snapshot when it falls inside the
    /// `record_from` window. Runtime banks committed by the digital settle
    /// (idt/operator state) post-date the in-step snapshot — re-attach so
    /// the recorded state matches this point. Device-state recording is
    /// gated by either `record_device_state` (full-bank shorthand) or a
    /// non-empty `probe_selection` (selective recording — ABI-34).
    fn record_step(&self, steps: &mut Vec<TransientStep>, t_next: f64, snapshot: TransientStep) {
        if t_next >= self.options.record_from {
            let snapshot = if self.options.record_device_state
                || !self.options.probe_selection.requests.is_empty()
            {
                snapshot.with_device_state(self.collect_device_banks())
            } else {
                snapshot
            };
            steps.push(snapshot);
        }
    }

    /// The next step-size proposal after an accepted step. The PI
    /// controller grows/shrinks `dt` from the global Milne error (always
    /// adaptive — SPICE has been adaptive since v2); across a live-set
    /// edge the Milne value measures the jump, not integration error, so
    /// dt is held; when a set just applied the run restarts small (SPICE
    /// breakpoint convention — the next TR stage starts first-order with
    /// `prev_h = 0`) and the PI regrows from clean LTE readings; finally
    /// any reactive device's own LTE floor caps the proposal (audit P5).
    /// Output interpolation onto a fixed print grid is a follow-up
    /// (ROADMAP): the recorded waveform is the adaptive time grid, and
    /// statistics weight by `dt` so they stay correct.
    fn propose_dt(
        &mut self,
        milne: f64,
        dt_actual: f64,
        dt_proposed: f64,
        post_set_step: bool,
        sets_just_applied: bool,
    ) -> f64 {
        let mut dt = if post_set_step {
            dt_proposed
        } else {
            // NOTE: the controller targets `lte == 1` while the accept gate
            // rejects only at `milne > trtol` (default 7). That gap is
            // deliberate, not a bug: the controller aims ~trtol deeper into the
            // accept region than the gate demands, so rejections stay rare.
            // Normalising `milne` by `trtol` here (tried, reverted) closes the
            // safety band and coarsens the grid enough to break `tran_rc`'s
            // ngspice parity at the steep part of the charging curve.
            self.plan.stepper_mut().propose_dt(milne, dt_actual, &self.options.step_limits())
        };
        if sets_just_applied {
            dt = (1e-3 * dt_actual).max(self.options.dt_min);
        }
        dt
    }

    /// Assemble the run result: the recorded steps plus the statistics
    /// accumulated by the Newton solver and the time loop.
    fn finish_run(&self, st: &TimeLoop, steps: Vec<TransientStep>) -> TransientAnalysisResult {
        let mut result = TransientAnalysisResult::new(steps);
        result.stats.newton_iterations = self.solver.total_iterations();
        result.stats.converged = true;
        result.stats.steps_accepted = st.steps_accepted;
        result.stats.steps_rejected = st.steps_rejected;
        result.stats.dt_min_floor_hits = st.dt_min_floor_hits;
        result.stats.dt_min = if st.dt_min_seen.is_finite() { st.dt_min_seen } else { 0.0 };
        result.stats.dt_max = st.dt_max_seen;
        result.stats.assembly_time_ns = self.solver.assembly_time_ns();
        result.stats.solve_time_ns = self.solver.solve_time_ns();
        result
    }

    fn snapshot(&self, time: f64) -> TransientStep {
        let mut values = HashMap::new();
        let netlist = self.system.circuit.netlist();
        // A circuit of devices with no unknown has no solution row.
        if let Some(latest_state) = self.solver.current_guess() {
            for reference in netlist.all_references() {
                if let Some(idx) = reference.idx() {
                    values.insert(reference.variable().clone(), latest_state[idx]);
                }
            }
        }

        let step = TransientStep::new(time, values);
        if !self.options.record_device_state
            && self.options.probe_selection.requests.is_empty()
        {
            return step;
        }
        step.with_device_state(self.collect_device_banks())
    }

    /// The recorded device state: the terms' values and the `VAR` watches'
    /// values of every instance (`record_device_state`), or only those the
    /// `probe_selection` requested. Empty `probe_selection` +
    /// `record_device_state = false` records nothing.
    fn collect_device_banks(&self) -> HashMap<String, (Vec<f64>, Vec<f64>)> {
        let bound = self.system.circuit.abi.as_ref().expect("a circuit of devices");
        Self::collect_instance_banks(bound, &self.options.probe_selection)
    }

    /// The recorded `(terms, VARs)` of each device instance: the terms' last
    /// values and the `VAR` watches' accepted values, all of them when the
    /// selection is empty (`record_device_state`), else the requested ones.
    fn collect_instance_banks(
        bound: &crate::core::bind::Bound,
        selection: &crate::core::introspect::ProbeSelection,
    ) -> HashMap<String, (Vec<f64>, Vec<f64>)> {
        let mut device_state = HashMap::new();
        let selective = !selection.requests.is_empty();
        for path in &bound.names.instances {
            if selective && !selection.requests.iter().any(|(d, _)| d == path) {
                continue;
            }
            let Some((terms, vars)) = bound.analog.observable_names(path) else { continue };
            let wanted = |name: &str| !selective || selection.contains(path, name);
            let state: Vec<f64> =
                terms.iter().filter(|n| wanted(n)).filter_map(|n| bound.analog.term_value(path, n)).collect();
            let vars: Vec<f64> = vars
                .iter()
                .filter(|n| wanted(n))
                .filter_map(|n| bound.analog.var(path, n))
                .filter_map(|b| b.get(..8).and_then(|b| b.try_into().ok()).map(f64::from_le_bytes))
                .collect();
            if selective || !state.is_empty() || !vars.is_empty() {
                device_state.insert(path.clone(), (state, vars));
            }
        }
        device_state
    }

}

/// What the protocol's analog side of a circuit of devices carries between
/// its calls (design "The two adapters"): the time loop, the recorded steps,
/// and the converged attempt waiting for its acceptance.
pub(crate) struct DeviceRun {
    st: TimeLoop,
    steps: Vec<TransientStep>,
    node_indices: Vec<usize>,
    pending: Option<PendingStep>,
    /// The end of the step this side proposed last, on its own clock.
    proposed: f64,
    /// The time of the last attempt, for its trace records.
    attempted: f64,
    /// The size of the last accepted step.
    last_dt: f64,
    /// Attempts made (the `max_steps` budget).
    attempts: usize,
    /// The last operating-point solution.
    op: Option<ndarray::Array1<f64>>,
    /// A Newton failure at the step floor: reported by the next attempt, after
    /// the protocol restored its checkpoint for the rejection.
    failed: Option<piperine_analog_solver::error::Error>,
    /// The session's tick length.
    spt: f64,
    /// The smallest `CAP` of the last accepted solution: the next step's bound.
    cap: f64,
    /// The digital nets' names, by net id (each step's digital values).
    net_names: std::sync::Arc<[String]>,
    /// The circuit has a `CROSS` watch: every step ends on its tick and a crossing is located.
    locates: bool,
    /// The attempts of the current step that went past a crossing.
    located: Option<Located>,
    /// The controller's step when the current step was first retaken toward a crossing: the
    /// step after the landing starts from it, not from the landing's short step.
    retaken: Option<f64>,
}

/// The attempts of one step that went past a crossing (§17.3): the accepted time the step
/// starts from, and each rising watch's `(instance, watch)` with its last attempt past the
/// crossing, `(time, value, attempts past it so far)`. It survives the protocol's restore; a
/// step from another accepted time drops it.
struct Located {
    from: f64,
    past: Vec<((usize, usize), (f64, f64, u32))>,
}

/// The crossing time of a watch between the last accepted solution `(t0, e0)`, `e0 ≤ 0`, and the
/// attempt `(t1, e1)`, `e1 > 0`. `prev` is this step's last attempt past the crossing and how
/// many there were. The secant through the attempt and `prev` when it falls inside the
/// bracket; else the bracket's secant, with `e0` halved once per earlier attempt past the
/// crossing (the Illinois rule). The bracket's plain secant stalls on a curved `e`: each retake
/// lands past the crossing again and closes in from one side; the two-point secant and the
/// halving pull the estimate across it (Appendix C.2).
fn crossing_estimate(t0: f64, e0: f64, t1: f64, e1: f64, prev: Option<(f64, f64, u32)>) -> f64 {
    let mut halvings = 0;
    if let Some((tp, ep, n)) = prev {
        if tp > t1 && ep > e1 {
            let t = t1 - e1 * (tp - t1) / (ep - e1);
            if t > t0 && t < t1 {
                return t;
            }
        }
        halvings = n.min(1000);
    }
    let e0 = e0 * 0.5f64.powi(halvings as i32);
    predict_crossing(CrossDir::Rising, t0, e0, t1, e1).map_or(t0, |p| p.t_cross)
}

/// A converged attempt and its truncation verdict, until `device_accept`.
struct PendingStep {
    prediction: StepPrediction,
    snapshot: TransientStep,
    milne: f64,
    post_set_step: bool,
}

/// A checkpoint of the analog side: the Newton history and the assembly's.
pub(crate) type DeviceMark = (CircularArrayBuffer2<f64>, AssemblySnapshot);

impl TransientSolver<'_> {
    /// A transient of a circuit of devices, through the protocol (abi.md §5,
    /// §10): the operating point with every continuous `init` held, then the
    /// step loop. The digital engine leaves the circuit for the run and comes
    /// back after it, whatever the outcome.
    pub fn solve(&mut self) -> crate::core::result::Result<TransientAnalysisResult> {
        use crate::analyses::adapters::{DigitalSide, TransientSide, empty_digital};
        use crate::protocol::{Protocol, ProtocolOptions};
        use piperine_analog_solver::error::{Error, SolverDomain};
        // Re-entry: the captured step and the state of the instances it carries.
        let resume = match self.reentry_state.take() {
            None if self.options.start_time != 0.0 => {
                return Err(Error::simple(
                    SolverDomain::Transient,
                    format!(
                        "a transient of devices starts at {:e} s only from a captured step \
                         (`with_initial_state`); without one it starts at 0",
                        self.options.start_time
                    ),
                ));
            }
            None => None,
            Some(step) => {
                let Some(state) = step.reentry().cloned() else {
                    return Err(Error::simple(
                        SolverDomain::Transient,
                        format!(
                            "the step at {:e} s carries no state of the device instances: \
                             re-enter from a step a transient of this circuit recorded",
                            step.time()
                        ),
                    ));
                };
                Some((step, state))
            }
        };
        // Scheduled host writes: one at or before the start is a write before the
        // run; a later one is a host commit the protocol lands on.
        let mut later = Vec::new();
        for set in std::mem::take(&mut self.sets) {
            let value = match set.value {
                crate::core::introspect::Value::Real(v) => crate::core::bind::ParamValue::Real(v),
                crate::core::introspect::Value::Integer(v) => crate::core::bind::ParamValue::Int(v),
                crate::core::introspect::Value::Boolean(v) => crate::core::bind::ParamValue::Bool(v),
                crate::core::introspect::Value::Text(_) => {
                    return Err(Error::simple(
                        SolverDomain::Transient,
                        format!("`{}.{}`: a text value has no byte layout", set.label, set.param),
                    ));
                }
            };
            if set.t <= self.options.start_time {
                self.system.circuit.set(&set.label, &set.param, value)?;
            } else {
                later.push((set.t, set.label, set.param, value));
            }
        }
        let bound = self.system.circuit.abi.as_mut().expect("a circuit of devices");
        let mut writes = Vec::with_capacity(later.len());
        for (t, path, param, value) in &later {
            let (net, bytes) = bound.param_bytes(path, param, value)?;
            writes.push((*t, format!("{path}.{param}"), net.0, bytes));
        }
        if let Some((_, state)) = &resume
            && state.nets.names() != bound.digital.plan().nets.iter().map(|n| n.name.clone()).collect::<Vec<_>>()
        {
            return Err(Error::simple(
                SolverDomain::Transient,
                "the captured step belongs to another binding of this circuit (it was re-bound since)",
            ));
        }
        let spt = bound.digital.plan().resolution.seconds_per_tick;
        let mut digital = std::mem::replace(&mut bound.digital, empty_digital(spt)?);
        // Reactions and discrete functions read the continuation values at rest: every
        // instant runs after the last solve of its sequence (abi.md §4.2).
        digital.set_continuation(self.system.context.tolerances.gmin, 1.0);
        let net_names: std::sync::Arc<[String]> = digital.plan().nets.iter().map(|n| n.name.clone()).collect();
        let mut side = DigitalSide::new(digital, bound, piperine_abi::raw::global::TRAN);
        let settled = crate::analyses::adapters::Settled::default();
        side.record_settled(settled.clone());
        // A host write made since the capture is written again at the re-entry.
        let mut rewrites = Vec::new();
        if let Some((_, state)) = &resume {
            for (slot, bytes) in side.resume(&state.digital, state.nets.bytes()) {
                bound.analog.set_discrete(slot, &bytes);
            }
            for (net, bytes) in &bound.host_writes {
                if state.nets.get(net.0 as usize) != Some(bytes.as_slice()) {
                    rewrites.push((net.0, bytes.clone()));
                }
            }
        }
        let locates = bound.analog.has_crossings();
        let run = DeviceRun {
            st: TimeLoop::new(self.options.start_time, self.options.dt),
            steps: Vec::new(),
            node_indices: Vec::new(),
            pending: None,
            proposed: self.options.start_time,
            attempted: self.options.start_time,
            last_dt: 0.0,
            attempts: 0,
            op: None,
            failed: None,
            spt,
            cap: f64::INFINITY,
            net_names,
            locates,
            located: None,
            retaken: None,
        };
        let (start, stop) = (self.options.start_time, self.options.stop_time);
        let hooks = self.host.clone();
        let nets = hooks.nets(bound);
        let protocol = Protocol::new(side, TransientSide { tran: self, run, settled }, ProtocolOptions::new(spt));
        let mut protocol = hooks.install(protocol, nets.as_deref().unwrap_or(&[]));
        let ran = nets
            .and_then(|_| match &resume {
                None => protocol.operating_point().and_then(|_| protocol.analog_mut().begin()),
                Some((step, state)) => {
                    let samples = protocol.analog_mut().resume(step, state)?;
                    protocol.resume_at_tick(start, state.digital.now().0, samples);
                    for (net, bytes) in &rewrites {
                        if let crate::protocol::SetOutcome::Refused { reason } = protocol.set(*net, 0, bytes, None)? {
                            return Err(Error::simple(
                                SolverDomain::Transient,
                                format!("a host write made since the capture is refused at the re-entry: {reason}"),
                            ));
                        }
                    }
                    Ok(())
                }
            })
            .and_then(|_| {
                let (digital, nets) = protocol.discrete().state();
                protocol.analog_mut().mark_start(digital, nets);
                Ok(())
            })
            .and_then(|_| {
                for (t, what, net, bytes) in &writes {
                    match protocol.set(*net, 0, bytes, Some(*t))? {
                        crate::protocol::SetOutcome::Committed => {}
                        crate::protocol::SetOutcome::Refused { reason } => {
                            return Err(Error::simple(
                                SolverDomain::Transient,
                                format!("the write of `{what}` at {t:e} s is refused: {reason}"),
                            ));
                        }
                        crate::protocol::SetOutcome::Reshape { instance } => {
                            return Err(Error::simple(
                                SolverDomain::Transient,
                                format!(
                                    "the write of `{what}` at {t:e} s is refused: instance {instance} would need \
                                     another shape inside a transient"
                                ),
                            ));
                        }
                    }
                }
                Ok(())
            })
            .and_then(|_| protocol.transient(stop));
        let (side, analog) = protocol.into_sides();
        let mut run = analog.run;
        if let Some(bound) = self.system.circuit.abi.as_mut() {
            bound.digital = side.into_digital();
        }
        ran?;
        // The last step carries the state a later run re-enters from.
        let bound = self.system.circuit.abi.as_ref().expect("a circuit of devices");
        let nets = (0..bound.digital.plan().nets.len())
            .map(|i| bound.digital.read(piperine_digital_solver::net::NetId(i as u32)).to_vec())
            .collect();
        let state = crate::core::result::ReentryState {
            digital: bound.digital.checkpoint(),
            nets: std::sync::Arc::new(crate::core::result::DigitalNets::new(run.net_names.clone(), nets)),
            assembly: bound.analog.snapshot(),
            cap: run.cap,
        };
        if let Some(last) = run.steps.last_mut() {
            last.set_reentry(Some(std::sync::Arc::new(state)));
        }
        Ok(self.finish_run(&run.st, run.steps))
    }

    pub(crate) fn device_has_unknowns(&self) -> bool {
        self.system.circuit.netlist().max_index().is_some()
    }

    /// One equilibrium round of the transient's operating point: the DC plan
    /// with the hold clamps, started from each pin's `init` (the first round)
    /// or from the last round's solution; then the assembly accepts it.
    pub(crate) fn device_op_round(
        &mut self,
        run: &mut DeviceRun,
    ) -> crate::core::result::Result<(ndarray::Array1<f64>, Ambient)> {
        let uic_clamps: Vec<UicClamp> = self
            .initial_seeds()
            .into_iter()
            .filter_map(|(plus, minus, ic)| plus.map(|plus| UicClamp { plus, minus, ic }))
            .collect();
        let guess: Vec<InitialValue<AnalogReference, f64>> = match &run.op {
            None => uic_clamps
                .iter()
                .filter(|c| c.minus.is_none())
                .map(|c| InitialValue { reference: c.plus.clone(), value: c.ic })
                .collect(),
            Some(x) => self.system.circuit.netlist().initial_values(&self.values_of(x)),
        };
        let x = {
            let mut dc = DcSolver::new(self.system.circuit, Context::default())?;
            dc.system.uic_clamps = uic_clamps.clone();
            dc.apply_initial_conditions(guess);
            dc.equilibrium(true)?
        };
        self.system.uic_clamps = uic_clamps;
        run.op = Some(x.clone());
        let amb = Ambient { time: self.options.start_time, is_static: true, ..self.system.ambient() };
        Ok((x, amb))
    }

    /// The solution vector `x` by variable, as a DC result holds it.
    fn values_of(&self, x: &ndarray::Array1<f64>) -> HashMap<std::sync::Arc<piperine_analog_solver::netlist::AnalogVariable>, f64> {
        let mut values = HashMap::new();
        for reference in self.system.circuit.netlist().all_references() {
            if let Some(i) = reference.idx() {
                values.insert(reference.variable().clone(), x[i]);
            }
        }
        values
    }

    /// After the operating point: its solution seeds the history rows as
    /// `compute_initial_conditions` does, and the start is recorded.
    pub(crate) fn device_begin(&mut self, run: &mut DeviceRun) -> crate::core::result::Result<()> {
        self.system.uic_hold = false;
        if let Some(x) = run.op.take() {
            let iv_dc = self.system.circuit.netlist().initial_values(&self.values_of(&x));
            let device_ic = self.device_initial_conditions(&iv_dc);
            self.push_initial(iv_dc.clone())?;
            self.push_initial(iv_dc)?;
            if !device_ic.is_empty() {
                self.push_initial(device_ic.clone())?;
                self.push_initial(device_ic)?;
            }
            if !self.initial_conditions.is_empty() {
                self.push_initial(self.initial_conditions.clone())?;
                self.push_initial(self.initial_conditions.clone())?;
            }
        }
        self.device_record_start(run);
        Ok(())
    }

    /// The start is recorded, and the run takes the node unknowns the
    /// truncation test reads.
    fn device_record_start(&mut self, run: &mut DeviceRun) {
        let start_time = self.options.start_time;
        if start_time >= self.options.record_from {
            run.steps.push(self.snapshot(start_time));
        }
        self.solver.reset_iteration_counter();
        run.node_indices = self
            .system
            .circuit
            .netlist()
            .all_references()
            .iter()
            .filter(|r| !r.is_branch())
            .filter_map(|r| r.idx())
            .collect();
    }

    /// Re-entry from a captured step instead of an operating point (the PSS
    /// shooting seam), as `apply_reentry_state` does for elements: the
    /// assembly's histories and limit states come back, the captured solution
    /// seeds both history rows (no DC solve, no device or user seed), and the
    /// start is recorded. Returns the continuous values reactions read there.
    pub(crate) fn device_resume(
        &mut self,
        run: &mut DeviceRun,
        step: &TransientStep,
        state: &crate::core::result::ReentryState,
    ) -> crate::core::result::Result<Vec<f64>> {
        self.system.uic_hold = false;
        self.device_bound().analog.restore(&state.assembly);
        let ivs: Vec<InitialValue<AnalogReference, f64>> = self
            .system
            .circuit
            .netlist()
            .all_references()
            .into_iter()
            .filter_map(|reference| {
                step.get(reference.variable().clone()).map(|value| InitialValue { reference: reference.clone(), value })
            })
            .collect();
        self.push_initial(ivs.clone())?;
        self.push_initial(ivs)?;
        run.cap = state.cap;
        self.device_record_start(run);
        let x = self.solver.current_guess().map(|g| g.to_vec()).unwrap_or_default();
        Ok(self.device_bound().analog.samples(&x))
    }

    /// The re-entry state of this run's first recorded step, at the start.
    pub(crate) fn device_mark_start(&mut self, run: &mut DeviceRun, digital: piperine_digital_solver::engine::Checkpoint, nets: Vec<Vec<u8>>) {
        if run.steps.len() != 1 {
            return;
        }
        let state = self.device_reentry_state(run, digital, nets);
        run.steps[0].set_reentry(Some(state));
    }

    /// The re-entry state of the step accepted last, taken at the top of the
    /// next step (its instant ran and its boundary was delivered, nothing has
    /// moved since): the discrete side's settled state, the assembly's and the
    /// step bound. A step already carrying one, or a time not recorded, is left.
    pub(crate) fn device_capture(&mut self, run: &mut DeviceRun, digital: piperine_digital_solver::engine::Checkpoint, nets: Vec<Vec<u8>>) {
        let due = run.steps.last().is_some_and(|s| s.reentry().is_none() && s.time() == run.st.current_time);
        if due {
            let state = self.device_reentry_state(run, digital, nets);
            run.steps.last_mut().expect("a recorded step").set_reentry(Some(state));
        }
    }

    fn device_reentry_state(
        &mut self,
        run: &DeviceRun,
        digital: piperine_digital_solver::engine::Checkpoint,
        nets: Vec<Vec<u8>>,
    ) -> std::sync::Arc<crate::core::result::ReentryState> {
        let assembly = self.device_bound().analog.snapshot();
        let nets = std::sync::Arc::new(crate::core::result::DigitalNets::new(run.net_names.clone(), nets));
        std::sync::Arc::new(crate::core::result::ReentryState { digital, nets, assembly, cap: run.cap })
    }

    /// The end of the next step: the controller's proposal, at most the stop
    /// time and the smallest `CAP` of the last accepted solution.
    pub(crate) fn device_propose(&mut self, run: &mut DeviceRun) -> f64 {
        let st = &run.st;
        run.proposed = (st.current_time + st.dt).min(self.options.stop_time).min(st.current_time + run.cap);
        run.proposed
    }

    /// One step to `to`. The protocol puts every step's end on the tick grid;
    /// a step that is not a landing ends at this side's own proposal, which
    /// lies within one tick of `to`, so the time grid is the controller's.
    /// The step whose target is the stop ends at the stop itself, bit for bit,
    /// as a landing ends at its tick: a sum of controller steps falls ulps short.
    // SPEC_DEVIATION: the step solves to its own proposal, not to `to`, when
    // the two are less than one tick apart and the step is not a landing.
    // Reason: the parity oracles pin the controller's time grid (`parity_baseline`
    // 161 steps, `differential` 1e-15); a femtosecond floor moves every step.
    pub(crate) fn device_attempt(
        &mut self,
        run: &mut DeviceRun,
        to: f64,
        exempt: bool,
    ) -> crate::core::result::Result<crate::protocol::Attempt> {
        use crate::protocol::Attempt;
        if let Some(e) = run.failed.take() {
            return Err(e);
        }
        run.attempts += 1;
        if run.attempts > self.options.max_steps {
            let st = &run.st;
            let progress = (st.current_time - self.options.start_time)
                / (self.options.stop_time - self.options.start_time).max(f64::MIN_POSITIVE);
            return Err(piperine_analog_solver::error::Error::simple(
                piperine_analog_solver::error::SolverDomain::Transient,
                format!(
                    "exceeded the {} step budget at t = {:.6e} s of {:.6e} s \
                     ({:.2}% of the run) with dt = {:.3e} s \
                     (accepted {}, rejected {}, dt_min floor hits {}, \
                     smallest dt seen {:.3e} s). \
                     The timestep has collapsed: the circuit is either stiff \
                     at a switching edge or has a discontinuity the stepper \
                     cannot land on. Raise `max_steps` to let it run longer, \
                     raise `dt_min` to make the floor fail instead of crawl, \
                     or soften the discontinuity.",
                    self.options.max_steps,
                    st.current_time,
                    self.options.stop_time,
                    progress * 100.0,
                    st.dt,
                    st.steps_accepted,
                    st.steps_rejected,
                    st.dt_min_floor_hits,
                    if st.dt_min_seen.is_finite() { st.dt_min_seen } else { 0.0 },
                ),
            ));
        }
        let stop = self.options.stop_time;
        let t_next = if exempt {
            to
        } else if (stop - to).abs() < run.spt {
            stop
        } else if run.locates {
            // A design with a `CROSS` watch steps on the tick grid: a located crossing lands
            // on a tick (§15 T4). No parity oracle has one, so the deviation below stays theirs.
            to
        } else if (run.proposed - to).abs() < run.spt {
            run.proposed
        } else {
            to
        };
        run.attempted = t_next;
        let st = &mut run.st;
        if t_next <= st.current_time {
            // A zero-length step integrates nothing (a `DDT` stamps no companion
            // at `h <= 0`): never attempted, never accepted.
            return Err(piperine_analog_solver::error::Error::simple(
                piperine_analog_solver::error::SolverDomain::Transient,
                format!("a step to t = {t_next:e} s does not advance from t = {:e} s", st.current_time),
            ));
        }
        let prediction = StepPrediction {
            t_next,
            dt_actual: t_next - st.current_time,
            dt_proposed: st.dt,
            landed_on_breakpoint: exempt,
        };
        let post_set_step = st.sets_just_applied;
        let outcome = self.execute_timestep(st.current_time, prediction.dt_actual, st.last_step_accepted);
        if let Ok(Some(snapshot)) = outcome {
            let assessment = self.assess_step(&prediction, post_set_step, &run.node_indices);
            if assessment.accept || self.device_reject_lte(st, &prediction, assessment.milne) {
                run.pending = Some(PendingStep { prediction, snapshot, milne: assessment.milne, post_set_step });
                // §17.3 rule 8: a step that reads a jump does not locate its crossings.
                if run.locates && !post_set_step
                    && let Some(landing) = self.device_locate(run, t_next)?
                {
                    return Ok(Attempt::LandedEarlier(landing));
                }
                return Ok(Attempt::Converged);
            }
            return Ok(Attempt::Rejected);
        }
        st.steps_rejected += 1;
        st.last_step_accepted = false;
        st.dt = self.plan.stepper_mut().reject_dt(prediction.dt_proposed, &self.options.step_limits());
        if st.dt <= self.options.dt_min
            && let Err(e) = outcome
        {
            run.failed = Some(e);
        }
        Ok(Attempt::Rejected)
    }

    /// The locator of §17.3 at the converged attempt `t1`: each `CROSS` watch that went from
    /// `≤ 0` at the last accepted solution to `> 0` here is within its tolerance when its value
    /// is under its `etol` or `t1` exceeds its estimated crossing by at most its `ttol` (a
    /// watch's own pins, else the session's `ttol` and potential `abstol`). When one is not, the
    /// step is retaken at the earliest estimate rounded up to a tick (§15 T4): `Some(landing)`.
    /// `None` accepts the attempt: every crossing is within tolerance, or the earliest estimate
    /// rounds to the attempt's own tick.
    fn device_locate(&mut self, run: &mut DeviceRun, t1: f64) -> crate::core::result::Result<Option<f64>> {
        let t0 = run.st.current_time;
        if run.located.as_ref().is_some_and(|l| l.from != t0) {
            run.located = None;
        }
        let x = self.solver.current_guess().map(|g| g.to_vec()).unwrap_or_default();
        let amb = Ambient { time: t1, ..self.system.ambient() };
        let (ttol, etol) = (self.options.ttol, self.system.context.tolerances.vntol);
        let rising = self.device_bound().analog.rising(&x, amb)?;
        let mut earliest = f64::INFINITY;
        for r in &rising {
            let prev = run.located.as_ref().and_then(|l| {
                l.past.iter().find(|(k, _)| *k == (r.instance, r.watch)).map(|(_, p)| *p)
            });
            let estimate = crossing_estimate(t0, r.before, t1, r.now, prev);
            let within = r.now < r.etol.unwrap_or(etol) || t1 - estimate <= r.ttol.unwrap_or(ttol);
            if !within {
                earliest = earliest.min(estimate);
            }
        }
        // The landing tick: the estimate rounded up, after the accepted time's tick.
        let first = (t0 / run.spt).round() + 1.0;
        let landing = (earliest / run.spt).ceil().max(first);
        if !landing.is_finite() || landing >= (t1 / run.spt).round() {
            run.located = None;
            return Ok(None);
        }
        run.retaken.get_or_insert(run.st.dt);
        let located = run.located.get_or_insert_with(|| Located { from: t0, past: Vec::new() });
        for r in &rising {
            let key = (r.instance, r.watch);
            match located.past.iter_mut().find(|(k, _)| *k == key) {
                Some((_, p)) => *p = (t1, r.now, p.2 + 1),
                None => located.past.push((key, (t1, r.now, 1))),
            }
        }
        Ok(Some(landing * run.spt))
    }

    /// `reject_lte_step` without the restores (the protocol restores its
    /// checkpoint): true when the step is accepted at the step floor.
    fn device_reject_lte(&mut self, st: &mut TimeLoop, prediction: &StepPrediction, milne: f64) -> bool {
        if self.policy.trace.transient {
            eprintln!(
                "REJECT t={:.3e} dt={:.3e} milne={:.3e} (trtol={})",
                st.current_time,
                prediction.dt_actual,
                milne,
                self.system.context.tolerances.trtol
            );
        }
        st.dt = self.plan.stepper_mut().reject_dt(prediction.dt_proposed, &self.options.step_limits());
        if st.dt <= self.options.dt_min {
            st.dt_min_floor_hits += 1;
            tracing::warn!(
                "transient LTE exceeded trtol at dt_min ({:.3e}); \
                 accepting step at t={:.3e} with reduced accuracy",
                st.dt, st.current_time
            );
            return true;
        }
        st.steps_rejected += 1;
        st.last_step_accepted = false;
        false
    }

    /// Accepts the converged attempt: `accept_step`'s bookkeeping without the
    /// old discrete engine; the restart comes with `device_deliver`.
    pub(crate) fn device_accept(&mut self, run: &mut DeviceRun) -> crate::core::result::Result<crate::protocol::Accepted> {
        let PendingStep { prediction, snapshot, milne, post_set_step } =
            run.pending.take().expect("accept follows a converged attempt");
        let StepPrediction { t_next, dt_actual, dt_proposed, .. } = prediction;
        let st = &mut run.st;
        st.steps_accepted += 1;
        st.last_step_accepted = true;
        self.system.uic_hold = false;
        st.dt_min_seen = st.dt_min_seen.min(dt_actual);
        st.dt_max_seen = st.dt_max_seen.max(dt_actual);
        let x = self.solver.current_guess().map(|g| g.to_vec()).unwrap_or_default();
        let amb = Ambient { time: t_next, ..self.system.ambient() };
        let (accepted, cap) = crate::analyses::adapters::accepted(self.device_bound(), &x, amb)?;
        run.cap = cap;
        let st = &mut run.st;
        self.record_step(&mut run.steps, t_next, snapshot);
        st.current_time = t_next;
        self.system.prev_h = dt_actual;
        // A design that locates crossings ends a step on the tick below the controller's
        // proposal: the controller grows from its proposal, or a step of a few ticks would
        // round down to the same tick forever.
        let grown_from = if run.locates && (run.proposed - t_next).abs() < run.spt {
            dt_actual + (run.proposed - t_next)
        } else {
            dt_actual
        };
        st.dt = self.propose_dt(milne, grown_from, dt_proposed, post_set_step, false);
        // A step retaken toward a crossing is short for the crossing's sake, not the error's:
        // the next step starts from the controller's step before the retake.
        if let Some(before) = run.retaken.take() {
            st.dt = st.dt.max(before);
        }
        st.sets_just_applied = false;
        run.last_dt = dt_actual;
        Ok(accepted)
    }

    /// The analog copy takes the boundary; after a jump the next step starts
    /// first-order (`prev_h = 0`) and is exempt from the truncation test and
    /// the controller, as after a scheduled set; after a host write it is also
    /// `1e-3` of the last step, floored at `dt_min`.
    pub(crate) fn device_deliver(
        &mut self,
        run: &mut DeviceRun,
        boundary: &[crate::protocol::Boundary],
        restart: crate::protocol::Restart,
    ) {
        use crate::protocol::Restart;
        if let Some(bound) = self.system.circuit.abi.as_mut() {
            for b in boundary {
                bound.analog.set_discrete(b.slot, &b.bytes);
            }
        }
        match restart {
            Restart::None => {}
            Restart::Integrator => {
                self.system.prev_h = 0.0;
                run.st.sets_just_applied = true;
            }
            Restart::IntegratorSmall => {
                self.system.prev_h = 0.0;
                run.st.dt = (1e-3 * run.last_dt).max(self.options.dt_min);
                run.st.sets_just_applied = true;
            }
        }
    }

    pub(crate) fn device_mark(&self) -> DeviceMark {
        let bound = self.system.circuit.abi.as_ref().expect("a circuit of devices");
        (self.solver.state_snapshot(), bound.analog.snapshot())
    }

    pub(crate) fn device_restore(&mut self, mark: &DeviceMark) {
        self.solver.restore_state(mark.0.clone());
        if let Some(bound) = self.system.circuit.abi.as_mut() {
            bound.analog.restore(&mark.1);
        }
    }

    pub(crate) fn device_bound(&mut self) -> &mut crate::core::bind::Bound {
        self.system.circuit.abi.as_mut().expect("a circuit of devices")
    }
}

impl DeviceRun {
    /// The time of the last attempt.
    pub(crate) fn attempted(&self) -> f64 {
        self.attempted
    }

    /// The smallest `CAP` of an accepted solution bounds the next step.
    pub(crate) fn set_cap(&mut self, cap: f64) {
        self.cap = cap;
    }
}
