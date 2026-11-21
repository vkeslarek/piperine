//! The transient protocol of abi.md §5 over the two sides:
//!
//! ```text
//! C := checkpoint(discrete, analog, sink buffer)                 at accepted time t
//! 1. t_c := analog.propose(t, stop)           (the stop time when there is no unknown)
//! 2. discrete.advance(tick(t_c)): a Landing or Final shortens t_c to its tick
//! 3. attempt to t_c on the value before the instant (the left value), exempt when
//!    t_c is a scheduled instant
//! 4. Rejected: restore C, propose again.  LandedEarlier(t'): restore C, t_c := t', go to 2
//! 5. accept; fire the crossings at (t_c, 0); run the instant t_c, whose reactions read the
//!    solution at t_c, settling when it is a time point; deliver what it changed
//!    (microstep 0 included) with the restart the next step starts from
//! 6. t := t_c; a Final mark or a FINISH ends the analysis after this instant
//! ```
//!
//! The order of steps 3 and 5 is the user's (solver-completeness context.md,
//! 2026-11-15: "ABI order: solve, then react"); the jump handling is the kept one.
//!
//! What a restore replays is discrete work only; one analog solve per landing.

use piperine_analog_solver::error::{Error, Result, SolverDomain};

use super::host::Waiter;
use super::sides::{Advance, AnalogSide, AnalysisKind, Attempt, DiscreteSide, Horizon, InstantMode, Outcome, Restart, Tick};
use super::sink::{Delivery, Sink};
use super::step::{StepObserver, observe};

/// How the protocol maps time and bounds its loops.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProtocolOptions {
    /// The session resolution: seconds per tick.
    pub seconds_per_tick: f64,
    /// The bound on the equilibrium solves of an operating point (`E0506`).
    pub max_rounds: u32,
    /// The bound on consecutive rejected attempts from one accepted time;
    /// beyond it the transient fails naming that time.
    pub max_rejections: u32,
}

impl ProtocolOptions {
    /// The default bound on consecutive rejected attempts.
    pub const MAX_REJECTIONS: u32 = 64;

    /// Options with the default bounds: 20 rounds, 64 rejections.
    pub fn new(seconds_per_tick: f64) -> ProtocolOptions {
        ProtocolOptions { seconds_per_tick, max_rounds: 20, max_rejections: Self::MAX_REJECTIONS }
    }
}

/// How an analysis ended.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ended {
    /// It ran to its end (the stop time; for an operating point, its settle).
    StopTime,
    /// A reaction returned `FINISH`; the analysis ended after the instant at this time.
    Finish(f64),
}

/// The owner's protocol over a discrete and an analog side.
pub struct Protocol<D: DiscreteSide, A: AnalogSide> {
    pub(super) discrete: D,
    pub(super) analog: A,
    pub(super) options: ProtocolOptions,
    /// The last accepted time.
    pub(super) now: f64,
    /// The tick of the last instant run, at or before `now`'s step: the tick
    /// the next step starts from. Equal to `tick(now)` except after a resume
    /// from a time off the grid.
    pub(super) now_tick: Tick,
    pub(super) in_transient: bool,
    pub(super) delivery: Delivery,
    /// The time a `FINISH` ended the run at, if one did.
    pub(super) finished: Option<f64>,
    /// The operating point has started the discrete side.
    pub(super) started: bool,
    /// The continuous values reactions read, at the last accepted solution.
    pub(super) samples: Vec<f64>,
    /// Host waiters by event net (abi.md R7).
    pub(super) waiters: Vec<(u32, Waiter)>,
    /// The host's observer of accepted steps.
    pub(super) observer: Option<Box<dyn StepObserver>>,
    /// A settle commit was delivered to the analog side: the step that arrives at the next time
    /// point is exempt from the truncation test, until one is accepted (§15 S3).
    pub(super) exempt_next: bool,
}

/// The three marks of one checkpoint.
struct Checkpoint<D, A> {
    discrete: D,
    analog: A,
    sink: usize,
}

impl<D: DiscreteSide, A: AnalogSide> Protocol<D, A> {
    pub fn new(discrete: D, analog: A, options: ProtocolOptions) -> Protocol<D, A> {
        Protocol {
            discrete,
            analog,
            options,
            now: 0.0,
            now_tick: 0,
            in_transient: false,
            delivery: Delivery::default(),
            finished: None,
            started: false,
            samples: Vec::new(),
            waiters: Vec::new(),
            observer: None,
            exempt_next: false,
        }
    }

    /// Hands every accepted step of a transient to `observer`, after the sink has the
    /// observations up to its time.
    pub fn with_step_observer(mut self, observer: Box<dyn StepObserver>) -> Protocol<D, A> {
        self.observer = Some(observer);
        self
    }

    /// Hands the observer the sides as they stand at `time`: after each flush, and for the start
    /// of a transient (called by its driver).
    pub(crate) fn observe_step(&mut self, time: f64) {
        observe(&mut self.observer, time, &self.discrete, &self.analog);
    }

    /// Sends every observation to `sink` from now on.
    pub fn with_sink(mut self, sink: Box<dyn Sink>) -> Protocol<D, A> {
        self.delivery.set_sink(sink);
        self
    }

    pub fn discrete(&self) -> &D {
        &self.discrete
    }

    pub fn analog(&self) -> &A {
        &self.analog
    }

    pub fn analog_mut(&mut self) -> &mut A {
        &mut self.analog
    }

    /// Gives both sides back.
    pub fn into_sides(self) -> (D, A) {
        (self.discrete, self.analog)
    }

    /// The last accepted time.
    pub fn now(&self) -> f64 {
        self.now
    }

    /// The observations waiting for an acceptance.
    pub fn buffered(&self) -> usize {
        self.delivery.buffered()
    }

    pub(super) fn tick(&self, t: f64) -> Tick {
        (t / self.options.seconds_per_tick).round() as Tick
    }

    /// The last tick not after `t` (a step never ends past what was proposed).
    /// A time a few ulps short of a tick (a sum of controller steps) is that
    /// tick: the slack is relative, as the error of the sum is, and stays
    /// under half a tick, so a step never ends a tick or more past the
    /// proposal (nor past the `CAP` bound the proposal honors).
    fn floor_tick(&self, t: f64) -> Tick {
        let ticks = t / self.options.seconds_per_tick;
        (ticks + (1e-9 + ticks.abs() * 1e-13).min(0.5)).floor() as Tick
    }

    pub(super) fn time(&self, tick: Tick) -> f64 {
        tick as f64 * self.options.seconds_per_tick
    }

    /// How many `f64` spacings the shortest step spans once a tick is finer than the spacing.
    const SHORTEST_STEP_SPACINGS: f64 = 256.0;

    /// The fewest ticks a proposed step advances. A tick is an exact integer, but the time the
    /// analog side steps on is an `f64`: once its spacing (`ulp`) passes one tick (past 2^52 ticks,
    /// about 4.5 s at the default 1 fs) a step of a few ticks is a few `f64` steps, and a stepper
    /// that collapses onto a stiff edge crawls on them for ever. A step then spans at least
    /// `SHORTEST_STEP_SPACINGS` spacings: about 0.9 ps at 16 s and 1.8 ps at 32 to 64 s at 1 fs, growing
    /// with the time reached (a stopgap, see Appendix C.2.1). While one tick spans more than one spacing
    /// (every run under 4.5 s, and every run at a coarser resolution) the minimum stays one tick.
    fn min_advance(&self) -> Tick {
        let spacing = self.now.next_up() - self.now;
        if spacing <= self.options.seconds_per_tick {
            return 1;
        }
        ((Self::SHORTEST_STEP_SPACINGS * spacing) / self.options.seconds_per_tick).ceil() as Tick
    }

    /// The time of the step that ends at `tick`, a tick after `now_tick` or later. Past 2^52 ticks
    /// (about 4.5 s at the default 1 fs) neighbouring ticks are closer than an `f64` can tell
    /// apart, so `time(tick)` can equal the time already reached; the step then ends one `f64`
    /// step after `now`, which keeps time strictly advancing. The tick stays the exact instant of
    /// the discrete side, and a time the plain conversion already moves past `now` is unchanged.
    fn step_time(&self, tick: Tick) -> f64 {
        let time = self.time(tick);
        if time > self.now { time } else { self.now.next_up() }
    }

    fn checkpoint(&mut self) -> Checkpoint<D::Mark, A::Mark> {
        Checkpoint { discrete: self.discrete.mark(), analog: self.analog.mark(), sink: self.delivery.mark() }
    }

    fn restore(&mut self, c: &Checkpoint<D::Mark, A::Mark>) {
        self.discrete.restore(&c.discrete);
        self.analog.restore(&c.analog);
        self.delivery.restore(c.sink);
    }

    /// Ends a checkpoint: the discrete side's journal closes (the analog side's snapshot is dropped).
    fn release(&mut self, c: Checkpoint<D::Mark, A::Mark>) {
        self.discrete.release(c.discrete);
    }

    /// Continues from a state both sides were put back to (a transient
    /// re-entered at a captured step, the PSS shooting seam): the last
    /// accepted time is `now`, the discrete side has started, and `samples`
    /// are the continuous values its reactions read there. No operating point
    /// runs; `transient` steps forward from `now`, on from the tick `now` floors
    /// to, as a stop does.
    pub fn resume(&mut self, now: f64, samples: Vec<f64>) {
        let tick = self.floor_tick(now);
        self.resume_at_tick(now, tick, samples);
    }

    /// `resume`, with the tick the discrete side ran its last instant at (its
    /// checkpoint's own): a time off the grid lies within a tick of it, on
    /// either side, so no rounding of `now` recovers it.
    pub fn resume_at_tick(&mut self, now: f64, tick: Tick, samples: Vec<f64>) {
        self.now = now;
        self.now_tick = tick;
        self.started = true;
        self.finished = None;
        self.exempt_next = false;
        self.samples = samples;
    }

    /// Puts the protocol's clock at `now` before the operating point of a run that starts there:
    /// the operating point's instant is `(tick(now), 0)`.
    pub fn begin_at(&mut self, now: f64) {
        self.now = now;
        self.now_tick = self.tick(now);
    }

    /// Buffers what the discrete side observed.
    pub(super) fn collect(&mut self, analysis: AnalysisKind) {
        let discrete = &mut self.discrete;
        self.delivery.collect(analysis, |out| discrete.drain(out));
    }

    /// Runs a transient from the last accepted time (after `operating_point`)
    /// to `stop`.
    pub fn transient(&mut self, stop: f64) -> Result<Ended> {
        if let Some(t) = self.finished {
            return Ok(Ended::Finish(t));
        }
        self.in_transient = true;
        let ended = self.transient_loop(stop);
        self.drop_settle_commits_at_stop();
        self.delivery.flush_all();
        self.in_transient = false;
        ended
    }

    fn transient_loop(&mut self, stop: f64) -> Result<Ended> {
        // Every step ends on the tick grid, so the instant run at a step's end
        // is that tick's own (an instant runs only at its own tick). The stop
        // takes the steps' own rounding: rounded up, a stop off the grid lies a
        // tick past the step that reaches it, and the loop would run once more.
        let stop_tick = self.floor_tick(stop);
        if stop > self.now && stop_tick <= self.now_tick {
            // Every step ends on a later tick, past this stop: a run that took
            // no step would end short of it with no error.
            return Err(Error::simple(
                SolverDomain::Transient,
                format!(
                    "the stop {stop:e} s lies after t = {:e} s but inside its tick {}: no step reaches it",
                    self.now, self.now_tick
                ),
            ));
        }
        let stop = self.time(stop_tick);
        // A run error carries its own code and time: it is stamped, never wrapped.
        let at = |t: f64, e: Error| match e {
            Error::Run(_) => e.with_time(t),
            other => Error::wrap(SolverDomain::Transient, format!("at t = {t:e} s"), other),
        };
        let mut forced: Option<Tick> = None;
        let mut rejections = 0u32;
        while self.now_tick < stop_tick {
            let c = self.checkpoint();
            let now_tick = self.now_tick;
            let unknowns = self.analog.has_unknowns();
            let mut tick_c = match forced.take() {
                Some(t) => t,
                None if unknowns => {
                    let next_due = self.discrete.next_due().map(|tick| self.time(tick));
                    let proposed = self.analog.propose(self.now, stop, next_due);
                    self.floor_tick(proposed).clamp((now_tick + self.min_advance()).min(stop_tick), stop_tick)
                }
                None => stop_tick,
            };
            // Step 2: the discrete instants before t_c, stopping before a landing.
            let (mut settle, mut final_mark) = (true, false);
            let advanced = self.discrete.advance(tick_c).map_err(|e| at(self.time(tick_c), e))?;
            match advanced {
                Advance::Reached => {}
                Advance::Landing { tick, time_point } => {
                    tick_c = tick;
                    settle = time_point;
                }
                Advance::Final(tick) => {
                    tick_c = tick;
                    final_mark = true;
                }
            }
            if tick_c <= now_tick {
                return Err(Error::simple(
                    SolverDomain::Transient,
                    format!("a landing at tick {tick_c} does not advance from tick {now_tick} ({:e} s)", self.now),
                ));
            }
            let t_c = self.step_time(tick_c);
            self.collect(AnalysisKind::Transient);

            // Steps 3 and 4: one analog attempt to t_c.
            let samples = if unknowns {
                let exempt = !matches!(advanced, Advance::Reached) || self.exempt_next;
                let attempt = self.analog.attempt(t_c, exempt).map_err(|e| at(t_c, e))?;
                let analog = &mut self.analog;
                self.delivery.keep_traces(AnalysisKind::Transient, |out| analog.drain_traces(out));
                // A retry: `Some(tick)` to attempt again (`None`: propose again).
                let retry = match attempt {
                    Attempt::Converged => None,
                    Attempt::Rejected => Some(None),
                    Attempt::LandedEarlier(t) => {
                        let nearest = self.tick(t);
                        let shortest = now_tick + self.min_advance();
                        if t >= t_c || tick_c - 1 < shortest {
                            // The crossing lies at or past the step's end, or the step is
                            // too short for a shorter one to fit in it: the step is taken at
                            // its end and the analog side reports the crossing in
                            // `Accepted::fired`.
                            None
                        } else {
                            // The nearest tick inside the step, at least one tick shorter.
                            Some(Some(nearest.clamp(shortest, tick_c - 1)))
                        }
                    }
                };
                if let Some(target) = retry {
                    self.restore(&c);
                    self.release(c);
                    rejections += 1;
                    if rejections >= self.options.max_rejections {
                        return Err(Error::simple(
                            SolverDomain::Transient,
                            format!(
                                "{rejections} consecutive rejected attempts from t = {:e} s (last toward {t_c:e} s): \
                                 the step does not converge",
                                self.now
                            ),
                        ));
                    }
                    forced = target;
                    continue;
                }
                // Step 5: accept, fire the crossings at (t_c, 0).
                let accepted = self.analog.accept().map_err(|e| at(t_c, e))?;
                rejections = 0;
                self.exempt_next = false;
                for net in &accepted.fired {
                    self.discrete.fire(*net, tick_c).map_err(|e| at(t_c, e))?;
                }
                accepted.samples
            } else {
                Vec::new()
            };
            self.samples.clone_from(&samples);
            let mode = InstantMode { is_static: false, settle };
            let crossing = if unknowns { tick_c } else { stop_tick };
            let limit = Horizon { stop: stop_tick, crossing };
            self.land_settle_commits(tick_c, settle);
            let outcome = self.run_whole_instant(tick_c, &samples, limit, mode).map_err(|e| at(t_c, e))?;
            if unknowns && !outcome.boundary.is_empty() {
                let restart = if outcome.host_param_write { Restart::IntegratorSmall } else { Restart::Integrator };
                self.analog.deliver(&outcome.boundary, restart);
            }
            self.deliver_settle_boundary(unknowns, &outcome);
            // Step 6.
            self.collect(AnalysisKind::Transient);
            self.delivery.flush(t_c);
            self.observe_step(t_c);
            self.now = t_c;
            self.now_tick = tick_c;
            self.release(c);
            if final_mark || outcome.finish {
                self.finished = Some(t_c);
                return Ok(Ended::Finish(t_c));
            }
        }
        Ok(Ended::StopTime)
    }
}

impl<D: DiscreteSide, A: AnalogSide> Protocol<D, A> {
    /// §15 S1: the accepted instant `tick` lands the settle commits the last time point held, due
    /// at its microstep 0. Only an instant that is a time point (`settle`) lands them: a forced
    /// landing that is not one runs on the old value (S4; abi.md R6), as does every instant the
    /// step skipped.
    fn land_settle_commits(&mut self, tick: Tick, time_point: bool) {
        if time_point {
            self.discrete.land_settle_commits(tick);
        }
    }

    /// §15 S2 and S3: the settle commits the instant held reach the analog side now, so the solve
    /// that arrives at the next time point reads them; the integrator restarts from them, and that
    /// arriving step is exempt from the truncation test (`exempt_next`).
    pub(super) fn deliver_settle_boundary(&mut self, unknowns: bool, outcome: &Outcome) {
        if unknowns && !outcome.settle_boundary.is_empty() {
            self.analog.deliver(&outcome.settle_boundary, Restart::Integrator);
            self.exempt_next = true;
        }
    }

    /// §15 S6: a settle commit made at the stop time is dropped, and no step follows to read it.
    fn drop_settle_commits_at_stop(&mut self) {
        self.end_sweep();
    }
}
