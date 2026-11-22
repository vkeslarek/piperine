//! The facade of the ABI engine: one value, `Digital`, owning the store, the
//! queue and the observation buffer of a bound plan. Its methods mirror the
//! discrete side of the owner protocol one to one (abi.md §5, §10).

use crate::net::{NetId, NetKind};
use crate::observe::{Obs, Observation};
use piperine_abi::raw::pin_flag;

use crate::plan::{Binding, DigitalPlan, NetWriter};
use crate::queue::{Pending, Queue, QueueMark, Writer};
use crate::run::{Ambient, DirtySet, run_cone, write_net};
use crate::settle::SettleCommits;
use crate::store::{Store, StoreMark};
use crate::tick::Tick;
use crate::error::DigitalError;
use crate::landing::TimePointRule;
use crate::limits::DigitalLimits;

/// The latest ticks a reaction's `HORIZON` may reach at an instant (abi.md R5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Horizon {
    /// The stop time: the bound when nothing queued reaches the reaction.
    pub stop: Tick,
    /// The bound for a reaction a `CROSS` watch can reach: the current instant while the analog side
    /// has unknowns, the stop time when it has none.
    pub crossing: Tick,
}

impl Horizon {
    /// The same bound for every reaction.
    pub fn uniform(t: Tick) -> Horizon {
        Horizon { stop: t, crossing: t }
    }
}

/// How an instant runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mode {
    /// An instant of an equilibrium (`STATIC` reads 1).
    pub is_static: bool,
    /// Fire `SETTLE` once the microstep loop is quiet.
    pub settle: bool,
}

/// The answer to a host write checked on a shadow of the store (abi.md P6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Shadow {
    /// Commit it (also the answer for a net outside the parameter cone).
    Accept,
    /// A `check` pin of the written param came out false: its path.
    Refused { check: String },
    /// A function of the cone returned `RESHAPE`: the instance's path.
    Reshape { instance: String },
}

/// What running an instant did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// The nets the analog side reads that the instant changed, with their bytes after it.
    pub boundary: Vec<(NetId, Vec<u8>)>,
    /// The event nets that fired in the instant.
    pub fired: Vec<NetId>,
    /// A host write to a `PARAM` or `GLOBAL` net applied.
    pub host_param_write: bool,
    /// A reaction returned `FINISH`.
    pub finish: bool,
    /// How many microsteps ran.
    pub microsteps: u32,
    /// Event nets with a host waiter (`Digital::wait`) that fired in the last microstep run. When
    /// not empty the instant is suspended: host commits made now are due at its next microstep,
    /// and the next `run_instant` at the same tick continues it (abi.md §5 R7).
    pub host_events: Vec<NetId>,
    /// The nets the analog side reads and the bytes the held settle commits will give them: the
    /// solve that arrives at the next time point reads these (§15 S2).
    pub settle_boundary: Vec<(NetId, Vec<u8>)>,
}

/// The engine's state at one point: the store, the queue, how many observations existed, and the
/// bookkeeping that follows them. Taking and restoring one calls no device (abi.md §5 R6, §9).
#[derive(Clone, Debug, PartialEq)]
pub struct Checkpoint {
    store: Store,
    queue: Queue,
    dirty: DirtySet,
    observed: usize,
    now: Tick,
    call: u32,
    running: bool,
    suspended: Option<(Tick, u32)>,
    /// The settle commits held when it was taken: a restore brings them back, so a rejected step
    /// keeps them for the retried time point and a landing it made is taken back (§15 S5).
    settle_commits: SettleCommits,
}

/// A place of the engine to rewind to without a copy of its state (HYG-14): the journal
/// positions of the store and the queue, and the small bookkeeping. Taking one opens a journal
/// that `Digital::release` closes; marks nest and are released newest first.
#[derive(Clone, Debug)]
pub struct Mark {
    store: StoreMark,
    queue: QueueMark,
    dirty: DirtySet,
    observed: usize,
    now: Tick,
    call: u32,
    running: bool,
    suspended: Option<(Tick, u32)>,
    /// The settle commits held when it was taken: a restore brings them back, so a rejected step
    /// keeps them for the retried time point and a landing it made is taken back (§15 S5).
    settle_commits: SettleCommits,
}

impl Checkpoint {
    /// The tick of the last instant entered when it was taken.
    pub fn now(&self) -> Tick {
        self.now
    }
}

/// The discrete engine over a bound plan.
#[derive(Debug)]
pub struct Digital {
    pub(crate) plan: DigitalPlan,
    pub(crate) limits: DigitalLimits,
    /// Which scheduled instants a look-ahead lands on.
    pub(crate) rule: TimePointRule,
    pub(crate) store: Store,
    pub(crate) queue: Queue,
    pub(crate) obs: Obs,
    pub(crate) dirty: DirtySet,
    /// The tick of the last instant entered.
    pub(crate) now: Tick,
    /// The commits of the settle phase held for the next time point (§15 S1).
    pub(crate) settle_commits: SettleCommits,
    /// Numbers the commits of a microstep (abi.md §6 C2).
    pub(crate) call: u32,
    pub(crate) running: bool,
    /// Event nets a host waiter listens to (abi.md §5 R7): an `IRREVERSIBLE` reaction of the host's.
    pub(crate) waited: Vec<NetId>,
    /// An instant suspended for the host's waiters: its tick and the microstep it continues at.
    pub(crate) suspended: Option<(Tick, u32)>,
    /// The instant the last `run_instant` ran to its end and the last microstep it ran, for `reopen`.
    pub(crate) last_run: Option<(Tick, u32)>,
    /// `GMIN` and `SOURCE_SCALE` as reactions and discrete functions read them.
    pub(crate) continuation: (f64, f64),
    /// The full copies of the state `checkpoint` has taken (a diagnostic).
    pub(crate) checkpoints_taken: std::sync::atomic::AtomicU64,
}

impl Digital {
    /// An engine over `plan`; `rule` decides which scheduled instants a look-ahead lands on.
    pub fn new(plan: DigitalPlan, limits: DigitalLimits, rule: TimePointRule) -> Digital {
        let mut store = Store::new(&plan.nets);
        // A param holds its initial value from the start, before any run (§5.3.1): a host `set`
        // checks its `where` against the other params' values, which a zeroed store would not give.
        for (index, net) in plan.nets.iter().enumerate() {
            if net.param && !net.init.is_empty() {
                store.seed(NetId(index as u32), &net.init);
            }
        }
        let dirty = DirtySet::all(&plan);
        Digital {
            plan,
            limits,
            rule,
            store,
            queue: Queue::new(),
            obs: Obs::new(),
            dirty,
            now: Tick(0),
            settle_commits: SettleCommits::default(),
            call: 0,
            running: false,
            waited: Vec::new(),
            suspended: None,
            last_run: None,
            continuation: (0.0, 1.0),
            checkpoints_taken: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Sets the values reactions and discrete functions read as `GMIN` and `SOURCE_SCALE`
    /// (abi.md §4.2): the owner's continuation values at rest, given before `start`, where every
    /// function is evaluated. 0 and 1 until set.
    pub fn set_continuation(&mut self, gmin: f64, source_scale: f64) {
        self.continuation = (gmin, source_scale);
    }

    /// The ambient of an instant at `tick` (see `Ambient::at`), with the continuation values.
    pub(crate) fn ambient(&self, tick: Tick) -> Ambient {
        let (gmin, source_scale) = self.continuation;
        Ambient { gmin, source_scale, ..Ambient::at(tick, self.plan.resolution.seconds_per_tick) }
    }

    /// An analysis from initial values (abi.md §9): zero the store, drop the queue, commit every
    /// `init` at (0, 0) and fire `START`. No device is called; the first instant applies them. The
    /// held settle commits stay: those of the previous sweep point land at this point (§15 S6),
    /// and the owner drops them at a sweep's end (`drop_settle_commits`).
    pub fn start(&mut self) -> Result<(), DigitalError> {
        self.start_at(Tick(0))
    }

    /// `start`, with the inits committed and `START` fired at `(tick, 0)`: a run that begins later
    /// than time zero (a transient whose start time is not 0).
    pub fn start_at(&mut self, tick: Tick) -> Result<(), DigitalError> {
        self.store = Store::new(&self.plan.nets);
        self.queue = Queue::new();
        self.obs = Obs::new();
        self.dirty = DirtySet::all(&self.plan);
        self.now = tick;
        self.call = 0;
        self.running = true;
        self.suspended = None;
        for (i, net) in self.plan.nets.iter().enumerate() {
            if net.init.is_empty() {
                continue;
            }
            // A param's initial value is its value from the start, not a change (§5.3.1).
            if net.param {
                self.store.seed(NetId(i as u32), &net.init);
                continue;
            }
            self.call += 1;
            let pending = Pending {
                net: NetId(i as u32),
                offset: 0,
                bytes: net.init.clone(),
                writer: Writer::Host,
                statement: 0,
                made: (tick, 0),
                call: self.call,
                due: (tick, 0),
                flags: 0,
            };
            self.queue.push(pending, false);
        }
        for net in self.plan.analysis().start_nets().to_vec() {
            self.fire(net, tick)?;
        }
        Ok(())
    }

    /// The tick of the next thing to run: the earliest queued commit, or the current tick while
    /// functions still wait for their first evaluation.
    pub fn next_due(&self) -> Option<Tick> {
        let queued = self.queue.next_due().map(|(t, _)| t);
        match (queued, self.dirty.any()) {
            (Some(t), true) => Some(t.min(self.now)),
            (Some(t), false) => Some(t),
            (None, true) => Some(self.now),
            (None, false) => None,
        }
    }

    /// Gives the param pin `net` the value `bytes` as the value it holds from the start of the run,
    /// which a `set` before the run is (§5.3.1): no change, `PREVIOUS` the same bytes. Only for a
    /// param net, before the first instant.
    pub fn seed_param(&mut self, net: NetId, bytes: &[u8]) {
        self.store.seed(net, bytes);
    }

    /// A commit by the host, due at microstep 0 of `at` (abi.md §6 C6); while the instant `at` is
    /// suspended for the host's waiters, due at the microstep it continues at (R7).
    ///
    /// A write to a `STRUCTURAL` net while a run is in progress is refused, and nothing changes (P5).
    pub fn host_commit(&mut self, net: NetId, offset: u32, bytes: &[u8], at: Tick) -> Result<(), DigitalError> {
        let Some(spec) = self.plan.nets.get(net.0 as usize) else {
            return Err(DigitalError::HostWrite { net: format!("#{}", net.0), reason: "no such net" });
        };
        let end = offset as u64 + bytes.len() as u64;
        if end > spec.size as u64 || (spec.kind == NetKind::Event && !bytes.is_empty()) {
            return Err(DigitalError::HostWrite {
                net: spec.name.clone(),
                reason: "the bytes lie outside the net (an event net holds none)",
            });
        }
        if spec.structural && self.running {
            return Err(DigitalError::HostWrite { net: spec.name.clone(), reason: "a STRUCTURAL pin cannot change during a run" });
        }
        self.call += 1;
        let pending = Pending {
            net,
            offset,
            bytes: bytes.to_vec(),
            writer: Writer::Host,
            statement: 0,
            made: (self.now, 0),
            call: self.call,
            due: match self.suspended {
                Some((tick, microstep)) if tick == at => (at, microstep),
                _ => (at, 0),
            },
            flags: 0,
        };
        self.queue.push(pending, false);
        Ok(())
    }

    /// Registers a host waiter on event net `net` (abi.md §5 R7): the host is an `IRREVERSIBLE`
    /// reaction on it. A look-ahead stops before an instant that fires it; `run_instant` suspends
    /// that instant after the microstep it fired in and reports it in `Outcome::host_events`.
    pub fn wait(&mut self, net: NetId) {
        if !self.waited.contains(&net) {
            self.waited.push(net);
        }
    }

    /// Makes the held settle commits due at microstep 0 of `tick`, the next time point (§15 S1, S6),
    /// where `changed()` fires for what they change. The owner calls it when it accepts that time
    /// point, before it runs the instant.
    pub fn land_settle_commits(&mut self, tick: Tick) {
        self.settle_commits.land(tick, &mut self.queue);
    }

    /// Forgets the held settle commits: the stop time was reached or the last sweep point ran (§15 S6).
    pub fn drop_settle_commits(&mut self) {
        self.settle_commits.drop_all();
    }

    /// Whether a settle commit is held, waiting for the next time point.
    pub fn holds_settle_commits(&self) -> bool {
        !self.settle_commits.is_empty()
    }

    /// Reopens the instant `at` that the last `run_instant` ran to its end, so that the owner goes on
    /// with it instead of starting a fresh one at microstep 0: the next `run_instant(at)` resumes at
    /// the microstep after the last one it ran (`next_microstep`, for a crossing the owner fires there,
    /// §15.2 rule 4) or at the last one (for the settle phase, whose records are stamped `(at, last)`,
    /// §19 Resolution). A host commit or `fire` made meanwhile is due at the microstep it resumes at.
    /// An instant `at` that did not just run to its end is left alone.
    pub fn reopen(&mut self, at: Tick, next_microstep: bool) {
        if let Some((tick, last)) = self.last_run
            && tick == at
        {
            self.suspended = Some((at, if next_microstep { last + 1 } else { last }));
        }
    }

    /// Fires event net `net`: a commit due at microstep 0 of `at` (abi.md §6 C6).
    pub fn fire(&mut self, net: NetId, at: Tick) -> Result<(), DigitalError> {
        if self.plan.nets[net.0 as usize].kind != NetKind::Event {
            return Err(DigitalError::Bind {
                rule: "fire",
                what: vec![format!("net `{}` is not an event net", self.plan.nets[net.0 as usize].name)],
            });
        }
        self.host_commit(net, 0, &[], at)
    }

    /// The state to come back to with `restore`.
    pub fn checkpoint(&self) -> Checkpoint {
        self.checkpoints_taken.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Checkpoint {
            store: self.store.clone(),
            queue: self.queue.clone(),
            dirty: self.dirty.clone(),
            observed: self.obs.mark(),
            now: self.now,
            call: self.call,
            running: self.running,
            suspended: self.suspended,
            settle_commits: self.settle_commits.clone(),
        }
    }

    /// A mark to `rewind` to: the store and the queue journal what changes from here, so no net
    /// is copied. Release it with `release` once it is no longer needed.
    pub fn mark(&mut self) -> Mark {
        Mark {
            store: self.store.mark(),
            queue: self.queue.mark(),
            dirty: self.dirty.clone(),
            observed: self.obs.mark(),
            now: self.now,
            call: self.call,
            running: self.running,
            suspended: self.suspended,
            settle_commits: self.settle_commits.clone(),
        }
    }

    /// Puts the engine back as `mark` found it, as `restore` does a checkpoint: store, queue and
    /// observations equal, byte for byte, and no device called. The mark stays open.
    pub fn rewind(&mut self, mark: &Mark) {
        self.store.rewind(&mark.store);
        self.queue.rewind(&mark.queue);
        self.dirty = mark.dirty.clone();
        self.obs.truncate(mark.observed);
        self.now = mark.now;
        self.call = mark.call;
        self.running = mark.running;
        self.suspended = mark.suspended;
        self.settle_commits = mark.settle_commits.clone();
    }

    /// Ends the newest open mark; the journals empty with the outermost.
    pub fn release(&mut self, mark: Mark) {
        self.store.release(mark.store);
        self.queue.release(mark.queue);
    }

    /// How many full copies of the state `checkpoint` has taken since the engine was built (a
    /// diagnostic: a journal `mark` copies nothing and is not counted).
    pub fn checkpoints_taken(&self) -> u64 {
        self.checkpoints_taken.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// The number of nets a `rewind` would put back: those written since the oldest open mark
    /// (a diagnostic).
    pub fn journaled_nets(&self) -> usize {
        self.store.journal_len()
    }

    /// Puts the engine back as `cp` found it: store, queue and observations equal, byte for byte,
    /// and no device is called. Observations already drained stay drained.
    pub fn restore(&mut self, cp: &Checkpoint) {
        self.store = cp.store.clone();
        self.queue = cp.queue.clone();
        self.dirty = cp.dirty.clone();
        self.obs.truncate(cp.observed);
        self.now = cp.now;
        self.call = cp.call;
        self.running = cp.running;
        self.suspended = cp.suspended;
        self.settle_commits = cp.settle_commits.clone();
    }

    /// Checks a host write to a `PARAM` or `GLOBAL` net before it is committed (abi.md P6): the
    /// parameter cone downstream of the net runs on a copy of the store. A false `check` pin anywhere
    /// in the design (every dependent `where` is re-checked, §8.1.23) refuses
    /// the write and names the pin; a `RESHAPE` names the instance. Nothing of the engine changes.
    pub fn shadow(&self, net: NetId, offset: u32, bytes: &[u8]) -> Result<Shadow, DigitalError> {
        let analysis = self.plan.analysis();
        if !analysis.is_param_net(net) || analysis.writer(net) != NetWriter::Host {
            return Ok(Shadow::Accept);
        }
        let mut store = self.store.clone();
        // The functions still waiting for a first evaluation run too, and the cone of the written
        // net runs whether or not the value changed: a check reads the other params as they are
        // now, so a write of the value a param already holds is still checked against them.
        let mut dirty = self.dirty.clone();
        let mut obs = Obs::new();
        let here = (self.now, 0);
        write_net(&self.plan, &mut store, &mut dirty, &mut obs, net, offset, bytes, here);
        dirty.mark_net(&self.plan, net);
        let ambient = self.ambient(self.now);
        if let Some(instance) = run_cone(&self.plan, &mut store, &mut dirty, here, &ambient, &mut obs)? {
            return Ok(Shadow::Reshape { instance });
        }
        for inst in &self.plan.instances {
            let shape = inst.handle.shape();
            for pin in shape.pins.iter() {
                if pin.flags & pin_flag::HAS_CHECK == 0 {
                    continue;
                }
                let Some(check) = shape.pins.get(pin.check as usize) else { continue };
                if let Binding::Net(c) = inst.pins[pin.check as usize]
                    && store.value(c).first() == Some(&0)
                {
                    return Ok(Shadow::Refused { check: format!("{}.{}", inst.path, check.name.as_str()) });
                }
            }
        }
        Ok(Shadow::Accept)
    }

    /// The name of `net` as the user reads it, for error reports.
    pub fn net_name(&self, net: NetId) -> String {
        self.plan.nets[net.0 as usize].name.clone()
    }

    /// The bytes of `net` now.
    pub fn read(&self, net: NetId) -> &[u8] {
        self.store.value(net)
    }

    /// The plan this engine runs.
    pub fn plan(&self) -> &DigitalPlan {
        &self.plan
    }

    /// The observations of finished microsteps not yet drained.
    pub fn observations(&self) -> &[Observation] {
        self.obs.list()
    }

    /// Hands out every observation made since the last drain.
    pub fn drain_observations(&mut self) -> Vec<Observation> {
        self.obs.drain()
    }
}
