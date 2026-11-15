//! The facade of the ABI engine: one value, `Digital`, owning the store, the
//! queue and the observation buffer of a bound plan. Its methods mirror the
//! discrete side of the owner protocol one to one (abi.md §5, §10).

use super::net::{NetId, NetKind};
use super::observe::{Obs, Observation};
use super::plan::DigitalPlan;
use super::queue::{Pending, Queue, Writer};
use super::run::DirtySet;
use super::store::Store;
use super::tick::Tick;
use crate::error::DigitalError;
use crate::limits::DigitalLimits;

/// How an instant runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mode {
    /// An instant of an equilibrium (`STATIC` reads 1).
    pub is_static: bool,
    /// Fire `SETTLE` once the microstep loop is quiet.
    pub settle: bool,
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

    /// A commit by the host, due at microstep 0 of `at` (abi.md §6 C6).
    pub fn host_commit(&mut self, net: NetId, offset: u32, bytes: &[u8], at: Tick) -> Result<(), DigitalError> {
        self.call += 1;
        let pending = Pending {
            net,
            offset,
            bytes: bytes.to_vec(),
            writer: Writer::Host,
            statement: 0,
            made: (self.now, 0),
            call: self.call,
            due: (at, 0),
        };
        self.queue.push(pending, false);
        Ok(())
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
