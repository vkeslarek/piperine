//! Transient (`.tran`) — the options, the UIC hold clamps (ngspice `CKTsetIC`),
//! and the adaptive TR-BDF2 driver over the device assembly: Milne-LTE accept
//! gate, PI timestep control, and live scheduled parameter sets, the time loop
//! itself run by the owner protocol.
//!
//! There is no per-analysis `TransientContext`: MD-03 is superseded
//! (p6-cleanup-architecture D2). The step tunables stay on
//! [`TransientAnalysisOptions`], and the shared
//! [`Context`](crate::analyses::Context) carries what every analysis shares.
//!
//! One file per responsibility: `options` (what a run asks for), `system` (the stage's nonlinear
//! system and the UIC clamps), `scheduled_sets` (the host's timed writes), `step_control` (the
//! step, its error test and the next step's size), `attempt` (the analog side the protocol calls);
//! this file holds the solver, its setup and `solve`'s stages.

mod attempt;
mod options;
mod scheduled_sets;
mod step_control;
mod system;

pub(crate) use attempt::{DeviceMark, DeviceRun};
pub use options::{Keep, Reentry, TransientAnalysisOptions};
pub use scheduled_sets::ScheduledSet;
pub use system::{TransientSystem, UicClamp};

use piperine_analog_solver::device::JumpPosition;
use piperine_analog_solver::netlist::AnalogReference;
use crate::analyses::Context;
use crate::core::circuit::CircuitInstance;
use piperine_analog_solver::math::integration::TrBdf2Phase;
use piperine_analog_solver::math::iv::InitialValue;
use piperine_analog_solver::math::linear::AsIndex;
use piperine_analog_solver::math::newton_raphson::NewtonRaphsonSolver;
use piperine_analog_solver::device::Ambient;
use crate::prelude::{TransientAnalysisResult, TransientStep};
use std::collections::HashMap;

use piperine_analog_solver::error::{Error, SolverDomain};

use crate::analyses::adapters::{DigitalSide, Settled, TransientSide};
use crate::core::result::{ReentryState, Result};
use crate::protocol::{Ended, Protocol, ProtocolOptions, StepStatistics};

use scheduled_sets::{HostCommits, ScheduledWrite};
use step_control::TimeLoop;

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

/// The transient solver: the stage system and its Newton solver, the options and the convergence
/// plan, the initial conditions and the host's scheduled writes; [`solve`](Self::solve) runs it.
pub struct TransientSolver<'a> {
    pub system: TransientSystem<'a>,
    pub solver: NewtonRaphsonSolver<AnalogReference, f64>,
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
    /// Host-scheduled writes of a value to a discrete net, in scheduling order.
    net_writes: Vec<ScheduledWrite>,
    /// Full-state re-entry point (PSS shooting enabler): when set, the run
    /// starts from this captured step instead of a DC operating point.
    reentry_state: Option<TransientStep>,
    /// The host's sink and waiters, for a circuit of devices.
    pub(crate) host: crate::analyses::adapters::HostHooks,
}

impl<'a> TransientSolver<'a> {
    /// A transient solver over `circuit` with the default convergence plan.
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
        if options.keep == Keep::Last && (options.record_device_state || !options.probe_selection.requests.is_empty()) {
            return Err(piperine_analog_solver::error::Error::simple(
                piperine_analog_solver::error::SolverDomain::Transient,
                "`Keep::Last` keeps one step and cannot record device state: drop `record_device_state` \
                 and the `probe_selection`, or keep every step",
            ));
        }
        Self::validate_probe_selection(circuit, &options.probe_selection)?;

        let size = circuit.netlist().max_index().map(|i| i + 1).unwrap_or(0);

        let mut system = TransientSystem {
            circuit,
            context,
            time: options.start_time,
            phase: TrBdf2Phase::Trapezoidal,
            h: options.dt,
            prev_h: 0.0,
            jump: JumpPosition::None,
            last_read_jump: false,
            tfinal: options.stop_time,
            uic_clamps: Vec::new(),
            uic_hold: false,
            callback_error: None,
        };
        let bound = &system.circuit.abi;
        let mut tolerances = system.context.tolerances.clone();
        bound.tighten(&mut tolerances);
        system.context.tolerances = tolerances;

        let solver = match NewtonRaphsonSolver::new(&mut system, size, 4) {
            Ok(solver) => solver,
            Err(error) => return Err(system.circuit.name_error(error)),
        };

        Ok(Self {
            system,
            solver,
            options,
            initial_conditions: Vec::new(),
            plan,
            policy: piperine_analog_solver::tolerances::Policy::default(),
            sets: Vec::new(),
            net_writes: Vec::new(),
            reentry_state: None,
            host: Default::default(),
        })
    }

    /// Replace the convergence plan (ABI-42). The host plugs in a plan with a
    /// custom stepper; the driver delegates rejection/proposal to it.
    /// Kept for a host that sets the convergence plan of a transient (`tests/stepper_fold.rs`).
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
        let bound = &circuit.abi;
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
        let x = self.solver.current_guess().map(|g| g.to_vec()).unwrap_or_default();
        let amb = Ambient { time: self.options.start_time, is_static: true, ..self.system.ambient() };
        let bound = &mut self.system.circuit.abi;
        bound.analog.push_stage(&x, amb)?;
        Ok(())
    }

    /// The `(plus, minus, ic)` seeds of the circuit: a continuous pin's `init`
    /// (abi.md §3.1).
    fn initial_seeds(&self) -> Vec<(Option<AnalogReference>, Option<AnalogReference>, f64)> {
        let bound = &self.system.circuit.abi;
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
    /// Record an accepted step's snapshot when it falls inside the
    /// `record_from` window. Runtime banks committed by the digital settle
    /// (idt/operator state) post-date the in-step snapshot — re-attach so
    /// the recorded state matches this point. Device-state recording is
    /// gated by either `record_device_state` (full-bank shorthand) or a
    /// non-empty `probe_selection` (selective recording — ABI-34).
    fn record_step(&self, steps: &mut Vec<TransientStep>, t_next: f64, snapshot: TransientStep) {
        if self.options.keep == Keep::Every && t_next >= self.options.record_from {
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

    /// Assemble the run result: the recorded steps plus the statistics
    /// accumulated by the Newton solver and the time loop.
    fn finish_run(&self, st: &TimeLoop, steps: Vec<TransientStep>) -> TransientAnalysisResult {
        let mut result = TransientAnalysisResult::new(steps);
        result.stats.newton_iterations = self.solver.total_iterations();
        result.stats.converged = true;
        result.stats.steps_accepted = st.steps_accepted;
        result.stats.steps_rejected = st.steps_rejected();
        result.stats.rejected_truncation = st.rejected_truncation;
        result.stats.rejected_newton = st.rejected_newton;
        result.stats.jump_steps = st.jump_steps;
        result.stats.jump_retakes = st.jump_retakes;
        result.stats.steps_at_max_step = st.steps_at_max_step;
        result.stats.dt_min_floor_hits = st.dt_min_floor_hits;
        result.stats.floor_accepts = st.floor_accepts;
        result.stats.dt_min = if st.dt_min_seen.is_finite() { st.dt_min_seen } else { 0.0 };
        result.stats.dt_max = st.dt_max_seen;
        result.stats.assembly_time_ns = self.solver.assembly_time_ns();
        result.stats.solve_time_ns = self.solver.solve_time_ns();
        result
    }

    /// The step a converged attempt carries: empty under `Keep::Last`, where nothing is kept
    /// per step and the one kept step is built after the loop.
    fn snapshot(&self, time: f64) -> TransientStep {
        if self.options.keep == Keep::Last {
            return TransientStep::new(time, HashMap::new());
        }
        self.full_snapshot(time)
    }

    /// The step at `time`: every unknown's value, with the recorded device state when the options ask for it.
    fn full_snapshot(&self, time: f64) -> TransientStep {
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
        let bound = &self.system.circuit.abi;
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

/// The protocol a transient of devices runs: the discrete side and this solver's analog side.
type TransientProtocol<'s, 'a> = Protocol<DigitalSide, TransientSide<'s, 'a>>;

/// Where a run starts: a captured step and the state of the instances it carries (a re-entry), or
/// nothing (an operating point at the start time); and the host writes made since the capture,
/// written again at the re-entry.
struct RunStart {
    /// The captured step and the state of the instances it carries, or nothing.
    resume: Option<(TransientStep, std::sync::Arc<ReentryState>)>,
    /// The `(net, bytes)` of each host write the captured state does not hold.
    rewrites: Vec<(u32, Vec<u8>)>,
}

impl RunStart {
    /// Enters the run at `start`: the operating point and the analog side's beginning, or the
    /// re-entry with its rewrites; then the start's marks and its observed step.
    fn enter(&self, protocol: &mut TransientProtocol<'_, '_>, start: f64) -> Result<()> {
        match &self.resume {
            None => {
                protocol.begin_at(start);
                protocol.operating_point().and_then(|_| protocol.analog_mut().begin())?;
            }
            Some((step, state)) => {
                let samples = protocol.analog_mut().resume(step, state)?;
                protocol.resume_at_tick(start, state.digital.now().0, samples);
                for (net, bytes) in &self.rewrites {
                    if let crate::protocol::SetOutcome::Refused { reason } = protocol.set(*net, 0, bytes, None)? {
                        return Err(Error::simple(
                            SolverDomain::Transient,
                            format!("a host write made since the capture is refused at the re-entry: {reason}"),
                        ));
                    }
                }
            }
        }
        let (digital, nets) = protocol.discrete().state();
        protocol.analog_mut().mark_start(digital, nets);
        protocol.observe_step(start);
        Ok(())
    }
}

/// What the protocol hands back when a run ends: its outcome, its statistics, the analog side's run.
struct RunEnd {
    /// How the run ended, or its failure.
    outcome: Result<Ended>,
    /// What the steps of the run came to.
    statistics: StepStatistics,
    /// The analog side's run.
    run: DeviceRun,
}

impl TransientSolver<'_> {
    /// A transient of a circuit of devices, through the protocol (abi.md §5,
    /// §10): the operating point with every continuous `init` held, then the
    /// step loop. The digital engine leaves the circuit for the run and comes
    /// back after it, whatever the outcome.
    pub fn solve(&mut self) -> Result<TransientAnalysisResult> {
        let resume = self.reentry_point()?;
        let commits = self.scheduled_commits()?;
        self.check_reentry_binding(&resume)?;
        let (side, run, settled, start) = self.sides(resume)?;
        let end = self.run_protocol(side, TransientRun { run, settled }, &start, &commits);
        self.finish_solve(end)
    }

    /// The captured step a re-entry starts from and the state it carries; `None` for a run from an
    /// operating point, whose start time must be a finite time at or after 0.
    fn reentry_point(&mut self) -> Result<Option<(TransientStep, std::sync::Arc<ReentryState>)>> {
        match self.reentry_state.take() {
            None if !self.options.start_time.is_finite() || self.options.start_time < 0.0 => Err(Error::simple(
                SolverDomain::Transient,
                format!(
                    "the start time {:e} s is not a finite time at or after 0; a transient starts at a \
                     time `t >= 0` with an operating point there",
                    self.options.start_time
                ),
            )),
            None => Ok(None),
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
                Ok(Some((step, state)))
            }
        }
    }

    /// A captured step belongs to this binding of the circuit: its nets are the digital plan's.
    fn check_reentry_binding(&self, resume: &Option<(TransientStep, std::sync::Arc<ReentryState>)>) -> Result<()> {
        let bound = &self.system.circuit.abi;
        if let Some((_, state)) = resume
            && state.nets.names() != bound.digital.plan().nets.iter().map(|n| n.name.clone()).collect::<Vec<_>>()
        {
            return Err(Error::simple(
                SolverDomain::Transient,
                "the captured step belongs to another binding of this circuit (it was re-bound since)",
            ));
        }
        Ok(())
    }

    /// The two sides of the protocol: the digital engine taken out of the circuit for the run (at
    /// rest, at the captured state on a re-entry), and the analog side's run state; with where the
    /// run starts.
    fn sides(&mut self, resume: Option<(TransientStep, std::sync::Arc<ReentryState>)>) -> Result<(DigitalSide, DeviceRun, Settled, RunStart)> {
        let bound = &mut self.system.circuit.abi;
        let spt = bound.digital.plan().resolution.seconds_per_tick;
        let mut digital = std::mem::replace(&mut bound.digital, DigitalSide::empty_digital(spt)?);
        // Reactions and discrete functions read the continuation values at rest: every
        // instant runs after the last solve of its sequence (abi.md §4.2).
        digital.set_continuation(self.system.context.tolerances.gmin, 1.0);
        let net_names: std::sync::Arc<[String]> = digital.plan().nets.iter().map(|n| n.name.clone()).collect();
        let mut side = DigitalSide::new(digital, bound, piperine_abi::raw::global::TRAN)?;
        let settled = Settled::default();
        if self.options.reenters_every_step() {
            side.record_settled(settled.clone());
        }
        // A host write made since the capture is written again at the re-entry.
        let mut rewrites = Vec::new();
        if let Some((_, state)) = &resume {
            for (slot, bytes) in side.resume(&state.digital, state.nets.bytes())? {
                bound.analog.set_discrete(slot, &bytes);
            }
            for (net, bytes) in &bound.host_writes {
                if state.nets.get(net.0 as usize) != Some(bytes.as_slice()) {
                    rewrites.push((net.0, bytes.clone()));
                }
            }
        }
        let locates = bound.analog.has_crossings();
        let run = DeviceRun::new(&self.options, spt, net_names, locates);
        Ok((side, run, settled, RunStart { resume, rewrites }))
    }

    /// Runs the protocol from `start` to the stop time, the scheduled `commits` committed after the
    /// start; the digital engine comes back to the circuit whatever the outcome.
    fn run_protocol(&mut self, side: DigitalSide, analog: TransientRun, start: &RunStart, commits: &HostCommits) -> RunEnd {
        let (begin, stop) = (self.options.start_time, self.options.stop_time);
        let hooks = self.host.clone();
        let nets = hooks.nets(&self.system.circuit.abi);
        let TransientRun { run, settled } = analog;
        let options = ProtocolOptions { max_rounds: self.system.context.max_rounds, ..ProtocolOptions::new(run.spt) };
        let protocol = Protocol::new(side, TransientSide { tran: self, run, settled }, options);
        let mut protocol = hooks.install(protocol, nets.as_deref().unwrap_or(&[]));
        let outcome = nets
            .and_then(|_| start.enter(&mut protocol, begin))
            .and_then(|_| commits.commit(&mut protocol))
            .and_then(|_| protocol.transient(stop));
        let statistics = protocol.statistics().clone();
        let (side, analog) = protocol.into_sides();
        let run = analog.run;
        self.system.circuit.abi.digital = side.into_digital();
        RunEnd { outcome, statistics, run }
    }

    /// The result of a run that ended: its error named, or its steps with the last carrying the
    /// state a later run re-enters from (under `Keep::Last`, the one step kept, from the final
    /// solution), and the statistics.
    fn finish_solve(&mut self, end: RunEnd) -> Result<TransientAnalysisResult> {
        let RunEnd { outcome, statistics, mut run } = end;
        outcome.map_err(|error| self.system.circuit.name_error(error))?;
        if self.options.keep == Keep::Last {
            run.steps.push(self.full_snapshot(run.st.origin + run.st.current_time));
        }
        let bound = &self.system.circuit.abi;
        let nets = (0..bound.digital.plan().nets.len())
            .map(|i| bound.digital.read_analog(piperine_digital_solver::net::NetId(i as u32)))
            .collect();
        let state = ReentryState {
            digital: bound.digital.checkpoint(),
            nets: std::sync::Arc::new(crate::core::result::DigitalNets::new(run.net_names.clone(), nets)),
            assembly: bound.analog.snapshot(),
            cap: run.cap,
            analog: self.analog_resume(&run),
        };
        if let Some(last) = run.steps.last_mut() {
            last.set_reentry(Some(std::sync::Arc::new(state)));
        }
        let mut result = self.finish_run(&run.st, run.steps);
        result.set_statistics(statistics);
        Ok(result)
    }
}

/// The analog side's state a run hands the protocol: its run and the discrete side's settled state.
struct TransientRun {
    /// The analog side's run.
    run: DeviceRun,
    /// The discrete side's settled state.
    settled: Settled,
}
