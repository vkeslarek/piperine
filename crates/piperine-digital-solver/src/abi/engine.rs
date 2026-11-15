//! The facade of the ABI engine: one value, `Digital`, owning the store, the
//! queue and the observation buffer of a bound plan. Its methods mirror the
//! discrete side of the owner protocol one to one (abi.md §5, §10).

use super::net::{NetId, NetKind};
use super::observe::{Obs, Observation};
use piperine_abi::raw::pin_flag;

use super::plan::{Binding, DigitalPlan, NetWriter};
use super::queue::{Pending, Queue, Writer};
use super::run::{Ambient, DirtySet, run_cone, write_net};
use super::store::Store;
use super::tick::Tick;
use crate::error::DigitalError;
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
}

/// The discrete engine over a bound plan.
#[derive(Debug)]
pub struct Digital {
    pub(crate) plan: DigitalPlan,
    pub(crate) limits: DigitalLimits,
    pub(crate) store: Store,
    pub(crate) queue: Queue,
    pub(crate) obs: Obs,
    pub(crate) dirty: DirtySet,
    /// The tick of the last instant entered.
    pub(crate) now: Tick,
    /// Numbers the commits of a microstep (abi.md §6 C2).
    pub(crate) call: u32,
    pub(crate) running: bool,
    /// Event nets a host waiter listens to (abi.md §5 R7): an `IRREVERSIBLE` reaction of the host's.
    pub(crate) waited: Vec<NetId>,
    /// An instant suspended for the host's waiters: its tick and the microstep it continues at.
    pub(crate) suspended: Option<(Tick, u32)>,
}

impl Digital {
    pub fn new(plan: DigitalPlan, limits: DigitalLimits) -> Digital {
        let store = Store::new(&plan.nets);
        let dirty = DirtySet::all(&plan);
        Digital {
            plan,
            limits,
            store,
            queue: Queue::new(),
            obs: Obs::new(),
            dirty,
            now: Tick(0),
            call: 0,
            running: false,
            waited: Vec::new(),
            suspended: None,
        }
    }

    /// An analysis from initial values (abi.md §9): zero the store, drop the queue, commit every
    /// `init` at (0, 0) and fire `START`. No device is called; the first instant applies them.
    pub fn start(&mut self) -> Result<(), DigitalError> {
        self.store = Store::new(&self.plan.nets);
        self.queue = Queue::new();
        self.obs = Obs::new();
        self.dirty = DirtySet::all(&self.plan);
        self.now = Tick(0);
        self.call = 0;
        self.running = true;
        self.suspended = None;
        for (i, net) in self.plan.nets.iter().enumerate() {
            if net.init.is_empty() {
                continue;
            }
            self.call += 1;
            let pending = Pending {
                net: NetId(i as u32),
                offset: 0,
                bytes: net.init.clone(),
                writer: Writer::Host,
                statement: 0,
                made: (Tick(0), 0),
                call: self.call,
                due: (Tick(0), 0),
            };
            self.queue.push(pending, false);
        }
        for net in self.plan.analysis().start_nets().to_vec() {
            self.fire(net, Tick(0))?;
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
        Checkpoint {
            store: self.store.clone(),
            queue: self.queue.clone(),
            dirty: self.dirty.clone(),
            observed: self.obs.mark(),
            now: self.now,
            call: self.call,
            running: self.running,
            suspended: self.suspended,
        }
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
    }

    /// Checks a host write to a `PARAM` or `GLOBAL` net before it is committed (abi.md P6): the
    /// parameter cone downstream of the net runs on a copy of the store. A false `check` pin refuses
    /// the write and names the pin; a `RESHAPE` names the instance. Nothing of the engine changes.
    pub fn shadow(&self, net: NetId, offset: u32, bytes: &[u8]) -> Result<Shadow, DigitalError> {
        let analysis = self.plan.analysis();
        if !analysis.is_param_net(net) || analysis.writer(net) != NetWriter::Host {
            return Ok(Shadow::Accept);
        }
        let mut store = self.store.clone();
        let mut dirty = DirtySet::none(&self.plan);
        let mut obs = Obs::new();
        let here = (self.now, 0);
        write_net(&self.plan, &mut store, &mut dirty, &mut obs, net, offset, bytes, here);
        let ambient = Ambient::at(self.now, self.plan.resolution.seconds_per_tick);
        if let Some(instance) = run_cone(&self.plan, &mut store, &mut dirty, here, &ambient, &mut obs)? {
            return Ok(Shadow::Reshape { instance });
        }
        for inst in &self.plan.instances {
            let shape = inst.handle.shape();
            for (i, pin) in shape.pins.iter().enumerate() {
                if inst.pins[i] != Binding::Net(net) || pin.flags & pin_flag::HAS_CHECK == 0 {
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
