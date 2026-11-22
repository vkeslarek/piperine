//! The retake of docs/spec §15.2 rule 4: a crossing made by a jump fires after the step that read
//! the jump, and when its reactions change what `analog` reads, that step is taken again at the
//! same time with the change. One solution per time point: the retaken solution replaces the
//! first, the reactions that ran are not run again, and a crossing only the retaken solution makes
//! fires at the next microstep and may retake again, up to the round bound (`E0504`).

use piperine_analog_solver::error::{Error, Result, RunCode, SolverDomain};

use super::sides::{AnalogSide, AnalysisKind, Attempt, Boundary, DiscreteSide, Horizon, InstantMode, Outcome, Restart, Tick};
use super::transient::Protocol;

/// Whether the step being taken reads a jump: the previous instant of the transient delivered a
/// change the analog side reads. It is the owner's whole memory of AO-13 between two steps.
#[derive(Clone, Copy, Debug)]
pub(super) struct JumpRetake {
    reads_jump: bool,
}

impl JumpRetake {
    /// A transient's start: its first step reads a jump when the operating point it follows holds
    /// a settle commit (`settle_pending`: that step is exempt, §15 S3), and none otherwise.
    pub(super) fn new(settle_pending: bool) -> JumpRetake {
        JumpRetake { reads_jump: settle_pending }
    }

    /// Records what the instant just run delivered: the next step reads a jump when the instant
    /// changed something `analog` reads (`boundary`), or its settle phase holds a commit the next
    /// solve reads (`settle_boundary`, §15 S2 and S3: a change due before that solve).
    pub(super) fn delivered(&mut self, boundary: &[Boundary], settle_boundary: &[Boundary]) {
        self.reads_jump = !boundary.is_empty() || !settle_boundary.is_empty();
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

impl<D: DiscreteSide, A: AnalogSide> Protocol<D, A> {
    /// Retakes the accepted step to `tick` while the crossings it made change what `analog` reads.
    /// `mark` is the analog checkpoint of the step, `fired` the crossing nets of its first
    /// solution and `first` the outcome of the instant run after it. Returns the outcome the
    /// instant ends with: the finish and host writes of every round, and the boundary of the last
    /// round only (the earlier ones were delivered to the retaken solutions).
    pub(super) fn retake_step(
        &mut self,
        mark: &A::Mark,
        tick: Tick,
        mut fired: Vec<u32>,
        first: Outcome,
        limit: Horizon,
        mode: InstantMode,
    ) -> Result<Outcome> {
        let time = self.time(tick);
        let mut whole = first;
        let mut delivered: Vec<Boundary> = Vec::new();
        let mut retakes = 0u32;
        loop {
            retakes += 1;
            if retakes > self.options.max_rounds {
                return Err(self.too_many_retakes(time, &delivered));
            }
            delivered.extend(std::mem::take(&mut whole.boundary));
            self.analog.retake(mark);
            self.analog.deliver(&delivered, Restart::Integrator);
            let attempt = self.analog.attempt(time, true)?;
            let analog = &mut self.analog;
            self.delivery.keep_traces(AnalysisKind::Transient, |out| analog.drain_traces(out));
            if attempt != Attempt::Converged {
                return Err(self.retake_failed(time, &delivered));
            }
            let accepted = self.analog.accept()?;
            let made: Vec<u32> = accepted.fired.iter().copied().filter(|net| !fired.contains(net)).collect();
            self.samples = accepted.samples;
            if made.is_empty() {
                return Ok(whole);
            }
            for net in &made {
                self.discrete.fire(*net, tick)?;
            }
            fired.extend(&made);
            let samples = self.samples.clone();
            let ran = self.run_whole_instant(tick, &samples, limit, mode)?;
            whole.finish |= ran.finish;
            whole.host_param_write |= ran.host_param_write;
            whole.boundary = ran.boundary;
            if whole.boundary.is_empty() {
                return Ok(whole);
            }
        }
    }

    /// The `E0504` of an instant whose crossings retake the step more often than the round bound.
    fn too_many_retakes(&self, time: f64, delivered: &[Boundary]) -> Error {
        Error::run(
            RunCode::E0504,
            SolverDomain::Transient,
            format!(
                "the crossings of the instant at t = {time:e} s retake its step more than {} times; the nets changed: {}",
                self.options.max_rounds,
                self.changed_nets(delivered)
            ),
        )
        .with_time(time)
    }

    /// The `E0504` of a retaken step that does not converge.
    fn retake_failed(&self, time: f64, delivered: &[Boundary]) -> Error {
        Error::run(
            RunCode::E0504,
            SolverDomain::Transient,
            format!(
                "the step retaken at t = {time:e} s, after its crossings changed {}, does not converge",
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
