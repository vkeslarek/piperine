//! The two adapters that let `Protocol` drive the real sub-solvers
//! (`solver-abi-integration`, design "The two adapters"). Each converts and
//! forwards: `DigitalSide` is the discrete side over
//! `piperine_digital_solver::engine::Digital`, `EquilibriumSide` the
//! analog side of an operating point over the DC driver and the device
//! assembly. Neither holds a rule of its own.
//!
//! SPEC_DEVIATION: design.md places this file at `src/protocol/adapters.rs`.
//! Reason: `tests/protocol_seam.rs` (a surviving guard, SAC-13) proves that
//! no file of `src/protocol/` names a sub-solver item; the adapters name both,
//! so they live with the drivers that call them.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

use piperine_analog_solver::device::{Ambient, AssemblySnapshot};
use piperine_analog_solver::error::{Error, Result, SolverDomain};
use piperine_digital_solver::advance::Advance as DigitalAdvance;
use piperine_digital_solver::landing::{LandingCause as DigitalLandingCause, TimePointRule};
use piperine_digital_solver::engine::{
    Checkpoint, Digital, Horizon as DigitalHorizon, Mark as DigitalMark, Mode, Shadow as DigitalShadow,
};
use piperine_digital_solver::net::NetId;
use piperine_digital_solver::observe::Observation as DigitalObservation;
use piperine_digital_solver::plan::DigitalPlan;
use piperine_digital_solver::tick::{Resolution, Tick as DigitalTick};
use piperine_digital_solver::error::DigitalError;
use piperine_digital_solver::limits::DigitalLimits;

use crate::protocol::{
    Accepted, Advance, AnalogSide, Attempt, Boundary, DiscreteSide, Horizon, HostCx, InstantMode, LandingCause,
    Observation, ObservationKind, Origin, Outcome, ProposalBound, Protocol, Restart, Shadow, Sink, StepObserver, StepView, Tick, Waiter,
};
use crate::analyses::dc::DcSolver;
use crate::analyses::transient::{DeviceMark, DeviceRun, TransientSolver};
use crate::core::bind::{Bound, NameMap, Site};
use crate::core::result::{ReentryState, TransientStep};

impl From<DigitalLandingCause> for LandingCause {
    /// The seam's copy of the digital engine's cause, variant for variant.
    fn from(cause: DigitalLandingCause) -> LandingCause {
        match cause {
            DigitalLandingCause::AnalogReadChanged => LandingCause::AnalogReadChanged,
            DigitalLandingCause::ContinuousRead => LandingCause::ContinuousRead,
            DigitalLandingCause::DeclaredReach => LandingCause::DeclaredReach,
            DigitalLandingCause::Irreversible => LandingCause::Irreversible,
            DigitalLandingCause::HostWaiter => LandingCause::HostWaiter,
        }
    }
}

/// An instant that does not go quiet (`E0501`) as the owner reports it, naming
/// the bound it ran into: the engine's own text names the nets and the tick, not the bound.
pub(crate) fn instant_error(e: DigitalError, microsteps: usize) -> Error {
    match e {
        DigitalError::NotQuiet { .. } => Error::simple(SolverDomain::Digital, format!("{e} (the bound is {microsteps} microsteps)")),
        other => digital_error(other),
    }
}

/// A digital failure as the owner reports it (the text keeps every typed field).
pub(crate) fn digital_error(e: DigitalError) -> Error {
    if let DigitalError::Unsettled { tick, drives, .. } = e {
        return Error::Unsettled { domain: SolverDomain::Digital, tick, drives };
    }
    Error::simple(SolverDomain::Digital, e.to_string())
}

/// A `Digital` over an empty plan: what `Bound` holds while a run owns the real one.
pub(crate) fn empty_digital(seconds_per_tick: f64) -> Result<Digital> {
    let plan = DigitalPlan::bind(Vec::new(), Vec::new(), Resolution { seconds_per_tick }).map_err(digital_error)?;
    Ok(Digital::new(plan, DigitalLimits::default(), TimePointRule::default()))
}

/// The discrete side's settled state at the top of a transient step: the
/// engine's checkpoint and every net's bytes, which the analog side attaches
/// to the step accepted before it (each step is a full circuit state).
pub(crate) type Settled = Rc<RefCell<Option<(Checkpoint, Vec<Vec<u8>>)>>>;

/// The discrete side: the digital engine, with the slot map of bind.
pub(crate) struct DigitalSide {
    digital: Digital,
    /// The analog copy's slots of each net an analog function reads (an array net's windows have one each).
    net_slots: HashMap<NetId, Vec<u32>>,
    /// The net behind each slot.
    slot_nets: Vec<NetId>,
    /// The elements `(first, length)` of its array net each slot holds; `None` for a whole net.
    slot_windows: Vec<Option<(u32, u32)>>,
    /// Where each slot's bytes lie in the bytes the engine hands the analog side for its net (`Digital::read_analog`): the range of
    /// the window's elements among the net's windows, `None` for a net that is no array (all of its bytes).
    slot_ranges: Vec<Option<Range<usize>>>,
    /// The bytes of each slot as the analog copy holds them: bind seeded the
    /// copy with each net's `init`, so a boundary is a slot whose bytes differ
    /// from it (the engine's own store starts at zero).
    copy: Vec<Vec<u8>>,
    /// Host commits made at the start of the analysis (its kind's global).
    on_start: Vec<(NetId, Vec<u8>)>,
    /// Where each mark also leaves the settled state (a transient's steps).
    settled: Option<Settled>,
    /// The engine was rewound since the last mark: the next mark finds the state the last one
    /// copied, which the step already carries, so it is not copied again.
    rewound: bool,
    /// The microstep bound of the engine, named when an instant does not go quiet.
    microsteps: usize,
}

impl DigitalSide {
    /// The discrete side of `digital` for an analysis whose kind is the global
    /// constant `analysis` (`DC`, `TRAN`, `AC` or `NOISE`).
    pub(crate) fn new(digital: Digital, bound: &Bound, analysis: u32) -> Result<DigitalSide> {
        let names: &NameMap = &bound.names;
        let written = |net: NetId| {
            bound
                .host_writes
                .iter()
                .rev()
                .chain(bound.temperature.iter())
                .find(|(n, _)| *n == net)
                .map(|(_, b)| b.clone())
        };
        let copy = names
            .slot_nets
            .iter()
            .enumerate()
            .map(|(slot, n)| {
                let spec = &digital.plan().nets[n.0 as usize];
                let whole = written(*n).unwrap_or_else(|| spec.init.clone());
                NameMap::window_bytes(&spec.name, names.slot_windows[slot], &whole, spec.size)
            })
            .collect::<Result<Vec<_>>>()?;
        let mut net_slots: HashMap<NetId, Vec<u32>> = HashMap::new();
        for (slot, net) in names.slot_nets.iter().enumerate() {
            net_slots.entry(*net).or_default().push(slot as u32);
        }
        let on_start = bound
            .analyses
            .iter()
            .filter(|(g, _, _)| *g == analysis)
            .map(|(_, n, b)| (*n, b.clone()))
            .chain(bound.temperature.iter().cloned())
            .chain(bound.host_writes.iter().cloned())
            .collect();
        let mut slot_ranges: Vec<Option<Range<usize>>> = vec![None; names.slot_nets.len()];
        for (net, spec) in digital.plan().nets.iter().enumerate().filter(|(_, spec)| spec.count > 0) {
            let mut base = 0usize;
            for (slot, _, length) in names.array_slots(NetId(net as u32), spec.count) {
                let bytes = length as usize * spec.size as usize;
                slot_ranges[slot as usize] = Some(base..base + bytes);
                base += bytes;
            }
        }
        Ok(DigitalSide { digital, net_slots, slot_nets: names.slot_nets.clone(), slot_windows: names.slot_windows.clone(), slot_ranges, copy, on_start, settled: None, rewound: false, microsteps: bound.microsteps })
    }

    /// Each mark also leaves the settled state in `settled`.
    pub(crate) fn record_settled(&mut self, settled: Settled) {
        self.settled = Some(settled);
    }

    /// The engine, back to its owner.
    pub(crate) fn into_digital(self) -> Digital {
        self.digital
    }

    /// The bytes of `slot` within `net_bytes`, the bytes the engine handed the analog side for its net: an internal error naming the
    /// net and the window when they lie outside them.
    fn slot_of_bytes(&self, slot: usize, net_bytes: &[u8]) -> Result<Vec<u8>> {
        let net = self.slot_nets[slot];
        match &self.slot_ranges[slot] {
            None => Ok(net_bytes.to_vec()),
            Some(range) => net_bytes.get(range.clone()).map(<[u8]>::to_vec).ok_or_else(|| {
                let window = self.slot_windows[slot].map_or_else(|| "the whole net".to_string(), |(first, length)| format!("elements {first}..{}", first + length));
                Error::simple(
                    SolverDomain::Digital,
                    format!("internal error: the analog copy's window ({window}) of net `{}` lies outside the {} bytes the engine delivered", self.digital.net_name(net), net_bytes.len()),
                )
            }),
        }
    }

    /// The engine's checkpoint and every net's bytes in it.
    pub(crate) fn state(&self) -> (Checkpoint, Vec<Vec<u8>>) {
        let nets = (0..self.digital.plan().nets.len()).map(|i| self.digital.read_analog(NetId(i as u32))).collect();
        (self.digital.checkpoint(), nets)
    }

    /// Puts the engine back to a captured checkpoint whose nets hold `nets`;
    /// the analog copy then holds what they hold: returns each slot's bytes.
    pub(crate) fn resume(&mut self, checkpoint: &Checkpoint, nets: &[Vec<u8>]) -> Result<Vec<(u32, Vec<u8>)>> {
        self.digital.restore(checkpoint);
        let mut slots = Vec::with_capacity(self.slot_nets.len());
        for slot in 0..self.slot_nets.len() {
            let bytes = self.slot_of_bytes(slot, &nets[self.slot_nets[slot].0 as usize])?;
            self.copy[slot].clone_from(&bytes);
            slots.push((slot as u32, bytes));
        }
        Ok(slots)
    }

    /// The nets of `nets` whose bytes differ from the analog side's copy, as the boundary slots
    /// they are; the copy takes the new bytes, as the analog side will once they are delivered.
    fn changed_slots(&mut self, nets: Vec<(NetId, Vec<u8>)>) -> Result<Vec<Boundary>> {
        let mut boundary = Vec::new();
        for (net, whole) in nets {
            for slot in self.net_slots[&net].clone() {
                let bytes = self.slot_of_bytes(slot as usize, &whole)?;
                if self.copy[slot as usize] != bytes {
                    self.copy[slot as usize].clone_from(&bytes);
                    boundary.push(Boundary { slot, bytes });
                }
            }
        }
        Ok(boundary)
    }

    /// The values an instant passed through (`Outcome::passing`) as the slots they are. The copy
    /// does not take them: they reach the analog side for the zero-width solve only.
    /// An array net gives one per window an analog function reads.
    fn passing_slots(&self, nets: Vec<(NetId, Vec<u8>)>) -> Result<Vec<Boundary>> {
        let mut passing = Vec::new();
        for (net, whole) in nets {
            for slot in &self.net_slots[&net] {
                passing.push(Boundary { slot: *slot, bytes: self.slot_of_bytes(*slot as usize, &whole)? });
            }
        }
        Ok(passing)
    }

    /// Adds to `boundary` each slot that `passing` moves away from the copy and the instant left
    /// where it began (a pulse): after the zero-width solve, the analog side takes the copy's bytes back.
    fn restore_passed(&self, passing: &[Boundary], boundary: &mut Vec<Boundary>) {
        for passed in passing {
            let kept = &self.copy[passed.slot as usize];
            if passed.bytes != *kept && !boundary.iter().any(|b| b.slot == passed.slot) {
                boundary.push(Boundary { slot: passed.slot, bytes: kept.clone() });
            }
        }
    }

    fn time(&self, tick: DigitalTick) -> f64 {
        tick.0 as f64 * self.digital.plan().resolution.seconds_per_tick
    }
}

impl DiscreteSide for DigitalSide {
    type Mark = DigitalMark;

    fn start(&mut self, at: Tick) -> Result<()> {
        self.digital.start_at(DigitalTick(at)).map_err(digital_error)?;
        for (net, bytes) in &self.on_start {
            // A param bound before the run holds that value from the start: it is no change (§5.3.1).
            if self.digital.plan().nets[net.0 as usize].param {
                self.digital.seed_param(*net, bytes);
            } else {
                // a value the host gave before the run is the carrier's initial value (§5.3, D8)
                self.digital.expect_initial(*net);
                self.digital.host_commit(*net, 0, bytes, DigitalTick(at)).map_err(digital_error)?;
            }
        }
        Ok(())
    }

    fn next_due(&self) -> Option<Tick> {
        self.digital.next_due().map(|t| t.0)
    }

    fn advance(&mut self, to: Tick) -> Result<Advance> {
        let microsteps = self.microsteps;
        Ok(match self.digital.advance(DigitalTick(to)).map_err(|e| instant_error(e, microsteps))? {
            DigitalAdvance::Reached => Advance::Reached,
            DigitalAdvance::Landing { tick, cause } => Advance::Landing { tick: tick.0, cause: cause.into() },
            DigitalAdvance::Final(tick) => Advance::Final(tick.0),
        })
    }

    fn fire(&mut self, net: u32, at: Tick) -> Result<()> {
        self.digital.fire(NetId(net), DigitalTick(at)).map_err(digital_error)
    }

    fn reopen(&mut self, at: Tick, next_microstep: bool) {
        self.digital.reopen(DigitalTick(at), next_microstep);
    }

    fn is_idle_at(&self, at: Tick) -> bool {
        self.digital.is_idle_at(DigitalTick(at))
    }

    fn pass_instant(&mut self, at: Tick) {
        self.digital.pass_instant(DigitalTick(at));
    }

    fn run_instant(&mut self, at: Tick, samples: &[f64], limit: Horizon, mode: InstantMode) -> Result<Outcome> {
        let limit = DigitalHorizon { stop: DigitalTick(limit.stop), crossing: DigitalTick(limit.crossing) };
        let mode = Mode { is_static: mode.is_static, settle: mode.settle };
        let microsteps = self.microsteps;
        let out = self.digital.run_instant(DigitalTick(at), samples, limit, mode).map_err(|e| instant_error(e, microsteps))?;
        // read against the copy as it stood before the instant, so it is taken first
        let passing = self.passing_slots(out.passing)?;
        let mut boundary = self.changed_slots(out.boundary)?;
        self.restore_passed(&passing, &mut boundary);
        let host_events = out.host_events.iter().map(|n| n.0).collect();
        let settle_boundary = self.changed_slots(out.settle_boundary)?;
        Ok(Outcome { boundary, host_param_write: out.host_param_write, finish: out.finish, host_events, settle_boundary, passing })
    }

    fn land_settle_commits(&mut self, tick: Tick) {
        self.digital.land_settle_commits(DigitalTick(tick));
    }

    fn drop_settle_commits(&mut self) {
        self.digital.drop_settle_commits();
    }

    /// The engine's journal mark: no net is copied. The full copy and every net's bytes are
    /// taken only where a re-entry state is recorded (`settled`), once per accepted step: the
    /// retry of a rejected attempt starts from the state its rewind restored, which the first
    /// attempt's copy already holds.
    fn mark(&mut self) -> DigitalMark {
        if let Some(settled) = &self.settled
            && !self.rewound
        {
            let nets = (0..self.digital.plan().nets.len()).map(|i| self.digital.read_analog(NetId(i as u32))).collect();
            *settled.borrow_mut() = Some((self.digital.checkpoint(), nets));
        }
        self.rewound = false;
        self.digital.mark()
    }

    fn restore(&mut self, mark: &DigitalMark) {
        self.digital.rewind(mark);
        self.rewound = true;
    }

    fn release(&mut self, mark: DigitalMark) {
        self.digital.release(mark);
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

    fn net(&self, id: u32) -> Option<&[u8]> {
        ((id as usize) < self.digital.plan().nets.len()).then(|| self.digital.read(NetId(id)))
    }

    fn net_bytes(&self, id: u32) -> Option<std::borrow::Cow<'_, [u8]>> {
        let spec = self.digital.plan().nets.get(id as usize)?;
        if spec.count == 0 {
            return self.net(id).map(std::borrow::Cow::Borrowed);
        }
        // an array net: every element, an element never committed at its initial value
        let mut bytes = Vec::with_capacity(spec.count as usize * spec.size as usize);
        for element in 0..spec.count {
            bytes.extend_from_slice(self.digital.read_element(NetId(id), element));
        }
        Some(std::borrow::Cow::Owned(bytes))
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
                DigitalObservation::Commit { instance, reaction, statement, made, due } => {
                    let due = due.map_or(Tick::MAX, |d| d.0.0);
                    (instance, made.0, ObservationKind::Commit { reaction, statement, made: made.0.0, due, bytes: Vec::new() })
                }
                DigitalObservation::Record { stream, reaction, statement, at, bytes } => {
                    (stream, at.0, ObservationKind::Record { site: Some((reaction, statement)), bytes })
                }
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
            SolverDomain::Device,
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
        &mut self.dc.system.circuit.abi
    }
}

impl AnalogSide for EquilibriumSide<'_, '_> {
    type Mark = AssemblySnapshot;

    fn has_unknowns(&self) -> bool {
        self.dc.system.circuit.netlist().max_index().is_some()
    }

    fn propose(&mut self, _now: f64, _stop: f64, _next_due: Option<f64>) -> f64 {
        unreachable!("an operating point takes no step")
    }

    fn deliver(&mut self, boundary: &[Boundary], _restart: Restart) {
        let bound = self.bound();
        for b in boundary {
            bound.analog.set_discrete(b.slot, &b.bytes);
        }
    }

    fn attempt(&mut self, _to: f64, _landing: bool) -> Result<Attempt> {
        unreachable!("an operating point takes no step")
    }

    fn accept(&mut self) -> Result<Accepted> {
        unreachable!("an operating point takes no step")
    }

    fn zero_width(&mut self) -> Result<Option<Accepted>> {
        unreachable!("an operating point reads no passed value (an equilibrium holds no state)")
    }

    fn equilibrium(&mut self) -> Result<Accepted> {
        let x = self.dc.equilibrium(self.solution.is_none())?;
        let amb = self.dc.system.ambient();
        let (acc, _cap) = accepted(self.bound(), x.as_slice().expect("a contiguous solution"), amb)?;
        self.solution = Some(x);
        Ok(acc)
    }

    fn mark(&self) -> AssemblySnapshot {
        self.dc.system.circuit.abi.analog.snapshot()
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
    /// The discrete side's state at the top of each step.
    pub(crate) settled: Settled,
}

impl TransientSide<'_, '_> {
    /// After the operating point: the history rows and the recorded start.
    pub(crate) fn begin(&mut self) -> Result<()> {
        self.tran.device_begin(&mut self.run)
    }

    /// Instead of the operating point: the captured step's solution and
    /// histories; returns the continuous values reactions read there.
    pub(crate) fn resume(&mut self, step: &TransientStep, state: &ReentryState) -> Result<Vec<f64>> {
        self.tran.device_resume(&mut self.run, step, state)
    }

    /// The recorded start carries the state a later run re-enters from.
    pub(crate) fn mark_start(&mut self, digital: Checkpoint, nets: Vec<Vec<u8>>) {
        self.tran.device_mark_start(&mut self.run, digital, nets);
    }
}

impl AnalogSide for TransientSide<'_, '_> {
    type Mark = DeviceMark;

    fn has_unknowns(&self) -> bool {
        self.tran.device_has_unknowns()
    }

    fn propose(&mut self, _now: f64, _stop: f64, next_due: Option<f64>) -> f64 {
        self.tran.device_propose(&mut self.run, next_due)
    }

    fn proposal_bound(&self) -> ProposalBound {
        self.run.proposal_bound()
    }

    fn deliver(&mut self, boundary: &[Boundary], restart: Restart) {
        self.tran.device_deliver(&mut self.run, boundary, restart);
    }

    fn attempt(&mut self, to: f64, landing: bool) -> Result<Attempt> {
        // Nothing has moved since the protocol's mark: the step accepted last
        // takes the settled state as its re-entry state.
        if let Some((digital, nets)) = self.settled.borrow_mut().take() {
            self.tran.device_capture(&mut self.run, digital, nets);
        }
        self.tran.device_attempt(&mut self.run, to, landing)
    }

    fn accept(&mut self) -> Result<Accepted> {
        self.tran.device_accept(&mut self.run)
    }

    fn zero_width(&mut self) -> Result<Option<Accepted>> {
        self.tran.device_zero_width(&mut self.run)
    }

    fn equilibrium(&mut self) -> Result<Accepted> {
        let (x, amb) = self.tran.device_op_round(&mut self.run)?;
        let (acc, cap) = accepted(self.tran.device_bound(), x.as_slice().expect("a contiguous solution"), amb)?;
        self.run.set_cap(cap);
        Ok(acc)
    }

    fn mark(&self) -> DeviceMark {
        self.tran.device_mark()
    }

    fn restore(&mut self, mark: &DeviceMark) {
        self.tran.device_restore(mark);
    }

    fn rebase(&mut self, origin: Origin) {
        self.run.rebase(origin);
    }

    fn retake(&mut self, mark: &DeviceMark) {
        self.tran.device_unaccept(&mut self.run);
        self.tran.device_restore(mark);
    }

    fn drain_traces(&mut self, out: &mut Vec<Observation>) {
        let time = self.run.attempted();
        traces(self.tran.device_bound(), time, out);
    }

    fn solution(&self) -> &[f64] {
        self.tran.device_solution()
    }

    fn step_time(&self) -> Option<f64> {
        self.has_unknowns().then(|| self.run.accepted_time())
    }
}

/// The host's sink and waiters (`Solver::with_sink`, `Solver::on_event`),
/// shared by every analysis a solver runs and forwarded to each run's protocol.
#[derive(Clone, Default)]
pub(crate) struct HostHooks {
    pub(crate) sink: Option<Rc<RefCell<Box<dyn Sink>>>>,
    /// Waiters by event net name.
    pub(crate) waiters: Vec<(String, Rc<RefCell<Waiter>>)>,
    /// The host's observer of accepted steps (a transient's).
    pub(crate) observer: Option<Rc<RefCell<Box<dyn StepObserver>>>>,
}

/// A sink every run shares.
struct SharedSink(Rc<RefCell<Box<dyn Sink>>>);

impl Sink for SharedSink {
    fn observe(&mut self, o: &Observation) {
        self.0.borrow_mut().observe(o);
    }
}

/// An observer every run shares.
struct SharedObserver(Rc<RefCell<Box<dyn StepObserver>>>);

impl StepObserver for SharedObserver {
    fn step(&mut self, step: &StepView<'_>) {
        self.0.borrow_mut().step(step);
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
        if let Some(observer) = &self.observer {
            p = p.with_step_observer(Box::new(SharedObserver(Rc::clone(observer))));
        }
        for ((_, waiter), net) in self.waiters.iter().zip(nets) {
            let waiter = Rc::clone(waiter);
            p.on_event(*net, Box::new(move |cx: &mut HostCx<'_>| (waiter.borrow_mut())(cx)));
        }
        p
    }
}
