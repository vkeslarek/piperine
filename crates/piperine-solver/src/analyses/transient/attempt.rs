//! The analog side of a transient of devices, as the owner protocol calls it: the operating point, re-entry, each attempt, its acceptance and the delivery of the discrete side's boundary.

use piperine_analog_solver::device::{Ambient, AssemblySnapshot, JumpPosition};
use piperine_analog_solver::netlist::AnalogReference;
use crate::analyses::dc::DcSolver;
use piperine_analog_solver::math::circular_array::CircularArrayBuffer2;
use piperine_analog_solver::math::integration::{TrBdf2, TrBdf2Phase};
use piperine_analog_solver::math::iv::InitialValue;
use crate::protocol::ProposalBound;
use crate::prelude::TransientStep;
use std::collections::HashMap;

use super::{Keep, TransientAnalysisOptions, TransientSolver, UicClamp};
use super::step_control::{Rejection, StepKind, StepPrediction, TimeLoop};

/// What the protocol's analog side of a circuit of devices carries between
/// its calls (design "The two adapters"): the time loop, the recorded steps,
/// and the converged attempt waiting for its acceptance.
pub(crate) struct DeviceRun {
    pub(super) st: TimeLoop,
    pub(super) steps: Vec<TransientStep>,
    pub(super) pending: Option<PendingStep>,
    /// The end of the step this side proposed last, on its own clock.
    pub(super) proposed: f64,
    /// What cut that proposal.
    pub(super) bound: ProposalBound,
    /// The time of the last attempt, for its trace records.
    pub(super) attempted: f64,
    /// The size of the last accepted step.
    pub(super) last_dt: f64,
    /// Attempts made (the `max_steps` budget).
    pub(super) attempts: usize,
    /// The last operating-point solution.
    pub(super) op: Option<ndarray::Array1<f64>>,
    /// A Newton failure at the step floor: reported by the next attempt, after
    /// the protocol restored its checkpoint for the rejection.
    pub(super) failed: Option<piperine_analog_solver::error::Error>,
    /// The nets whose values the last delivery changed under a restart, until
    /// the step that reads them is accepted: what an `E0504` names.
    pub(super) delivered: Vec<String>,
    /// The session's tick length.
    pub(super) spt: f64,
    /// The smallest `CAP` of the last accepted solution: the next step's bound.
    pub(super) cap: f64,
    /// The digital nets' names, by net id (each step's digital values).
    pub(super) net_names: std::sync::Arc<[String]>,
    /// The circuit has a `CROSS` watch: every step ends on its tick and a crossing is located.
    pub(super) locates: bool,
    /// The attempts of the current step that went past a crossing.
    pub(super) located: Option<Located>,
    /// The controller's step when the current step was first retaken toward a crossing: the
    /// step after the landing starts from it, not from the landing's short step.
    pub(super) retaken: Option<f64>,
    /// What the last acceptance changed, until the next one: a step retaken after its crossings
    /// changed what `analog` reads takes it back (§15.2 rule 4).
    pub(super) undo: Option<AcceptUndo>,
    /// The last acceptance was taken back, and the step to be attempted next is its retake.
    pub(super) retaking: bool,
}

impl DeviceRun {
    /// The run of a transient with `options` before its first step: the clock at the start, the
    /// session's tick length `spt`, the digital nets' names, and whether a `CROSS` watch is located.
    pub(super) fn new(options: &TransientAnalysisOptions, spt: f64, net_names: std::sync::Arc<[String]>, locates: bool) -> DeviceRun {
        DeviceRun {
            st: TimeLoop::new(options.start_time, options.stop_time, options.dt),
            steps: Vec::new(),
            pending: None,
            proposed: options.start_time,
            bound: ProposalBound::Controller,
            attempted: options.start_time,
            last_dt: 0.0,
            attempts: 0,
            op: None,
            failed: None,
            delivered: Vec::new(),
            spt,
            cap: f64::INFINITY,
            net_names,
            locates,
            located: None,
            retaken: None,
            undo: None,
            retaking: false,
        }
    }
}

/// What `device_accept` changes in the run, kept so `device_unaccept` can put it back: the time
/// loop, the step controller's memory and the bookkeeping beside them.
pub(super) struct AcceptUndo {
    pub(super) st: TimeLoop,
    pub(super) prev_h: f64,
    pub(super) last_read_jump: bool,
    pub(super) uic_hold: bool,
    pub(super) steps: usize,
    pub(super) cap: f64,
    pub(super) retaken: Option<f64>,
    pub(super) delivered: Vec<String>,
    pub(super) last_dt: f64,
    pub(super) stepper: piperine_analog_solver::convergence::StepperMemory,
}

/// The attempts of one step that went past a crossing (§17.3): the accepted time the step
/// starts from, and each rising watch's `(instance, watch)` with its last attempt past the
/// crossing, `(time, value, attempts past it so far)`. It survives the protocol's restore; a
/// step from another accepted time drops it.
pub(super) struct Located {
    pub(super) from: f64,
    pub(super) past: Vec<((usize, usize), (f64, f64, u32))>,
}


/// A converged attempt and its truncation verdict, until `device_accept`.
pub(super) struct PendingStep {
    pub(super) prediction: StepPrediction,
    pub(super) snapshot: TransientStep,
    pub(super) milne: f64,
    /// The attempt is at the step floor and fails the truncation test: it is accepted anyway.
    pub(super) over_error_at_floor: bool,
}

/// A checkpoint of the analog side: the Newton history and the assembly's.
pub(crate) type DeviceMark = (CircularArrayBuffer2<f64>, AssemblySnapshot);

impl TransientSolver<'_> {
    /// The solution as the Newton solver holds it (empty with no unknown), as `snapshot` reads it.
    pub(crate) fn device_solution(&self) -> &[f64] {
        self.solver.current_guess().and_then(|guess| guess.to_slice()).unwrap_or(&[])
    }

    /// Whether the circuit has an analog unknown (a circuit of devices may have none).
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
            // The session's context: its gmin, the one the transient's reactions read, and its
            // tolerances.
            let context = self.system.context.clone();
            // The operating point iterates with the transient's own plan (its Newton strategy and
            // homotopies); the plan comes back whatever the solve returned.
            let plan = std::mem::take(&mut self.plan);
            let mut dc = DcSolver::new(self.system.circuit, context)?.with_plan(plan);
            dc.system.uic_clamps = uic_clamps.clone();
            dc.system.time = self.options.start_time;
            dc.apply_initial_conditions(guess);
            let solved = dc.equilibrium(true);
            self.plan = dc.into_plan();
            solved?
        };
        self.system.uic_clamps = uic_clamps;
        run.op = Some(x.clone());
        let amb = Ambient { time: self.options.start_time, is_static: true, ..self.system.ambient() };
        Ok((x, amb))
    }

    /// The solution vector `x` by variable, as a DC result holds it.
    pub(super) fn values_of(&self, x: &ndarray::Array1<f64>) -> HashMap<std::sync::Arc<piperine_analog_solver::netlist::AnalogVariable>, f64> {
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

    /// The start is recorded.
    pub(super) fn device_record_start(&mut self, run: &mut DeviceRun) {
        let start_time = self.options.start_time;
        if self.options.keep == Keep::Every && start_time >= self.options.record_from {
            run.steps.push(self.snapshot(start_time));
        }
        self.solver.reset_iteration_counter();
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
        // The captured step's own solution: the Newton history, the time loop and the step
        // controller continue from where the continuous run stood. A shooting analysis
        // re-enters from a solution of its own with the state of another step: that run starts
        // from its solution as a restart, its histories rewritten to it.
        let analog = &state.analog;
        let own = self.system.circuit.netlist().all_references().iter().all(|reference| {
            let Some(i) = reference.idx() else { return true };
            step.get(reference.variable().clone()).is_none_or(|value| {
                analog.newton().latest().is_some_and(|row| row[i].to_bits() == value.to_bits())
            })
        });
        if own {
            self.solver.restore_state(analog.newton().clone());
            run.last_dt = analog.last_dt();
            run.st.dt = analog.dt();
            run.st.last_step_accepted = analog.last_step_accepted();
            run.st.sets_just_applied = analog.restart_pending();
            self.system.prev_h = analog.prev_h();
            self.system.last_read_jump = analog.last_read_jump();
            self.plan.stepper_mut().resume(analog.stepper());
        } else {
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
        }
        run.cap = state.cap;
        self.device_record_start(run);
        let x = self.solver.current_guess().map(|g| g.to_vec()).unwrap_or_default();
        Ok(self.device_bound().analog.samples(&x))
    }

    /// The re-entry state of this run's first recorded step, at the start.
    pub(crate) fn device_mark_start(&mut self, run: &mut DeviceRun, digital: piperine_digital_solver::engine::Checkpoint, nets: Vec<Vec<u8>>) {
        if !self.options.reenters_every_step() || run.steps.len() != 1 {
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
        let due = self.options.reenters_every_step() && run.steps.last().is_some_and(|s| s.reentry().is_none() && s.time() == run.st.origin + run.st.current_time);
        if due {
            let state = self.device_reentry_state(run, digital, nets);
            run.steps.last_mut().expect("a recorded step").set_reentry(Some(state));
        }
    }

    /// The re-entry state of the step accepted last: the discrete side's `digital` checkpoint and
    /// `nets`, the assembly's snapshot, the step bound and the analog time loop.
    pub(super) fn device_reentry_state(
        &mut self,
        run: &DeviceRun,
        digital: piperine_digital_solver::engine::Checkpoint,
        nets: Vec<Vec<u8>>,
    ) -> std::sync::Arc<crate::core::result::ReentryState> {
        let assembly = self.device_bound().analog.snapshot();
        let nets = std::sync::Arc::new(crate::core::result::DigitalNets::new(run.net_names.clone(), nets));
        let analog = self.analog_resume(run);
        std::sync::Arc::new(crate::core::result::ReentryState { digital, nets, assembly, cap: run.cap, analog })
    }

    /// The analog time loop as it stands between two steps.
    pub(super) fn analog_resume(&self, run: &DeviceRun) -> crate::core::result::AnalogResume {
        crate::core::result::AnalogResume::new(
            self.solver.state_snapshot(),
            run.st.dt,
            self.system.prev_h,
            run.last_dt,
            run.st.last_step_accepted,
            run.st.sets_just_applied,
            self.system.last_read_jump,
            self.plan.stepper().memory(),
        )
    }

    /// One step to `to`. The protocol puts every step's end on the tick grid;
    /// a step that is not a landing ends at this side's own proposal, which
    /// lies within one tick of `to`, so the time grid is the controller's.
    /// The step whose target is the stop ends at the stop itself, bit for bit,
    /// as a landing ends at its tick: a sum of controller steps falls ulps short.
    pub(crate) fn device_attempt(
        &mut self,
        run: &mut DeviceRun,
        to: f64,
        landing: bool,
    ) -> crate::core::result::Result<crate::protocol::Attempt> {
        use crate::protocol::Attempt;
        if let Some(e) = run.failed.take() {
            return Err(e);
        }
        run.attempts += 1;
        if run.attempts > self.options.max_steps {
            let st = &run.st;
            let progress = (st.origin + st.current_time - self.options.start_time)
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
                    st.origin + st.current_time,
                    self.options.stop_time,
                    progress * 100.0,
                    st.dt,
                    st.steps_accepted,
                    st.steps_rejected(),
                    st.dt_min_floor_hits,
                    if st.dt_min_seen.is_finite() { st.dt_min_seen } else { 0.0 },
                ),
            ));
        }
        let stop = run.st.stop;
        let t_next = if landing {
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
                format!("a step to t = {:e} s does not advance from t = {:e} s", st.origin + t_next, st.origin + st.current_time),
            ));
        }
        let prediction = StepPrediction {
            t_next,
            dt_actual: t_next - st.current_time,
            dt_proposed: st.dt,
            kind: if run.retaking { StepKind::Retake } else { StepKind::of(st.sets_just_applied) },
        };
        self.system.jump = if st.sets_just_applied {
            JumpPosition::Reads
        } else if self.system.last_read_jump {
            JumpPosition::After
        } else {
            JumpPosition::None
        };
        let outcome = self.execute_timestep(st.origin, st.current_time, prediction.dt_actual, st.last_step_accepted);
        if let Ok(Some(snapshot)) = outcome {
            let kind = prediction.kind;
            let assessment = self.assess_step(&prediction);
            // The floor rule, ngspice's (`dctran.c`, "Timestep too small"): nothing smaller than
            // `dt_min` is tried, so an attempt already at it that only fails the truncation test
            // is accepted and counted; a Newton failure there ends the run (below).
            let over_error_at_floor = !assessment.accept && self.is_at_floor(run, prediction.dt_actual);
            if assessment.accept || over_error_at_floor {
                run.pending = Some(PendingStep { prediction, snapshot, milne: assessment.milne, over_error_at_floor });
                // §17.3 rule 8: a step that reads a jump does not locate its crossings.
                if run.locates && !kind.reads_jump()
                    && let Some(landing) = self.device_locate(run, t_next)?
                {
                    return Ok(Attempt::LandedEarlier(landing));
                }
                return Ok(Attempt::Converged);
            }
            self.trace_rejection(run, &prediction, assessment.milne);
            self.cut_after_failure(run, &prediction, Rejection::Truncation);
            return Ok(Attempt::Rejected);
        }
        // A Newton failure: ngspice cuts the step to an eighth; one that would fall to the floor is
        // retaken there once, and the attempt at the floor that fails again is "Timestep too small".
        let spent = self.cut_after_failure(run, &prediction, Rejection::Newton);
        if spent && let Err(e) = outcome {
            run.failed = Some(run.floor_error(e, t_next, prediction.kind.reads_jump()));
        }
        Ok(Attempt::Rejected)
    }

    /// Accepts the converged attempt: `accept_step`'s bookkeeping without the
    /// old discrete engine; the restart comes with `device_deliver`.
    pub(crate) fn device_accept(&mut self, run: &mut DeviceRun) -> crate::core::result::Result<crate::protocol::Accepted> {
        let PendingStep { prediction, snapshot, milne, over_error_at_floor } =
            run.pending.take().expect("accept follows a converged attempt");
        run.retaking = false;
        run.undo = Some(AcceptUndo {
            st: run.st.clone(),
            prev_h: self.system.prev_h,
            last_read_jump: self.system.last_read_jump,
            uic_hold: self.system.uic_hold,
            steps: run.steps.len(),
            cap: run.cap,
            retaken: run.retaken,
            delivered: run.delivered.clone(),
            last_dt: run.last_dt,
            stepper: self.plan.stepper().memory(),
        });
        let StepPrediction { t_next, dt_actual, dt_proposed, kind, .. } = prediction;
        let st = &mut run.st;
        st.steps_accepted += 1;
        st.jump_steps += usize::from(kind.reads_jump());
        st.steps_at_max_step += usize::from(dt_actual + run.spt >= self.options.dt_max);
        st.floor_accepts += usize::from(over_error_at_floor);
        st.last_step_accepted = true;
        self.system.uic_hold = false;
        st.dt_min_seen = st.dt_min_seen.min(dt_actual);
        st.dt_max_seen = st.dt_max_seen.max(dt_actual);
        let x = self.solver.current_guess().map(|g| g.to_vec()).unwrap_or_default();
        let amb = Ambient { time: run.st.origin + t_next, ..self.system.ambient() };
        let (accepted, cap) = self.device_bound().accepted(&x, amb)?;
        run.cap = cap;
        let st = &mut run.st;
        self.record_step(&mut run.steps, st.origin + t_next, snapshot);
        st.current_time = t_next;
        self.system.prev_h = dt_actual;
        self.system.last_read_jump = st.sets_just_applied;
        // A design that locates crossings ends a step on the tick below the controller's
        // proposal: the controller grows from its proposal, or a step of a few ticks would
        // round down to the same tick forever.
        let grown_from = if run.locates && (run.proposed - t_next).abs() < run.spt {
            dt_actual + (run.proposed - t_next)
        } else {
            dt_actual
        };
        st.dt = self.propose_dt(milne, grown_from, dt_proposed, kind, false);
        // A step retaken toward a crossing is short for the crossing's sake, not the error's:
        // the next step starts from the controller's step before the retake.
        if let Some(before) = run.retaken.take() {
            st.dt = st.dt.max(before);
        }
        st.sets_just_applied = false;
        run.delivered.clear();
        run.last_dt = dt_actual;
        Ok(accepted)
    }

    /// Takes back the last `device_accept`: the time loop, the controller and the recorded step
    /// are as the attempt found them, so the same step can be attempted again (§15.2 rule 4).
    /// The solution history and the assembly come back with `device_restore`.
    pub(crate) fn device_unaccept(&mut self, run: &mut DeviceRun) {
        let undo = run.undo.take().expect("a retake follows an acceptance");
        run.retaking = true;
        run.st = undo.st;
        self.system.prev_h = undo.prev_h;
        self.system.last_read_jump = undo.last_read_jump;
        self.system.uic_hold = undo.uic_hold;
        run.steps.truncate(undo.steps);
        run.cap = undo.cap;
        run.retaken = undo.retaken;
        run.delivered = undo.delivered;
        run.last_dt = undo.last_dt;
        self.plan.stepper_mut().resume(&undo.stepper);
    }

    /// The zero-width solve of docs/spec §15.2 rule 4 at the last accepted time `t`, on the discrete
    /// values delivered since: one backward-Euler solve from the solution at `t` over
    /// `ZERO_WIDTH_TICKS` of a tick, a width no tick tells from zero, with the time kept at `t`.
    /// Each `ddt` state is held to within that width times its derivative, so no integrand gains
    /// area, and a constraint the delivered values force (an `idt` held at its `ic`) moves its
    /// state there, the sum over each cut it closes conserved as in the step that reads a jump.
    /// The solution replaces the recorded one at `t` and is the history the next step starts
    /// from, which reads a jump. `None`: Newton did not converge.
    pub(crate) fn device_zero_width(
        &mut self,
        run: &mut DeviceRun,
    ) -> crate::core::result::Result<Option<crate::protocol::Accepted>> {
        let time = run.st.origin + run.st.current_time;
        self.system.phase = TrBdf2Phase::Trapezoidal;
        self.system.h = Self::ZERO_WIDTH_TICKS * run.spt / TrBdf2::GAMMA;
        self.system.prev_h = 0.0;
        self.system.jump = JumpPosition::Reads;
        self.system.time = time;
        let (policy, tolerances) = (self.policy.clone(), self.system.context.tolerances.clone());
        let solved = self.solver.solve_with_strategy(&mut self.system, self.plan.newton(), &tolerances, &policy);
        if solved.is_err() || self.system.take_callback_error().is_err() {
            return Ok(None);
        }
        if run.steps.last().is_some_and(|step| step.time() == time) {
            run.steps.pop();
        }
        let snapshot = self.snapshot(time);
        self.record_step(&mut run.steps, time, snapshot);
        let x = self.solver.current_guess().map(|g| g.to_vec()).unwrap_or_default();
        let amb = Ambient { time, ..self.system.ambient() };
        let (accepted, cap) = self.device_bound().accepted(&x, amb)?;
        run.cap = cap;
        run.st.sets_just_applied = true;
        Ok(Some(accepted))
    }

    /// The width of a zero-width solve, in ticks.
    pub(super) const ZERO_WIDTH_TICKS: f64 = 1e-6;

    /// The analog copy takes the boundary; after a jump the next step starts
    /// first-order (`prev_h = 0`) and is sized and tested by the
    /// jump rule (§15.2 rule 6), as after a scheduled set; after a host write it is also
    /// `1e-3` of the last step, floored at `dt_min`.
    pub(crate) fn device_deliver(
        &mut self,
        run: &mut DeviceRun,
        boundary: &[crate::protocol::Boundary],
        restart: crate::protocol::Restart,
    ) {
        use crate::protocol::Restart;
        let bound = &mut self.system.circuit.abi;
        for b in boundary {
            bound.analog.set_discrete(b.slot, &b.bytes);
        }
        run.delivered = match restart {
            Restart::None => Vec::new(),
            Restart::Integrator | Restart::IntegratorSmall => boundary
                .iter()
                .filter_map(|b| bound.names.slot_nets.get(b.slot as usize))
                .filter_map(|net| run.net_names.get(net.0 as usize).cloned())
                .collect(),
        };
        match restart {
            Restart::None => {}
            Restart::Integrator => {
                self.system.prev_h = 0.0;
                run.st.sets_just_applied = true;
            }
            Restart::IntegratorSmall => {
                self.system.prev_h = 0.0;
                run.st.dt = (Self::HOST_RESTART_FRACTION * run.last_dt).max(self.options.dt_min);
                run.st.sets_just_applied = true;
            }
        }
    }

    /// A checkpoint of the analog side: the Newton history and the assembly's.
    pub(crate) fn device_mark(&self) -> DeviceMark {
        let bound = &self.system.circuit.abi;
        (self.solver.state_snapshot(), bound.analog.snapshot())
    }

    /// Puts the analog side back at the checkpoint `mark`.
    pub(crate) fn device_restore(&mut self, mark: &DeviceMark) {
        self.solver.restore_state(mark.0.clone());
        let bound = &mut self.system.circuit.abi;
        bound.analog.restore(&mark.1);
    }

    /// The bound circuit the analog side stamps.
    pub(crate) fn device_bound(&mut self) -> &mut crate::core::bind::Bound {
        &mut self.system.circuit.abi
    }
}

impl DeviceRun {
    /// The Newton failure `error` of the attempt to `time`, which is at the
    /// step floor: `E0508`, or `E0504` when it is the first step after a
    /// delivery under a restart (it reads a jump), naming the nets delivered.
    /// A failure with a code of its own (singular, non-finite) keeps it.
    pub(super) fn floor_error(
        &self,
        error: piperine_analog_solver::error::Error,
        time: f64,
        reads_a_jump: bool,
    ) -> piperine_analog_solver::error::Error {
        use piperine_analog_solver::error::{Error, RunCode, SolverDomain};
        let Error::Run(run) = &error else { return error };
        if run.code != RunCode::E0505 {
            return error.with_time(time);
        }
        let (code, what) = if reads_a_jump && !self.delivered.is_empty() {
            (
                RunCode::E0504,
                format!(
                    "the step after a jump fails at the smallest step; it reads the delivery of {}",
                    self.delivered.join(", ")
                ),
            )
        } else {
            (RunCode::E0508, "the step fails at the smallest step".to_string())
        };
        Error::run(code, SolverDomain::Transient, format!("{what} ({})", run.detail))
            .with_time(time)
            .with_unknowns(run.unknowns.clone())
    }

    /// What cut the proposal the last `device_propose` returned.
    pub(crate) fn proposal_bound(&self) -> ProposalBound {
        self.bound
    }

    /// The time of the last attempt, in seconds since the run's start.
    pub(crate) fn attempted(&self) -> f64 {
        self.st.origin + self.attempted
    }

    /// The time of the last accepted step (the start before any), as the step records it.
    pub(crate) fn accepted_time(&self) -> f64 {
        self.st.origin + self.st.current_time
    }

    /// Moves the origin of this run's clock forward (`AnalogSide::rebase`): every time the run
    /// holds becomes seconds since the new origin, and what reads absolute time adds `origin.time`
    /// back. A step's history of times (`Located`) moves with them.
    pub(crate) fn rebase(&mut self, origin: crate::protocol::Origin) {
        let moved = origin.moved;
        self.st.current_time -= moved;
        self.st.origin = origin.time;
        self.st.stop = origin.stop;
        self.proposed -= moved;
        self.attempted -= moved;
        if let Some(located) = &mut self.located {
            located.from -= moved;
            for (_, (time, _, _)) in &mut located.past {
                *time -= moved;
            }
        }
    }

    /// The smallest `CAP` of an accepted solution bounds the next step.
    pub(crate) fn set_cap(&mut self, cap: f64) {
        self.cap = cap;
    }
}
