//! The host's two entries into the protocol (abi.md R7, P5, P6, §10 "Host
//! actions"). A host waiter is an `IRREVERSIBLE` reaction on an event net: the
//! discrete side stops before an instant that fires it, the owner lands there,
//! runs the instant after the acceptance, calls the waiter once and queues what
//! it writes at the next microstep. A host write to a net is checked on the
//! discrete side's shadow before it is committed.

use piperine_analog_solver::error::Result;

use super::sides::{AnalogSide, AnalysisKind, DiscreteSide, Horizon, InstantMode, Outcome, Restart, Shadow, Tick};
use super::transient::Protocol;

/// What became of a host write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SetOutcome {
    /// Queued as a host commit.
    Committed,
    /// Not committed: a `check` came out false, or a `RESHAPE` inside a transient.
    Refused { reason: String },
    /// Outside a transient, the write needs `instance` re-created (abi.md P6);
    /// nothing was committed.
    Reshape { instance: String },
}

/// A host write `(net, offset, bytes)` as the protocol performs it.
type Write<'a> = dyn FnMut(u32, u32, &[u8]) -> Result<SetOutcome> + 'a;

/// What a waiter sees: the instant, the event that fired, and a way to write.
pub struct HostCx<'a> {
    time: f64,
    event: u32,
    write: &'a mut Write<'a>,
}

impl HostCx<'_> {
    /// The time of the instant.
    pub fn time(&self) -> f64 {
        self.time
    }

    /// The event net that fired.
    pub fn event(&self) -> u32 {
        self.event
    }

    /// Writes `bytes` at `offset` of `net`, due at the next microstep.
    pub fn set(&mut self, net: u32, offset: u32, bytes: &[u8]) -> Result<SetOutcome> {
        (self.write)(net, offset, bytes)
    }
}

/// A host waiter.
pub type Waiter = Box<dyn FnMut(&mut HostCx<'_>)>;


impl<D: DiscreteSide, A: AnalogSide> Protocol<D, A> {
    /// Checks a host write on the shadow, then commits it at `at`.
    pub(super) fn host_write(
        discrete: &mut D,
        in_transient: bool,
        net: u32,
        offset: u32,
        bytes: &[u8],
        at: Tick,
    ) -> Result<SetOutcome> {
        Ok(match discrete.shadow(net, offset, bytes)? {
            Shadow::Accept => {
                discrete.host_commit(net, offset, bytes, at)?;
                SetOutcome::Committed
            }
            Shadow::Refused { check } => SetOutcome::Refused { reason: format!("check {check} is false") },
            Shadow::Reshape { instance } if in_transient => {
                SetOutcome::Refused { reason: format!("instance {instance} would need another shape inside a transient") }
            }
            Shadow::Reshape { instance } => SetOutcome::Reshape { instance },
        })
    }

    /// Writes `bytes` at `offset` of `net` from the host (abi.md §10 "Host
    /// actions"): checked on the discrete side's shadow first (P6), then queued
    /// as a host commit at `at` (a scheduled write) or at the current time.
    /// A false `check` refuses it; a `RESHAPE` is refused inside a transient and
    /// reported outside one, with nothing committed.
    ///
    /// Between runs, a write due now is applied at once: the instant at the
    /// current time runs (no settle; it reads the accepted solution there) and
    /// what it changed reaches the analog side with the restart the next step
    /// starts from (the small one after a param write). The next transient
    /// then steps forward, never zero-length.
    pub fn set(&mut self, net: u32, offset: u32, bytes: &[u8], at: Option<f64>) -> Result<SetOutcome> {
        let now_tick = self.now_tick;
        let tick = at.map_or(now_tick, |t| self.tick(t)).max(now_tick);
        let outcome = Self::host_write(&mut self.discrete, self.in_transient, net, offset, bytes, tick)?;
        if outcome == SetOutcome::Committed && self.started && !self.in_transient && tick == now_tick {
            let samples = std::mem::take(&mut self.samples);
            let mode = InstantMode { is_static: false, settle: false };
            let ran = self.run_whole_instant(now_tick, &samples, Horizon { stop: now_tick, crossing: now_tick }, mode);
            self.samples = samples;
            let ran = ran?;
            if self.analog.has_unknowns() && !ran.boundary.is_empty() {
                let restart = if ran.host_param_write { Restart::IntegratorSmall } else { Restart::Integrator };
                self.analog.deliver(&ran.boundary, restart);
            }
            if ran.finish {
                self.finished = Some(self.now);
            }
            let analysis = if self.now > 0.0 { AnalysisKind::Transient } else { AnalysisKind::OperatingPoint };
            self.collect(analysis);
            self.delivery.flush(self.now);
        }
        Ok(outcome)
    }

    /// Calls `waiter` once each time event net `net` fires at an accepted instant.
    pub fn on_event(&mut self, net: u32, waiter: Waiter) {
        self.discrete.wait(net);
        self.waiters.push((net, waiter));
    }

    /// Runs the instant `at` to its end, calling the waiters of each host event
    /// it fires and queuing their writes at the next microstep.
    pub(super) fn run_whole_instant(&mut self, at: Tick, samples: &[f64], limit: Horizon, mode: InstantMode) -> Result<Outcome> {
        let mut whole = Outcome::default();
        loop {
            let part = self.discrete.run_instant(at, samples, limit, mode)?;
            whole.boundary.extend(part.boundary);
            whole.settle_boundary.extend(part.settle_boundary);
            // each part reports what the instant passed through so far: the last part's is the whole
            whole.passing = part.passing;
            whole.host_param_write |= part.host_param_write;
            whole.finish |= part.finish;
            if part.host_events.is_empty() {
                return Ok(whole);
            }
            let time = self.time(at);
            let in_transient = self.in_transient;
            let Protocol { discrete, waiters, .. } = self;
            for event in part.host_events {
                for (net, waiter) in waiters.iter_mut().filter(|(net, _)| *net == event) {
                    let mut write = |n: u32, offset: u32, bytes: &[u8]| Self::host_write(discrete, in_transient, n, offset, bytes, at);
                    waiter(&mut HostCx { time, event: *net, write: &mut write });
                }
            }
        }
    }
}
