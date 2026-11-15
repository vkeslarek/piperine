//! Scripted stand-ins for the two sub-solvers (feature `solver-abi-core`,
//! design `protocol` "Stubs"): each side replays a plain list of answers and
//! records every call, in order, into one log shared by both sides (and by any
//! host waiter a test registers), so a test reads the protocol's call sequence.
//! An exhausted list answers with the neutral value (`Reached`, no boundary,
//! an empty `Outcome`, `Accept`, `Converged`, the stop time).

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use piperine_analog_solver::error::Result;
use piperine_solver::protocol::{
    Accepted, Advance, AnalogSide, Attempt, Boundary, DiscreteSide, Horizon, InstantMode, Observation, Outcome, Restart,
    Shadow, Tick,
};

/// One recorded call.
#[derive(Clone, Debug, PartialEq)]
pub enum Call {
    Start,
    Advance(Tick),
    Fire(u32, Tick),
    RunInstant { at: Tick, samples: Vec<f64>, limit: Horizon, mode: InstantMode },
    DiscreteMark(u32),
    DiscreteRestore(u32),
    Shadow(u32, Vec<u8>),
    HostCommit { net: u32, offset: u32, bytes: Vec<u8>, at: Tick },
    Wait(u32),
    Drain,
    Propose(f64, f64),
    Deliver(Vec<Boundary>, Restart),
    Attempt { to: f64, exempt: bool },
    Accept,
    Equilibrium,
    AnalogMark(u32),
    AnalogRestore(u32),
    DrainTraces,
    /// Pushed by a test's host waiter.
    Waiter(u32),
}

/// The horizon bounds of a `run_instant` call.
pub fn horizon(stop: Tick, crossing: Tick) -> Horizon {
    Horizon { stop, crossing }
}

/// The log both stubs write.
pub type Log = Rc<RefCell<Vec<Call>>>;

pub fn new_log() -> Log {
    Rc::new(RefCell::new(Vec::new()))
}

/// The recorded calls for which `keep` holds.
pub fn calls(log: &Log, keep: impl Fn(&Call) -> bool) -> Vec<Call> {
    log.borrow().iter().filter(|c| keep(c)).cloned().collect()
}

/// A discrete side that answers from lists.
pub struct ScriptedDiscrete {
    pub log: Log,
    pub advances: VecDeque<Advance>,
    pub instants: VecDeque<Outcome>,
    pub shadows: RefCell<VecDeque<Shadow>>,
    /// Observations made by each `advance`, in order.
    pub advance_observations: VecDeque<Vec<Observation>>,
    /// Observations made by each `run_instant`, in order.
    pub instant_observations: VecDeque<Vec<Observation>>,
    pub next: Option<Tick>,
    /// The net names behind boundary slots; an unnamed slot reads `slot <n>`.
    pub names: Vec<(u32, String)>,
    buffer: Vec<Observation>,
    marks: Cell<u32>,
}

impl ScriptedDiscrete {
    pub fn new(log: &Log) -> ScriptedDiscrete {
        ScriptedDiscrete {
            log: Rc::clone(log),
            advances: VecDeque::new(),
            instants: VecDeque::new(),
            shadows: RefCell::new(VecDeque::new()),
            advance_observations: VecDeque::new(),
            instant_observations: VecDeque::new(),
            next: None,
            names: Vec::new(),
            buffer: Vec::new(),
            marks: Cell::new(0),
        }
    }

    fn record(&self, c: Call) {
        self.log.borrow_mut().push(c);
    }
}

impl DiscreteSide for ScriptedDiscrete {
    type Mark = u32;

    fn start(&mut self) -> Result<()> {
        self.record(Call::Start);
        Ok(())
    }
    fn next_due(&self) -> Option<Tick> {
        self.next
    }
    fn advance(&mut self, to: Tick) -> Result<Advance> {
        self.record(Call::Advance(to));
        self.buffer.extend(self.advance_observations.pop_front().unwrap_or_default());
        Ok(self.advances.pop_front().unwrap_or(Advance::Reached))
    }
    fn fire(&mut self, net: u32, at: Tick) -> Result<()> {
        self.record(Call::Fire(net, at));
        Ok(())
    }
    fn run_instant(&mut self, at: Tick, samples: &[f64], limit: Horizon, mode: InstantMode) -> Result<Outcome> {
        self.record(Call::RunInstant { at, samples: samples.to_vec(), limit, mode });
        self.buffer.extend(self.instant_observations.pop_front().unwrap_or_default());
        Ok(self.instants.pop_front().unwrap_or_default())
    }
    fn mark(&self) -> u32 {
        let m = self.marks.get();
        self.marks.set(m + 1);
        self.record(Call::DiscreteMark(m));
        m
    }
    fn restore(&mut self, mark: &u32) {
        self.record(Call::DiscreteRestore(*mark));
        self.buffer.clear();
    }
    fn shadow(&self, net: u32, _offset: u32, bytes: &[u8]) -> Result<Shadow> {
        self.record(Call::Shadow(net, bytes.to_vec()));
        Ok(self.shadows.borrow_mut().pop_front().unwrap_or(Shadow::Accept))
    }
    fn host_commit(&mut self, net: u32, offset: u32, bytes: &[u8], at: Tick) -> Result<()> {
        self.record(Call::HostCommit { net, offset, bytes: bytes.to_vec(), at });
        Ok(())
    }
    fn net_name(&self, slot: u32) -> String {
        self.names.iter().find(|(s, _)| *s == slot).map(|(_, n)| n.clone()).unwrap_or_else(|| format!("slot {slot}"))
    }
    fn wait(&mut self, net: u32) {
        self.record(Call::Wait(net));
    }
    fn drain(&mut self, out: &mut Vec<Observation>) {
        self.record(Call::Drain);
        out.append(&mut self.buffer);
    }
}

/// An analog side that answers from lists.
pub struct ScriptedAnalog {
    pub log: Log,
    pub unknowns: bool,
    pub proposals: VecDeque<f64>,
    pub attempts: VecDeque<Attempt>,
    pub accepts: VecDeque<Accepted>,
    pub equilibria: VecDeque<Accepted>,
    /// Trace records produced by each `attempt`, in order.
    pub traces: VecDeque<Vec<Observation>>,
    buffer: Vec<Observation>,
    marks: Cell<u32>,
}

impl ScriptedAnalog {
    pub fn new(log: &Log) -> ScriptedAnalog {
        ScriptedAnalog {
            log: Rc::clone(log),
            unknowns: true,
            proposals: VecDeque::new(),
            attempts: VecDeque::new(),
            accepts: VecDeque::new(),
            equilibria: VecDeque::new(),
            traces: VecDeque::new(),
            buffer: Vec::new(),
            marks: Cell::new(0),
        }
    }

    fn record(&self, c: Call) {
        self.log.borrow_mut().push(c);
    }
}

impl AnalogSide for ScriptedAnalog {
    type Mark = u32;

    fn has_unknowns(&self) -> bool {
        self.unknowns
    }
    fn propose(&mut self, now: f64, stop: f64) -> f64 {
        self.record(Call::Propose(now, stop));
        self.proposals.pop_front().unwrap_or(stop)
    }
    fn deliver(&mut self, boundary: &[Boundary], restart: Restart) {
        self.record(Call::Deliver(boundary.to_vec(), restart));
    }
    fn attempt(&mut self, to: f64, exempt: bool) -> Result<Attempt> {
        self.record(Call::Attempt { to, exempt });
        self.buffer.extend(self.traces.pop_front().unwrap_or_default());
        Ok(self.attempts.pop_front().unwrap_or(Attempt::Converged))
    }
    fn accept(&mut self) -> Result<Accepted> {
        self.record(Call::Accept);
        Ok(self.accepts.pop_front().unwrap_or_default())
    }
    fn equilibrium(&mut self) -> Result<Accepted> {
        self.record(Call::Equilibrium);
        Ok(self.equilibria.pop_front().unwrap_or_default())
    }
    fn mark(&self) -> u32 {
        let m = self.marks.get();
        self.marks.set(m + 1);
        self.record(Call::AnalogMark(m));
        m
    }
    fn restore(&mut self, mark: &u32) {
        self.record(Call::AnalogRestore(*mark));
    }
    fn drain_traces(&mut self, out: &mut Vec<Observation>) {
        self.record(Call::DrainTraces);
        out.append(&mut self.buffer);
    }
}
