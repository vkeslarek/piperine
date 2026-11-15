//! The two adapters that let `Protocol` drive the real sub-solvers
//! (`solver-abi-integration`, design "The two adapters"). Each converts and
//! forwards: `DigitalSide` is the discrete side over
//! `piperine_digital_solver::abi::engine::Digital`, `EquilibriumSide` the
//! analog side of an operating point over the DC driver and the device
//! assembly. Neither holds a rule of its own.
//!
//! SPEC_DEVIATION: design.md places this file at `src/protocol/adapters.rs`.
//! Reason: `tests/protocol_seam.rs` (a surviving guard, SAC-13) proves that
//! no file of `src/protocol/` names a sub-solver item; the adapters name both,
//! so they live with the drivers that call them.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use piperine_analog_solver::device::{Ambient, AssemblySnapshot};
use piperine_analog_solver::error::{Error, Result, SolverDomain};
use piperine_digital_solver::abi::advance::Advance as DigitalAdvance;
use piperine_digital_solver::abi::engine::{Checkpoint, Digital, Horizon as DigitalHorizon, Mode, Shadow as DigitalShadow};
use piperine_digital_solver::abi::net::NetId;
use piperine_digital_solver::abi::observe::Observation as DigitalObservation;
use piperine_digital_solver::abi::plan::DigitalPlan;
use piperine_digital_solver::abi::tick::{Resolution, Tick as DigitalTick};
use piperine_digital_solver::error::DigitalError;
use piperine_digital_solver::limits::DigitalLimits;

use crate::protocol::{
    Accepted, Advance, AnalogSide, Attempt, Boundary, DiscreteSide, Horizon, HostCx, InstantMode, Observation,
    ObservationKind, Outcome, Protocol, Restart, Shadow, Sink, Tick, Waiter,
};
use crate::analyses::dc::DcSolver;
use crate::analyses::transient::{DeviceMark, DeviceRun, TransientSolver};
use crate::core::bind::{Bound, NameMap, Site};

/// A digital failure as the owner reports it (the text keeps every typed field).
pub(crate) fn digital_error(e: DigitalError) -> Error {
    Error::simple(SolverDomain::Digital, e.to_string())
}

/// A `Digital` over an empty plan: what `Bound` holds while a run owns the real one.
pub(crate) fn empty_digital(seconds_per_tick: f64) -> Result<Digital> {
    let plan = DigitalPlan::bind(Vec::new(), Vec::new(), Resolution { seconds_per_tick }).map_err(digital_error)?;
    Ok(Digital::new(plan, DigitalLimits::default()))
}

/// The discrete side: the digital engine, with the slot map of bind.
pub(crate) struct DigitalSide {
    digital: Digital,
    /// The analog copy's slot of each net an analog function reads.
    slots: HashMap<NetId, u32>,
    /// The net behind each slot.
    slot_nets: Vec<NetId>,
    /// The bytes of each slot as the analog copy holds them: bind seeded the
    /// copy with each net's `init`, so a boundary is a slot whose bytes differ
    /// from it (the engine's own store starts at zero).
    copy: Vec<Vec<u8>>,
    /// Host commits made at the start of the analysis (its kind's global).
    on_start: Vec<(NetId, Vec<u8>)>,
}

impl DigitalSide {
    /// The discrete side of `digital` for an analysis whose kind is the global
    /// constant `analysis` (`DC`, `TRAN`, `AC` or `NOISE`).
    pub(crate) fn new(digital: Digital, bound: &Bound, analysis: u32) -> DigitalSide {
        let names: &NameMap = &bound.names;
        let written = |net: NetId| bound.host_writes.iter().rev().find(|(n, _)| *n == net).map(|(_, b)| b.clone());
        let copy = names
            .slot_nets
            .iter()
            .map(|n| written(*n).unwrap_or_else(|| digital.plan().nets[n.0 as usize].init.clone()))
            .collect();
        let on_start = bound
            .analyses
            .iter()
            .filter(|(g, _, _)| *g == analysis)
            .map(|(_, n, b)| (*n, b.clone()))
            .chain(bound.host_writes.iter().cloned())
            .collect();
        DigitalSide { digital, slots: names.slots.clone(), slot_nets: names.slot_nets.clone(), copy, on_start }
    }

    /// The engine, back to its owner.
    pub(crate) fn into_digital(self) -> Digital {
        self.digital
    }

    fn time(&self, tick: DigitalTick) -> f64 {
        tick.0 as f64 * self.digital.plan().resolution.seconds_per_tick
    }
}

impl DiscreteSide for DigitalSide {
    type Mark = Checkpoint;

    fn start(&mut self) -> Result<()> {
        self.digital.start().map_err(digital_error)?;
        for (net, bytes) in &self.on_start {
            self.digital.host_commit(*net, 0, bytes, DigitalTick(0)).map_err(digital_error)?;
        }
        Ok(())
    }

    fn next_due(&self) -> Option<Tick> {
        self.digital.next_due().map(|t| t.0)
    }

    fn advance(&mut self, to: Tick) -> Result<Advance> {
        Ok(match self.digital.advance(DigitalTick(to)).map_err(digital_error)? {
            DigitalAdvance::Reached => Advance::Reached,
            DigitalAdvance::Landing { tick, time_point } => Advance::Landing { tick: tick.0, time_point },
            DigitalAdvance::Final(tick) => Advance::Final(tick.0),
        })
    }

    fn fire(&mut self, net: u32, at: Tick) -> Result<()> {
        self.digital.fire(NetId(net), DigitalTick(at)).map_err(digital_error)
    }

    fn run_instant(&mut self, at: Tick, samples: &[f64], limit: Horizon, mode: InstantMode) -> Result<Outcome> {
        let limit = DigitalHorizon { stop: DigitalTick(limit.stop), crossing: DigitalTick(limit.crossing) };
        let mode = Mode { is_static: mode.is_static, settle: mode.settle };
        let out = self.digital.run_instant(DigitalTick(at), samples, limit, mode).map_err(digital_error)?;
        let mut boundary = Vec::new();
        for (net, bytes) in out.boundary {
            let slot = self.slots[&net];
            if self.copy[slot as usize] != bytes {
                self.copy[slot as usize].clone_from(&bytes);
                boundary.push(Boundary { slot, bytes });
            }
        }
        let host_events = out.host_events.iter().map(|n| n.0).collect();
        Ok(Outcome { boundary, host_param_write: out.host_param_write, finish: out.finish, host_events })
    }

    fn mark(&self) -> Checkpoint {
        self.digital.checkpoint()
    }

    fn restore(&mut self, mark: &Checkpoint) {
        self.digital.restore(mark);
    }

    fn shadow(&self, net: u32, offset: u32, bytes: &[u8]) -> Result<Shadow> {
        Ok(match self.digital.shadow(NetId(net), offset, bytes).map_err(digital_error)? {
            DigitalShadow::Accept => Shadow::Accept,
            DigitalShadow::Refused { check } => Shadow::Refused { check },
            DigitalShadow::Reshape { instance } => Shadow::Reshape { instance },
        })
    }

    fn host_commit(&mut self, net: u32, offset: u32, bytes: &[u8], at: Tick) -> Result<()> {
        self.digital.host_commit(NetId(net), offset, bytes, DigitalTick(at)).map_err(digital_error)
    }

    fn net_name(&self, slot: u32) -> String {
        self.digital.net_name(self.slot_nets[slot as usize])
    }

    fn wait(&mut self, net: u32) {
        self.digital.wait(NetId(net));
    }

    fn drain(&mut self, out: &mut Vec<Observation>) {
        for o in self.digital.drain_observations() {
            let (path, at, kind) = match o {
                DigitalObservation::Change { net, at, bytes } => (net, at.0, ObservationKind::Change { bytes }),
                DigitalObservation::Fire { net, at } => {
                    let crossing = self.digital.plan().nets.iter().any(|n| n.name == net && n.crossing);
                    let kind = if crossing { ObservationKind::Crossing } else { ObservationKind::Change { bytes: Vec::new() } };
                    (net, at.0, kind)
                }
                DigitalObservation::Commit { instance, statement, made, due, .. } => {
                    let due = due.map_or(Tick::MAX, |d| d.0.0);
                    (instance, made.0, ObservationKind::Commit { statement, made: made.0.0, due, bytes: Vec::new() })
                }
                DigitalObservation::Record { stream, at, bytes } => (stream, at.0, ObservationKind::Record { bytes }),
                // abi.md §9 D1: a relaxed field that still moved is one record on the resolver's log.
                DigitalObservation::Unsettled { net, at } => (net, at.0, ObservationKind::Record { bytes: Vec::new() }),
            };
            let time = self.time(at);
            out.push(Observation { path, analysis: Default::default(), time, kind });
        }
    }
}

/// The net of an instance's event pin (a `CROSS` watch's), as bind placed it.
fn event_net(names: &NameMap, instance: usize, pin: u32) -> Result<u32> {
    match names.pins.get(instance).and_then(|p| p.get(pin as usize)) {
        Some(Site::Net(n)) => Ok(n.0),
        _ => Err(Error::simple(
            SolverDomain::Element,
            format!("instance `{}`: crossing event pin {pin} is bound to no net", names.instances[instance]),
        )),
    }
}

/// An accepted solution of the assembly, as the discrete side reads it.
pub(crate) fn accepted(bound: &mut Bound, x: &[f64], amb: Ambient) -> Result<(Accepted, f64)> {
    let acc = bound.analog.accept(x, amb)?;
    let fired = acc.fired.iter().map(|(i, p)| event_net(&bound.names, *i, *p)).collect::<Result<Vec<u32>>>()?;
    Ok((Accepted { fired, samples: bound.analog.samples(x) }, acc.cap))
}

/// The trace records of the assembly, as the sink reads them.
pub(crate) fn traces(bound: &mut Bound, time: f64, out: &mut Vec<Observation>) {
    for t in bound.analog.drain_traces() {
        out.push(Observation {
            path: format!("{}.{}", t.instance, t.watch),
            analysis: Default::default(),
            time,
            kind: ObservationKind::Trace { iteration: t.iteration as u32, bytes: t.bytes },
        });
    }
}

/// The analog side of an operating point: the DC driver's plan and the assembly.
pub(crate) struct EquilibriumSide<'s, 'c> {
    pub(crate) dc: &'s mut DcSolver<'c>,
    /// The last equilibrium solution.
    pub(crate) solution: Option<ndarray::Array1<f64>>,
}

impl EquilibriumSide<'_, '_> {
    fn bound(&mut self) -> &mut Bound {
        self.dc.system.circuit.abi.as_mut().expect("a circuit of devices")
    }
}

impl AnalogSide for EquilibriumSide<'_, '_> {
    type Mark = AssemblySnapshot;

    fn has_unknowns(&self) -> bool {
        self.dc.system.circuit.netlist().max_index().is_some()
    }

    fn propose(&mut self, _now: f64, _stop: f64) -> f64 {
        unreachable!("an operating point takes no step")
    }

    fn deliver(&mut self, boundary: &[Boundary], _restart: Restart) {
        let bound = self.bound();
        for b in boundary {
            bound.analog.set_discrete(b.slot, &b.bytes);
        }
    }

    fn attempt(&mut self, _to: f64, _exempt: bool) -> Result<Attempt> {
        unreachable!("an operating point takes no step")
    }

    fn accept(&mut self) -> Result<Accepted> {
        unreachable!("an operating point takes no step")
    }

    fn equilibrium(&mut self) -> Result<Accepted> {
        let x = self.dc.equilibrium(self.solution.is_none())?;
        let amb = self.dc.system.ambient();
        let (acc, _cap) = accepted(self.bound(), x.as_slice().expect("a contiguous solution"), amb)?;
        self.solution = Some(x);
        Ok(acc)
    }

    fn mark(&self) -> AssemblySnapshot {
        self.dc.system.circuit.abi.as_ref().expect("a circuit of devices").analog.snapshot()
    }

    fn restore(&mut self, mark: &AssemblySnapshot) {
        self.bound().analog.restore(mark);
    }

    fn drain_traces(&mut self, out: &mut Vec<Observation>) {
        traces(self.bound(), 0.0, out);
    }
}

/// The analog side of a transient: the transient driver and the assembly.
pub(crate) struct TransientSide<'s, 'a> {
    pub(crate) tran: &'s mut TransientSolver<'a>,
    pub(crate) run: DeviceRun,
}

impl TransientSide<'_, '_> {
    /// After the operating point: the history rows and the recorded start.
    pub(crate) fn begin(&mut self) -> Result<()> {
        self.tran.device_begin(&mut self.run)
    }
}

impl AnalogSide for TransientSide<'_, '_> {
    type Mark = DeviceMark;

    fn has_unknowns(&self) -> bool {
        self.tran.device_has_unknowns()
    }

    fn propose(&mut self, _now: f64, _stop: f64) -> f64 {
        self.tran.device_propose(&mut self.run)
    }

    fn deliver(&mut self, boundary: &[Boundary], restart: Restart) {
        self.tran.device_deliver(&mut self.run, boundary, restart);
    }

    fn attempt(&mut self, to: f64, exempt: bool) -> Result<Attempt> {
        self.tran.device_attempt(&mut self.run, to, exempt)
    }

    fn accept(&mut self) -> Result<Accepted> {
        self.tran.device_accept(&mut self.run)
    }

    fn equilibrium(&mut self) -> Result<Accepted> {
        let (x, amb) = self.tran.device_op_round(&mut self.run)?;
        let (acc, _cap) = accepted(self.tran.device_bound(), x.as_slice().expect("a contiguous solution"), amb)?;
        Ok(acc)
    }

    fn mark(&self) -> DeviceMark {
        self.tran.device_mark()
    }

    fn restore(&mut self, mark: &DeviceMark) {
        self.tran.device_restore(mark);
    }

    fn drain_traces(&mut self, out: &mut Vec<Observation>) {
        let time = self.run.attempted();
        traces(self.tran.device_bound(), time, out);
    }
}

/// The host's sink and waiters (`Solver::with_sink`, `Solver::on_event`),
/// shared by every analysis a solver runs and forwarded to each run's protocol.
#[derive(Clone, Default)]
pub(crate) struct HostHooks {
    pub(crate) sink: Option<Rc<RefCell<Box<dyn Sink>>>>,
    /// Waiters by event net name.
    pub(crate) waiters: Vec<(String, Rc<RefCell<Waiter>>)>,
}

/// A sink every run shares.
struct SharedSink(Rc<RefCell<Box<dyn Sink>>>);

impl Sink for SharedSink {
    fn observe(&mut self, o: &Observation) {
        self.0.borrow_mut().observe(o);
    }
}

impl HostHooks {
    /// The net of each waiter, by name.
    pub(crate) fn nets(&self, bound: &Bound) -> Result<Vec<u32>> {
        self.waiters
            .iter()
            .map(|(name, _)| {
                bound.names.nets.get(&format!("net:{name}")).map(|n| n.0).ok_or_else(|| {
                    Error::simple(SolverDomain::Digital, format!("a host waiter names no event net `{name}`"))
                })
            })
            .collect()
    }

    /// Installs the sink and the waiters (on `nets`, from `nets`) on a run's protocol.
    pub(crate) fn install<D: DiscreteSide, A: AnalogSide>(&self, mut p: Protocol<D, A>, nets: &[u32]) -> Protocol<D, A> {
        if let Some(sink) = &self.sink {
            p = p.with_sink(Box::new(SharedSink(Rc::clone(sink))));
        }
        for ((_, waiter), net) in self.waiters.iter().zip(nets) {
            let waiter = Rc::clone(waiter);
            p.on_event(*net, Box::new(move |cx: &mut HostCx<'_>| (waiter.borrow_mut())(cx)));
        }
        p
    }
}
