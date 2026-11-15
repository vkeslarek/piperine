//! The transient protocol of abi.md §5 over the two sides:
//!
//! ```text
//! C := checkpoint(discrete, analog, sink buffer)                 at accepted time t
//! 1. t_c := analog.propose(t, stop)           (the stop time when there is no unknown)
//! 2. discrete.advance(tick(t_c)): a Landing or Final shortens t_c to its tick
//! 3. deliver the boundary preview of t_c; attempt to t_c, exempt when it changed a value
//! 4. Rejected: restore C, propose again.  LandedEarlier(t'): restore C, t_c := t', go to 2
//! 5. accept (with the restart the previous instant asked for); fire the crossings at
//!    (t_c, 0); run the instant t_c with the samples, settling when it is a time point;
//!    a later boundary change asks the next acceptance for a restart
//! 6. t := t_c; a Final mark or a FINISH ends the analysis after this instant
//! ```
//!
//! What a restore replays is discrete work only; one analog solve per landing.

use piperine_analog_solver::error::{Error, Result, SolverDomain};

use super::sides::{Advance, AnalogSide, AnalysisKind, Attempt, DiscreteSide, InstantMode, Outcome, Restart, Tick};
use super::sink::{Delivery, Sink};

/// How the protocol maps time and bounds its loops.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProtocolOptions {
    /// The session resolution: seconds per tick.
    pub seconds_per_tick: f64,
    /// The bound on the equilibrium solves of an operating point (`E0506`).
    pub max_rounds: u32,
}

impl ProtocolOptions {
    /// Options with the default bound of 20 rounds.
    pub fn new(seconds_per_tick: f64) -> ProtocolOptions {
        ProtocolOptions { seconds_per_tick, max_rounds: 20 }
    }
}

/// How a transient ended.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ended {
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
    pub(super) in_transient: bool,
    /// The restart the next acceptance passes to the analog side.
    pub(super) restart: Restart,
    pub(super) delivery: Delivery,
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
            in_transient: false,
            restart: Restart::None,
            delivery: Delivery::default(),
        }
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

    pub(super) fn time(&self, tick: Tick) -> f64 {
        tick as f64 * self.options.seconds_per_tick
    }

    fn checkpoint(&self) -> Checkpoint<D::Mark, A::Mark> {
        Checkpoint { discrete: self.discrete.mark(), analog: self.analog.mark(), sink: self.delivery.mark() }
    }

    fn restore(&mut self, c: &Checkpoint<D::Mark, A::Mark>) {
        self.discrete.restore(&c.discrete);
        self.analog.restore(&c.analog);
        self.delivery.restore(c.sink);
    }

    /// Buffers what the discrete side observed.
    pub(super) fn collect(&mut self, analysis: AnalysisKind) {
        let discrete = &mut self.discrete;
        self.delivery.collect(analysis, |out| discrete.drain(out));
    }

    /// Runs the instant `at` to its end.
    pub(super) fn run_whole_instant(&mut self, at: Tick, samples: &[f64], limit: Tick, mode: InstantMode) -> Result<Outcome> {
        self.discrete.run_instant(at, samples, limit, mode)
    }

    /// Runs a transient from the last accepted time (after `operating_point`)
    /// to `stop`.
    pub fn transient(&mut self, stop: f64) -> Result<Ended> {
        self.in_transient = true;
        let ended = self.transient_loop(stop);
        self.in_transient = false;
        ended
    }

    fn transient_loop(&mut self, stop: f64) -> Result<Ended> {
        let stop_tick = self.tick(stop);
        let at = |t: f64, e: Error| Error::wrap(SolverDomain::Transient, format!("at t = {t:e} s"), e);
        let mut forced: Option<f64> = None;
        while self.now < stop {
            let c = self.checkpoint();
            let unknowns = self.analog.has_unknowns();
            let mut t_c = match forced.take() {
                Some(t) => t,
                None if unknowns => self.analog.propose(self.now, stop).min(stop),
                None => stop,
            };
            if t_c <= self.now {
                return Err(Error::simple(
                    SolverDomain::Transient,
                    format!("the proposed step to {t_c:e} s does not advance from {:e} s", self.now),
                ));
            }
            // Step 2: the discrete instants before t_c, stopping before a landing.
            let (mut settle, mut final_mark) = (true, false);
            match self.discrete.advance(self.tick(t_c)).map_err(|e| at(t_c, e))? {
                Advance::Reached => {}
                Advance::Landing { tick, time_point } => {
                    t_c = self.time(tick);
                    settle = time_point;
                }
                Advance::Final(tick) => {
                    t_c = self.time(tick);
                    final_mark = true;
                }
            }
            self.collect(AnalysisKind::Transient);
            let tick_c = self.tick(t_c);

            // Steps 3 and 4: one analog attempt to t_c.
            let samples = if unknowns {
                let preview = self.discrete.preview_boundary(tick_c).map_err(|e| at(t_c, e))?;
                let exempt = !preview.is_empty();
                if exempt {
                    self.analog.deliver(&preview);
                }
                let attempt = self.analog.attempt(t_c, exempt).map_err(|e| at(t_c, e))?;
                let analog = &mut self.analog;
                self.delivery.deliver_now(AnalysisKind::Transient, |out| analog.drain_traces(out));
                match attempt {
                    Attempt::Converged => {}
                    Attempt::Rejected => {
                        self.restore(&c);
                        continue;
                    }
                    Attempt::LandedEarlier(t) => {
                        self.restore(&c);
                        forced = Some(t);
                        continue;
                    }
                }
                // Step 5: accept, fire the crossings at (t_c, 0).
                let accepted = self.analog.accept(std::mem::take(&mut self.restart)).map_err(|e| at(t_c, e))?;
                for net in &accepted.fired {
                    self.discrete.fire(*net, tick_c).map_err(|e| at(t_c, e))?;
                }
                accepted.samples
            } else {
                Vec::new()
            };
            let mode = InstantMode { is_static: false, settle };
            let outcome = self.run_whole_instant(tick_c, &samples, stop_tick, mode).map_err(|e| at(t_c, e))?;
            if unknowns && !outcome.boundary.is_empty() {
                self.analog.deliver(&outcome.boundary);
                self.restart = if outcome.host_param_write { Restart::IntegratorSmall } else { Restart::Integrator };
            }
            // Step 6.
            self.collect(AnalysisKind::Transient);
            self.delivery.flush();
            self.now = t_c;
            if final_mark || outcome.finish {
                return Ok(Ended::Finish(t_c));
            }
        }
        Ok(Ended::StopTime)
    }
}
