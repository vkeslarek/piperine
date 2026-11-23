//! The seam between the owner and its two sub-solvers (abi.md §5, §10): the
//! traits `DiscreteSide` and `AnalogSide` and the plain data they exchange.
//! Every type in a signature is a number, bytes, a `String` or defined here, so
//! neither sub-solver depends on the owner; `solver-abi-integration` writes one
//! thin adapter per side.

use piperine_analog_solver::error::Result;

/// A discrete instant's time in session ticks.
pub type Tick = i64;

/// A discrete value the analog side reads, changed: `slot` indexes the analog
/// side's own copy of the discrete pins its functions read (abi.md §10
/// "its own copy, updated only by boundary events").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Boundary {
    pub slot: u32,
    pub bytes: Vec<u8>,
}

/// How far `DiscreteSide::advance` got (abi.md §5 step 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Advance {
    /// Every due instant strictly before the target ran; none was a landing.
    Reached,
    /// The first instant at `tick` must be a landing: R4 makes it a time point
    /// (`time_point = true`) or it fires an `IRREVERSIBLE` reaction, a host
    /// waiter included (`time_point = false`). That instant is untouched (the
    /// side restored its sub-checkpoint `c_s`); every instant before it ran.
    Landing { tick: Tick, time_point: bool },
    /// A reaction at `tick` returned `FINISH`: the step lands there and the
    /// analysis ends after that instant, which is a time point. As for
    /// `Landing`, the instant itself is untouched.
    Final(Tick),
}

/// The phase an instant runs in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InstantMode {
    /// An instant of an equilibrium (`STATIC` reads 1).
    pub is_static: bool,
    /// Fire `SETTLE` once the microstep loop is quiet (the instant is a time point).
    pub settle: bool,
}

/// The latest ticks a reaction's `HORIZON` may reach at an instant (abi.md R5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Horizon {
    /// The stop time: the bound when nothing queued reaches the reaction.
    pub stop: Tick,
    /// The bound for a reaction a `CROSS` watch can reach: the current instant
    /// while the analog side has unknowns (a crossing may fire in the very next
    /// step, which is not proposed yet), the stop time when it has none.
    pub crossing: Tick,
}

/// What running (or resuming) an instant did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// The values the analog side reads that the instant changed, microstep 0
    /// included: the solve at the instant read the value before it (the left
    /// value), so every change reaches the analog side after that solve.
    pub boundary: Vec<Boundary>,
    /// One of those changes came from a host write to a `PARAM` or `GLOBAL` net.
    pub host_param_write: bool,
    /// A reaction returned `FINISH`.
    pub finish: bool,
    /// Event nets with a host waiter (`DiscreteSide::wait`) that fired in the
    /// last microstep. When not empty the instant is suspended: the owner calls
    /// the waiters, queues their writes at this tick (the next microstep) and
    /// calls `run_instant` again at the same tick to continue it (abi.md R7).
    pub host_events: Vec<u32>,
    /// The values the analog side reads that the settle commits held by the instant will give: the
    /// solve that arrives at the next time point reads them (§15 S2). Not yet applied on the
    /// discrete side, so no `boundary` entry repeats them.
    pub settle_boundary: Vec<Boundary>,
}

impl Outcome {
    /// Joins the outcome `settled` of the settle phase run after the instant's rounds: what the
    /// phase changed and finished is added, and the settle commits it reports are the ones held.
    pub(super) fn absorb_settle(&mut self, settled: Outcome) {
        self.boundary.extend(settled.boundary);
        self.host_param_write |= settled.host_param_write;
        self.finish |= settled.finish;
        self.settle_boundary = settled.settle_boundary;
    }
}

/// The discrete side's answer to a host write checked on a shadow (abi.md P6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Shadow {
    /// Commit it (also the answer for a net outside the parameter cone).
    Accept,
    /// A `check` pin of the written param came out false.
    Refused { check: String },
    /// A function of the cone returned `RESHAPE` for `instance`.
    Reshape { instance: String },
}

/// The result of one analog attempt (abi.md §5 step 3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Attempt {
    Converged,
    /// The step failed (Newton or truncation): propose again from the last acceptance.
    Rejected,
    /// A located crossing or a shortened step: the step must land at this time
    /// instead. The protocol lands on the nearest tick, at least one tick short
    /// of the target; when the time is at or past the target, or rounds to the
    /// end of a one-tick step, it takes the step at its end and calls `accept`,
    /// which then reports the crossing in `Accepted::fired`. Every retry counts
    /// toward `ProtocolOptions::max_rejections`.
    LandedEarlier(f64),
}

/// How the integrator restarts after a delivered change (the kept
/// discontinuity handling, solver-completeness Q7).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Restart {
    /// No integration follows (an operating-point round).
    #[default]
    None,
    /// A value the analog side reads jumped at the accepted time: the next
    /// step starts first-order from the jump, as at a breakpoint.
    Integrator,
    /// The jump came from a host write to a `PARAM` or `GLOBAL` net: restart,
    /// and the next step is at most 1e-3 of the last one.
    IntegratorSmall,
}

/// An accepted analog solution, as the discrete side needs it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Accepted {
    /// The event pins of the `CROSS` watches that fired (abi.md §4.3, C6).
    pub fired: Vec<u32>,
    /// The continuous values reactions read, in the discrete side's order.
    pub samples: Vec<f64>,
}

/// The analysis an observation belongs to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnalysisKind {
    #[default]
    OperatingPoint,
    Transient,
}

/// One thing the sink sees (abi.md C5, A5, §12 "Watch it"). The sides fill
/// `path`, `time` and `kind`; the protocol stamps `analysis`.
#[derive(Clone, Debug, PartialEq)]
pub struct Observation {
    pub path: String,
    pub analysis: AnalysisKind,
    pub time: f64,
    pub kind: ObservationKind,
}

/// What was observed.
#[derive(Clone, Debug, PartialEq)]
pub enum ObservationKind {
    /// A net changed to these bytes.
    Change { bytes: Vec<u8> },
    /// A commit, with the reaction and the statement that made it (abi.md C5) and the instants
    /// it was made and is due.
    Commit { reaction: u32, statement: u32, made: Tick, due: Tick, bytes: Vec<u8> },
    /// A `CROSS` watch fired.
    Crossing,
    /// A commit to a `RECORD` pin. `site` is the reaction and the statement that made it
    /// (abi.md C5), so a record is decoded on its own, never by pairing it with a `Commit`;
    /// `None` for a record no statement made.
    Record { site: Option<(u32, u32)>, bytes: Vec<u8> },
    /// A `TRACE` watch written in Newton iteration `iteration`.
    Trace { iteration: u32, bytes: Vec<u8> },
}

/// The discrete sub-solver as the owner drives it.
pub trait DiscreteSide {
    /// A checkpoint of the discrete store, the queue and the side's buffers.
    type Mark;
    /// From initial values at tick `at` (0, or a later start time): zero, commit every `init` and
    /// the host's initial writes, fire `START`, run the discrete functions (abi.md §10 step 1).
    fn start(&mut self, at: Tick) -> Result<()>;
    /// The tick of the next queued commit, if any.
    fn next_due(&self) -> Option<Tick>;
    /// Runs the due instants strictly before `to`, stopping before a landing (abi.md §5 step 2).
    fn advance(&mut self, to: Tick) -> Result<Advance>;
    /// Commits a firing of event pin `net` due at microstep 0 of `at` (C6).
    fn fire(&mut self, net: u32, at: Tick) -> Result<()>;
    /// Reopens the instant `at` that `run_instant` just ran to its end, so the next `run_instant(at)`
    /// and a `fire` made before it continue its microstep numbering: at the microstep after its last
    /// one (`next_microstep`) or at its last one (the settle phase after the rounds, §19 Resolution).
    /// A side that does not number its microsteps does nothing.
    fn reopen(&mut self, _at: Tick, _next_microstep: bool) {}
    /// Runs (or, after a suspension, continues) the instant `at`. `samples` are
    /// the accepted continuous values reactions read; `limit` bounds each
    /// reaction's `HORIZON` (R5): `limit.crossing` for one a `CROSS` watch can
    /// reach, `limit.stop` otherwise. An `IRREVERSIBLE` reaction never runs its
    /// own clock past its bound.
    fn run_instant(&mut self, at: Tick, samples: &[f64], limit: Horizon, mode: InstantMode) -> Result<Outcome>;
    /// Makes the settle commits the last settle phases held due at microstep 0 of `tick`, the next
    /// time point the owner accepted (§15 S1), or the next sweep point (S6).
    fn land_settle_commits(&mut self, tick: Tick);
    /// Forgets the held settle commits: the stop time or the last sweep point was reached (§15 S6).
    fn drop_settle_commits(&mut self);
    /// A mark to `restore` to. It may journal what changes from here instead of copying the
    /// state, so it is taken `&mut` and ended with `release` once the step is accepted or undone.
    fn mark(&mut self) -> Self::Mark;
    /// Puts the side back as `mark` found it. The mark stays valid until `release`.
    fn restore(&mut self, mark: &Self::Mark);
    /// Ends a mark. Marks are released newest first; a side that copies its state needs nothing.
    fn release(&mut self, _mark: Self::Mark) {}
    /// Runs the parameter cone downstream of a host write on a shadow (P6).
    fn shadow(&self, net: u32, offset: u32, bytes: &[u8]) -> Result<Shadow>;
    /// Queues a host commit due at `at` (the next microstep when `at` is the
    /// current instant).
    fn host_commit(&mut self, net: u32, offset: u32, bytes: &[u8], at: Tick) -> Result<()>;
    /// The name of the net behind boundary slot `slot`, as the user reads it
    /// (the instance path and pin name), for error reports such as `E0506`.
    fn net_name(&self, slot: u32) -> String;
    /// Registers a host waiter on event net `net`: an `IRREVERSIBLE` reaction (R7).
    fn wait(&mut self, net: u32);
    /// Moves the observations made since the last drain into `out`, in instant order.
    fn drain(&mut self, out: &mut Vec<Observation>);
    /// A discrete net's bytes as it stands (`None`: the side does not expose its nets). The
    /// host's step observer reads nets through it.
    fn net(&self, _id: u32) -> Option<&[u8]> {
        None
    }
}

/// The analog sub-solver as the owner drives it.
pub trait AnalogSide {
    /// A checkpoint of the solution and the histories. The step reduction a
    /// rejected attempt made is not part of it: it survives `restore`, so the
    /// next `propose` after a rejection is smaller (the protocol also caps
    /// consecutive rejections, `ProtocolOptions::max_rejections`).
    type Mark;
    /// Whether any continuous unknown exists.
    fn has_unknowns(&self) -> bool;
    /// The next step's end (step control, `CAP` watches), at most `stop`. `next_due` is the next
    /// scheduled time point in seconds (the discrete side's next queued commit), when there is one.
    fn propose(&mut self, now: f64, stop: f64, next_due: Option<f64>) -> f64;
    /// Updates the analog copy of discrete values after the accepted solution
    /// at the instant that changed them; `restart` says how the next step starts.
    fn deliver(&mut self, boundary: &[Boundary], restart: Restart);
    /// Solves a step to `to` on the current copy of discrete values (the left
    /// value at a scheduled instant); `exempt` skips the truncation test (a
    /// step that lands on a scheduled instant, as at a breakpoint).
    fn attempt(&mut self, to: f64, exempt: bool) -> Result<Attempt>;
    /// Accepts the last converged attempt.
    fn accept(&mut self) -> Result<Accepted>;
    /// Solves the equilibrium (`STATIC = 1`) and evaluates watches there.
    fn equilibrium(&mut self) -> Result<Accepted>;
    fn mark(&self) -> Self::Mark;
    fn restore(&mut self, mark: &Self::Mark);
    /// Takes back the last `accept` and puts the side back as `mark` found it, so the same step
    /// can be attempted again after a delivery (§15.2 rule 4). A side whose `accept` keeps nothing
    /// beyond what `restore` undoes needs no more than the default.
    fn retake(&mut self, mark: &Self::Mark) {
        self.restore(mark);
    }
    /// Moves the trace records of the attempts since the last drain into `out`.
    fn drain_traces(&mut self, out: &mut Vec<Observation>);
    /// The solution as it stands, indexed as `AnalogReference::idx` (empty: the side does not
    /// expose it). The host's step observer reads unknowns through it.
    fn solution(&self) -> &[f64] {
        &[]
    }
    /// The time of the last accepted step as the side keeps it (`None`: it keeps none, and the
    /// owner's own time of the step stands). The two differ in the last bit, since the owner's
    /// is a whole number of ticks and the side's is a sum of its steps.
    fn step_time(&self) -> Option<f64> {
        None
    }
}
