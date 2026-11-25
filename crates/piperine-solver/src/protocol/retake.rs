//! What the analog side reads at a time point `t` after the step that arrived there (docs/spec
//! §15.2 rule 4), round by round, in one loop:
//!
//! - a crossing made by a jump fires after the step that read the jump, and when its reactions
//!   change what `analog` reads, that step is taken again to `t` with the change (a **retake**);
//! - a value `analog` reads that passes within the instant (held at the end of one microstep and
//!   lost at a later one, as a pulse is) is read by a **zero-width solve** at `t`: a backward-Euler
//!   step of width ε = 1e-6 tick, so every `ddt` state moves by at most ε times its derivative,
//!   far below any tolerance, and no integrand gains area a tolerance sees; a read that forces a state (an
//!   `idt` whose `reset` pulses, the seat of `idtmod`'s wrap) takes effect there. After a
//!   zero-width solve, a change the next round makes is read by another one, so the earlier
//!   reading is never undone.
//!
//! One solution per time point: each solution replaces the one before it at `t`, the reactions that
//! ran are not run again, and a crossing only the new solution makes fires at the next microstep of
//! the instant (its numbering goes on) and may read again, up to the round bound (`E0504`).

use piperine_analog_solver::error::{Error, Result, RunCode, SolverDomain};

use super::sides::{Accepted, AnalogSide, AnalysisKind, Attempt, Boundary, DiscreteSide, Horizon, InstantMode, Outcome, Restart, Tick};
use super::statistics::ZeroWidthCause;
use super::transient::Protocol;

/// Whether the step being taken reads a jump: the previous instant of the transient delivered a
/// change the analog side reads. It is the owner's whole memory of AO-13 between two steps.
#[derive(Clone, Copy, Debug)]
pub(super) struct JumpRetake {
    reads_jump: bool,
}

impl JumpRetake {
    /// A transient's start: its first step reads a jump when the operating point it follows holds
    /// a settle commit (`settle_pending`: that step reads a jump, §15 S3), and none otherwise.
    pub(super) fn new(settle_pending: bool) -> JumpRetake {
        JumpRetake { reads_jump: settle_pending }
    }

    /// Records what the instant just run delivered: the next step reads a jump when the instant
    /// changed something `analog` reads (`boundary`), or its settle phase holds a commit the next
    /// solve reads (`settle_boundary`, §15 S2 and S3: a change due before that solve).
    pub(super) fn delivered(&mut self, boundary: &[Boundary], settle_boundary: &[Boundary]) {
        self.reads_jump = !boundary.is_empty() || !settle_boundary.is_empty();
    }

    /// Whether the step just accepted read a jump.
    pub(super) fn reads_jump(&self) -> bool {
        self.reads_jump
    }

    /// Whether the instant after the accepted step that made the crossings `fired` may have to
    /// retake it: the step read a jump and a crossing of its own fired. The owner then defers the
    /// settle phase of the instant until the retakes are over.
    pub(super) fn possible(&self, fired: &[u32]) -> bool {
        self.reads_jump && !fired.is_empty()
    }

    /// Whether the instant after the accepted step `fired` crossings must retake that step: it
    /// read a jump, a crossing of its own fired, and the instant changed what `analog` reads.
    pub(super) fn wanted(&self, fired: &[u32], outcome: &Outcome) -> bool {
        self.possible(fired) && !outcome.boundary.is_empty()
    }
}

/// What one round reads at `t`.
enum Reading {
    /// The step to `t` read a jump, a crossing of its own fired and the instant changed what
    /// `analog` reads: the step is retaken to `t` with every change read so far.
    Retake,
    /// Values passed through, or a change made after a zero-width solve: read by a zero-width
    /// solve at `t`, for this cause.
    ZeroWidth(Vec<Boundary>, ZeroWidthCause),
}

/// What one round of the instant changed, passed through and fired.
struct Round {
    /// The changes of what `analog` reads, not read yet.
    boundary: Vec<Boundary>,
    /// The values passed through (`Outcome::passing`).
    passing: Vec<Boundary>,
    /// The crossings the solve before the round made.
    made: Vec<u32>,
}

/// The readings of one instant so far.
struct Readings {
    /// The last solution at `t` read a jump.
    reads_jump: bool,
    /// A zero-width solve replaced the solution at `t`.
    zero_width: bool,
    /// The changes the retaken step reads.
    retaken: Vec<Boundary>,
    /// Every change of the instant, in order: what the first step after `t` reads after a
    /// zero-width solve, which leaves the analog copy at the values passed through.
    changed: Vec<Boundary>,
    /// The solves made at `t` after the first.
    rounds: u32,
}

impl Readings {
    /// The readings of an instant whose step read a jump or not (`reads_jump`) and changed `boundary`.
    fn new(reads_jump: bool, boundary: &[Boundary]) -> Readings {
        Readings { reads_jump, zero_width: false, retaken: Vec::new(), changed: boundary.to_vec(), rounds: 0 }
    }

    /// What `round` asks to be read at `t`, if anything: what it passed through, else rule 4's
    /// jump case (the last solution read a jump, made a crossing, and the round changed what
    /// `analog` reads), by a retake until a zero-width solve was made and by a zero-width solve after.
    fn next(&self, round: &mut Round) -> Option<Reading> {
        if !round.passing.is_empty() {
            return Some(Reading::ZeroWidth(std::mem::take(&mut round.passing), ZeroWidthCause::PassedValue));
        }
        if !self.reads_jump || round.made.is_empty() || round.boundary.is_empty() {
            return None;
        }
        Some(if self.zero_width { Reading::ZeroWidth(round.boundary.clone(), ZeroWidthCause::CrossingChange) } else { Reading::Retake })
    }

    /// What the first step after `t` reads, the last round left `unread`: every change of the
    /// instant after a zero-width solve, else what no retake read.
    fn left(self, unread: Vec<Boundary>) -> Vec<Boundary> {
        if self.zero_width { self.changed } else { unread }
    }
}

impl<D: DiscreteSide, A: AnalogSide> Protocol<D, A> {
    /// Reads at `tick` what the instant run after the accepted step changed or passed through,
    /// round by round (rule 4). `mark` is the analog checkpoint of the step, `fired` the crossing
    /// nets of its solution, `first` the outcome of the instant, `reads_jump` whether the step read
    /// a jump; `mode` runs each continued instant, without its settle phase (the caller runs it
    /// after). Returns the outcome the instant ends with: the finish and host writes of every round,
    /// `passing` empty, and in `boundary` what the first step after `tick` reads.
    pub(super) fn read_instant(
        &mut self,
        mark: &A::Mark,
        tick: Tick,
        mut fired: Vec<u32>,
        first: Outcome,
        reads_jump: bool,
        limit: Horizon,
        mode: InstantMode,
    ) -> Result<Outcome> {
        let mut whole = first;
        let mut round =
            Round { boundary: std::mem::take(&mut whole.boundary), passing: std::mem::take(&mut whole.passing), made: fired.clone() };
        let mut readings = Readings::new(reads_jump, &round.boundary);
        while let Some(reading) = readings.next(&mut round) {
            readings.rounds += 1;
            if readings.rounds > self.options.max_rounds {
                return Err(self.too_many_retakes(self.time(tick), &readings.changed));
            }
            let accepted = self.read_round(mark, tick, reading, &mut readings, &mut round)?;
            let made: Vec<u32> = accepted.fired.iter().copied().filter(|net| !fired.contains(net)).collect();
            self.samples = accepted.samples;
            if made.is_empty() {
                break;
            }
            // the crossings fire at the next microstep of `t`, and the instant goes on from there
            self.discrete.reopen(tick, true);
            for net in &made {
                self.discrete.fire(*net, tick)?;
            }
            fired.extend(&made);
            let samples = self.samples.clone();
            let ran = self.run_whole_instant(tick, &samples, limit, mode)?;
            whole.finish |= ran.finish;
            whole.host_param_write |= ran.host_param_write;
            readings.changed.extend(ran.boundary.iter().cloned());
            round = Round { boundary: ran.boundary, passing: ran.passing, made };
        }
        whole.boundary = readings.left(round.boundary);
        Ok(whole)
    }

    /// Solves at `tick` once more, as `reading` says, and accepts the solution in place of the one
    /// there: the step retaken on every change read so far, or a zero-width solve on the values.
    fn read_round(
        &mut self,
        mark: &A::Mark,
        tick: Tick,
        reading: Reading,
        readings: &mut Readings,
        round: &mut Round,
    ) -> Result<Accepted> {
        let time = self.time(tick);
        let accepted = match reading {
            Reading::Retake => {
                readings.retaken.extend(std::mem::take(&mut round.boundary));
                self.analog.retake(mark);
                self.analog.deliver(&readings.retaken, Restart::Integrator);
                let attempt = self.analog.attempt(self.offset(tick), true)?;
                self.keep_analog_traces();
                if attempt != Attempt::Converged {
                    return Err(self.retake_failed(time, &readings.retaken));
                }
                self.analog.accept()?
            }
            Reading::ZeroWidth(values, cause) => {
                self.analog.deliver(&values, Restart::Integrator);
                let solved = self.analog.zero_width()?;
                self.keep_analog_traces();
                self.statistics.record_zero_width(cause);
                readings.zero_width = true;
                solved.ok_or_else(|| self.zero_width_failed(time, &values))?
            }
        };
        readings.reads_jump = true;
        Ok(accepted)
    }

    /// Keeps the trace records of the solve just made.
    fn keep_analog_traces(&mut self) {
        let analog = &mut self.analog;
        self.delivery.keep_traces(AnalysisKind::Transient, |out| analog.drain_traces(out));
    }

    /// The `E0504` of an instant whose crossings or passing values retake the step more often than the round bound.
    fn too_many_retakes(&self, time: f64, delivered: &[Boundary]) -> Error {
        Error::run(
            RunCode::E0504,
            SolverDomain::Transient,
            format!(
                "the instant at t = {time:e} s retakes its step more than {} times; the nets changed: {}",
                self.options.max_rounds,
                self.changed_nets(delivered)
            ),
        )
        .with_time(time)
    }

    /// The `E0504` of a zero-width solve that does not converge.
    fn zero_width_failed(&self, time: f64, read: &[Boundary]) -> Error {
        Error::run(
            RunCode::E0504,
            SolverDomain::Transient,
            format!("the zero-width solve at t = {time:e} s, reading {}, does not converge", self.changed_nets(read)),
        )
        .with_time(time)
    }

    /// The `E0504` of a retaken step that does not converge.
    fn retake_failed(&self, time: f64, delivered: &[Boundary]) -> Error {
        Error::run(
            RunCode::E0504,
            SolverDomain::Transient,
            format!(
                "the step retaken at t = {time:e} s, after its instant changed {}, does not converge",
                self.changed_nets(delivered)
            ),
        )
        .with_time(time)
    }

    /// The names of the nets `boundary` changed, each once, in order.
    fn changed_nets(&self, boundary: &[Boundary]) -> String {
        let mut names: Vec<String> = Vec::new();
        for change in boundary {
            let name = self.discrete.net_name(change.slot);
            if !names.contains(&name) {
                names.push(name);
            }
        }
        names.join(", ")
    }
}
