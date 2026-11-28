//! The step controller of a transient: the TR-BDF2 step, its truncation test, the next step's size, the cut after a failure and the crossing locator.

use piperine_analog_solver::crossing::{CrossDir, Prediction};
use piperine_analog_solver::device::Ambient;
use piperine_analog_solver::math::integration::{TrBdf2, TrBdf2Phase};
use crate::protocol::ProposalBound;
use crate::prelude::TransientStep;

use super::TransientSolver;
use super::attempt::{DeviceRun, Located};

/// What a transient step reads, which decides how its truncation error is measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StepKind {
    /// A step that reads no change: tested with Milne's estimate.
    Ordinary,
    /// The step that reads a jump (a delivery under a restart, §15.2 rule 6).
    ReadsJump,
    /// The step taken again at the same time after its crossings changed what `analog` reads
    /// (§15.2 rule 4): exempt from the truncation-error test, since it cannot be retaken smaller.
    Retake,
}

impl StepKind {
    /// The kind of the step that starts at a time loop whose `sets_just_applied` is given.
    pub(super) fn of(sets_just_applied: bool) -> Self {
        if sets_just_applied { Self::ReadsJump } else { Self::Ordinary }
    }

    /// Whether the step reads a jump (a retaken step reads the change its crossings made).
    pub(super) fn reads_jump(self) -> bool {
        self != Self::Ordinary
    }
}

/// Phase 1 output — the candidate landing point chosen over the unified
/// breakpoint table from the PI-proposed step size.
pub(super) struct StepPrediction {
    /// Absolute landing time.
    pub(super) t_next: f64,
    /// The step size actually taken: `t_next − t_n`.
    pub(super) dt_actual: f64,
    /// The step size the PI controller proposed (before breakpoint clamping).
    pub(super) dt_proposed: f64,
    /// What the step reads.
    pub(super) kind: StepKind,
}

/// Why an attempt was rejected.
#[derive(Clone, Copy)]
pub(super) enum Rejection {
    /// The step failed the truncation-error test.
    Truncation,
    /// Newton did not converge on the step.
    Newton,
}

/// Phase 3 output — the Milne-LTE accept-gate verdict for a converged
/// candidate step.
pub(super) struct StepAssessment {
    /// The global Milne LTE over node-voltage unknowns (feeds the PI
    /// controller on accept).
    pub(super) milne: f64,
    /// Gate verdict: accept the candidate step.
    pub(super) accept: bool,
}

/// The adaptive time loop's mutable state: the clock, the current step
/// size, and the acceptance bookkeeping the phase methods read and
/// advance. Owned by [`TransientSolver::solve`]; the phase methods take it
/// as an explicit parameter.
#[derive(Clone)]
pub(super) struct TimeLoop {
    /// The accepted time, in seconds since `origin`.
    pub(super) current_time: f64,
    /// The time of the clock's origin since the start of the run: 0 until the protocol moves the
    /// origin forward (past 2^52 ticks, see `AnalogSide::rebase`), so that every time this loop
    /// holds stays small however long the run.
    pub(super) origin: f64,
    /// The stop time, in seconds since `origin`.
    pub(super) stop: f64,
    pub(super) dt: f64,
    pub(super) steps_accepted: usize,
    /// Attempts rejected by the truncation-error test.
    pub(super) rejected_truncation: usize,
    /// Attempts rejected because Newton did not converge.
    pub(super) rejected_newton: usize,
    /// Accepted steps that read a jump.
    pub(super) jump_steps: usize,
    /// Rejected attempts of a step that reads a jump.
    pub(super) jump_retakes: usize,
    /// Accepted steps of the size `max_step`.
    pub(super) steps_at_max_step: usize,
    /// Steps retaken at `dt_min` after a larger attempt failed (ngspice's `delta = delmin`).
    pub(super) dt_min_floor_hits: usize,
    /// Steps accepted at `dt_min` although they fail the truncation test: nothing smaller is
    /// allowed, and ngspice does not fail those either (see [`TransientSolver::device_attempt`]).
    pub(super) floor_accepts: usize,
    /// Predictor gate: extrapolation history is only valid coming off an
    /// accepted step (a rejection leaves rejected rows in the buffer).
    pub(super) last_step_accepted: bool,
    /// The step about to be attempted reads a delivered change (any `Restart::Integrator*`: an
    /// instant that changed a carrier or a param `analog` reads, a settle commit, a host `set`):
    /// its Milne window spans the discontinuity, so it is sized by §15.2 rule 6 and tested by
    /// the first-order jump estimate (`TrBdf2::jump_lte`) instead of the PI controller's error.
    pub(super) sets_just_applied: bool,
    pub(super) dt_min_seen: f64,
    pub(super) dt_max_seen: f64,
}

impl TimeLoop {
    /// The attempts rejected, by either cause.
    pub(super) fn steps_rejected(&self) -> usize {
        self.rejected_truncation + self.rejected_newton
    }

    /// The loop at `start_time` with its first step `dt`, nothing accepted yet.
    pub(super) fn new(start_time: f64, stop_time: f64, dt: f64) -> Self {
        Self {
            current_time: start_time,
            origin: 0.0,
            stop: stop_time,
            dt,
            steps_accepted: 0,
            rejected_truncation: 0,
            rejected_newton: 0,
            jump_steps: 0,
            jump_retakes: 0,
            steps_at_max_step: 0,
            dt_min_floor_hits: 0,
            floor_accepts: 0,
            last_step_accepted: false,
            sets_just_applied: false,
            dt_min_seen: f64::INFINITY,
            dt_max_seen: 0.0,
        }
    }
}

impl TransientSolver<'_> {
    /// Advance one TR-BDF2 step over `[t_n, t_n + dt]`, times in seconds since the clock's `origin`
    /// (devices and the recorded step read `origin` added back). Two Newton solves:
    /// phase 1 (Trapezoidal over `γ·dt`) → `x_{n+γ}`; phase 2 (BDF2 over
    /// `(1−γ)·dt`) → `x_{n+1}`, warm-started from `x_{n+γ}`. Either phase
    /// failing rejects the whole step (TRB-05). The Newton buffer's push
    /// semantics line the history up naturally: phase 1 reads `x_n` at
    /// `view(1)`; phase 2 reads `x_{n+γ}` at `view(1)` and `x_n` at `view(2)`.
    pub(super) fn execute_timestep(
        &mut self,
        origin: f64,
        t_n: f64,
        dt: f64,
        use_predictor: bool,
    ) -> crate::core::result::Result<Option<TransientStep>> {
        let policy = self.policy.clone();
        let tolerances = self.system.context.tolerances.clone();
        let t_next = t_n + dt;

        // Phase 1 — Trapezoidal over γ·dt → intermediate point x_{n+γ}.
        self.system.phase = TrBdf2Phase::Trapezoidal;
        self.system.h = dt;
        self.system.time = origin + (t_n + TrBdf2::GAMMA * dt);
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
            &mut self.system, self.plan.newton(), &tolerances, &policy,
        )?;
        self.system.take_callback_error()?;

        // Phase 2 — BDF2 over (1−γ)·dt → final point x_{n+1} (warm-start x_{n+γ}).
        self.system.phase = TrBdf2Phase::Bdf2;
        self.system.h = dt;
        self.system.time = origin + t_next;
        self.solver.solve_with_strategy(
            &mut self.system, self.plan.newton(), &tolerances, &policy,
        )?;
        self.system.take_callback_error()?;

        // `prev_h` is set by the caller once the global-LTE accept gate passes.
        Ok(Some(self.snapshot(origin + t_next)))
    }

    /// Phase 3 — assess: the accept gate (TRB-05/06) over the truncation error of the step's
    /// `ddt` states (§15.2 rule 1, §17.2 Resolution), each normalized as ngspice's `CKTterr`
    /// does (`TrBdf2::state_lte`): Milne's difference of the state's three values over the two
    /// stages, against `max(h·(abstol + reltol·|i|), reltol·max(|q|, chgtol))`. A circuit with
    /// no `ddt` state has no truncation error, so the controller grows the step. Every step is
    /// tested, whether or not it ends on a landing; the step that reads a jump is tested with the
    /// first-order estimate of its own two stages (`TrBdf2::jump_lte`) against the same `trtol`.
    pub(super) fn assess_step(&self, prediction: &StepPrediction) -> StepAssessment {
        let tolerances = &self.system.context.tolerances;
        let states = self.system.circuit.abi.analog.ddt_states();
        let error = match prediction.kind {
            StepKind::Ordinary => TrBdf2::state_lte(&states, tolerances, prediction.dt_actual),
            StepKind::ReadsJump => TrBdf2::jump_lte(&states, tolerances, prediction.dt_actual),
            StepKind::Retake => return StepAssessment { milne: 0.0, accept: true },
        };
        StepAssessment { milne: error, accept: error <= tolerances.trtol }
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
    pub(super) fn propose_dt(
        &mut self,
        milne: f64,
        dt_actual: f64,
        dt_proposed: f64,
        kind: StepKind,
        sets_just_applied: bool,
    ) -> f64 {
        let mut dt = if kind.reads_jump() {
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
            dt = (Self::HOST_RESTART_FRACTION * dt_actual).max(self.options.dt_min);
        }
        dt
    }

    /// The crossing time of a watch between the last accepted solution `(t0, e0)`, `e0 ≤ 0`, and the
    /// attempt `(t1, e1)`, `e1 > 0`. `prev` is this step's last attempt past the crossing and how
    /// many there were. The secant through the attempt and `prev` when it falls inside the
    /// bracket; else the bracket's secant, with `e0` halved once per earlier attempt past the
    /// crossing (the Illinois rule). The bracket's plain secant stalls on a curved `e`: each retake
    /// lands past the crossing again and closes in from one side; the two-point secant and the
    /// halving pull the estimate across it (Appendix C.2).
    pub(super) fn crossing_estimate(t0: f64, e0: f64, t1: f64, e1: f64, prev: Option<(f64, f64, u32)>) -> f64 {
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
        Prediction::from_samples(CrossDir::Rising, t0, e0, t1, e1).map_or(t0, |p| p.t_cross)
    }

    /// The end of the next step: the controller's proposal, at most the stop
    /// time and the smallest `CAP` of the last accepted solution. The step after a jump (a
    /// delivery under a restart) is at most a tenth of the last step and of the gap to the next
    /// scheduled time `next_due`, floored at `dt_min` (the step reads a discontinuity; ngspice
    /// breaks the same way, a tenth of the interval, `dctran.c`).
    pub(crate) fn device_propose(&mut self, run: &mut DeviceRun, next_due: Option<f64>) -> f64 {
        let st = &run.st;
        let mut dt = st.dt;
        if st.sets_just_applied {
            let last = (run.last_dt > 0.0).then_some(run.last_dt);
            let gap = next_due.map(|due| due - st.current_time).filter(|gap| *gap > 0.0);
            if let Some(span) = [last, gap].into_iter().flatten().reduce(f64::min) {
                dt = dt.min(Self::JUMP_FRACTION * span).max(self.options.dt_min);
            }
        }
        let cap_end = st.current_time + run.cap;
        let mut end = (st.current_time + dt).min(st.stop).min(cap_end);
        // A step that would end within two ticks short of the stop ends at the stop: the
        // remainder would be a step of a few femtoseconds, or less than a tick, which the
        // companion of a charge integrates as noise (a capped step after a jump can end there).
        // The `CAP` bound still holds after the snap: a step is never stretched past it.
        if st.stop - end < 2.0 * run.spt {
            end = st.stop.min(st.current_time + run.cap);
        }
        run.proposed = end;
        run.bound = if end == cap_end && end < st.current_time + dt {
            ProposalBound::Cap
        } else if end == st.stop {
            ProposalBound::Stop
        } else if dt >= self.options.dt_max {
            ProposalBound::MaxStep
        } else {
            ProposalBound::Controller
        };
        run.proposed
    }

    /// Whether a step of `dt_actual` is at the floor: within one tick of `dt_min` (a step on the
    /// tick grid cannot be told apart from it).
    pub(super) fn is_at_floor(&self, run: &DeviceRun, dt_actual: f64) -> bool {
        dt_actual <= self.options.dt_min + run.spt
    }

    /// The step after a failed attempt (Newton, or the truncation test above the floor): the
    /// controller's smaller step. ngspice's `olddelta` rule (`dctran.c`): a smaller step that would
    /// be at or under `dt_min` is `dt_min` instead, and the attempt retaken there is counted in
    /// `dt_min_floor_hits`, once; `true` when the failed attempt was itself at the floor, which has
    /// no retake left.
    pub(super) fn cut_after_failure(&mut self, run: &mut DeviceRun, prediction: &StepPrediction, cause: Rejection) -> bool {
        let at_floor = self.is_at_floor(run, prediction.dt_actual);
        let st = &mut run.st;
        match cause {
            Rejection::Truncation => st.rejected_truncation += 1,
            Rejection::Newton => st.rejected_newton += 1,
        }
        st.jump_retakes += usize::from(st.sets_just_applied);
        st.last_step_accepted = false;
        // The step that reads a jump is clamped below the controller's proposal (§15.2 rule 6), so
        // its retake is an eighth of the step that failed, or it would be retaken at the same size.
        let failed_from = if prediction.kind.reads_jump() { prediction.dt_actual } else { prediction.dt_proposed };
        st.dt = self.plan.stepper_mut().reject_dt(failed_from, &self.options.step_limits());
        let falls_to_floor = st.dt <= self.options.dt_min;
        if falls_to_floor && !at_floor {
            st.dt = self.options.dt_min;
            st.dt_min_floor_hits += 1;
        }
        falls_to_floor && at_floor
    }

    /// The locator of §17.3 at the converged attempt `t1`: each `CROSS` watch that went from
    /// `≤ 0` at the last accepted solution to `> 0` here is within its tolerance when its value
    /// is under its `etol` or `t1` exceeds its estimated crossing by at most its `ttol` (a
    /// watch's own pins, else the session's `ttol` and potential `abstol`). When one is not, the
    /// step is retaken at the earliest estimate rounded up to a tick (§15 T4): `Some(landing)`.
    /// `None` accepts the attempt: every crossing is within tolerance, or the earliest estimate
    /// rounds to the attempt's own tick.
    pub(super) fn device_locate(&mut self, run: &mut DeviceRun, t1: f64) -> crate::core::result::Result<Option<f64>> {
        let t0 = run.st.current_time;
        if run.located.as_ref().is_some_and(|l| l.from != t0) {
            run.located = None;
        }
        let x = self.solver.current_guess().map(|g| g.to_vec()).unwrap_or_default();
        let amb = Ambient { time: run.st.origin + t1, ..self.system.ambient() };
        let (ttol, etol) = (self.options.ttol, self.system.context.tolerances.vntol);
        let rising = self.device_bound().analog.rising(&x, amb)?;
        let mut earliest = f64::INFINITY;
        for r in &rising {
            let prev = run.located.as_ref().and_then(|l| {
                l.past.iter().find(|(k, _)| *k == (r.instance, r.watch)).map(|(_, p)| *p)
            });
            let estimate = Self::crossing_estimate(t0, r.before, t1, r.now, prev);
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

    /// The `REJECT` trace record of a step that failed the truncation test.
    pub(super) fn trace_rejection(&self, run: &DeviceRun, prediction: &StepPrediction, milne: f64) {
        if self.policy.trace.transient {
            self.policy.trace.sink.write(format!(
                "REJECT t={:.3e} dt={:.3e} milne={:.3e} (trtol={})",
                run.st.origin + run.st.current_time,
                prediction.dt_actual,
                milne,
                self.system.context.tolerances.trtol
            ));
        }
    }

    /// The step after a jump is at most this fraction of the last step and of the gap to the next
    /// scheduled time.
    pub(super) const JUMP_FRACTION: f64 = 0.1;

    /// The step that reads a host write restarts at this fraction of the last step, floored at
    /// `dt_min` (ngspice's first step after a breakpoint is about a sixth of the one before it, and the same tenth the landing restart takes).
    pub(super) const HOST_RESTART_FRACTION: f64 = 0.1;
}
