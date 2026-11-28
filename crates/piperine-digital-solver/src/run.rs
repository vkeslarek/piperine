//! Calling the devices (abi.md §4 functions, §5 reactions): the discrete
//! functions to a fixed point over the levelized order (D), and the reactions
//! of one microstep on one snapshot with the commit path (R).
//!
//! Both read and write through `Store` and call devices only through
//! `piperine_abi::caller::Handle`.

use piperine_abi::caller::{Commit, ElementSource, ViewHandle};
use piperine_abi::device::{Access, Flow};
use piperine_abi::raw::{ArrayView, Cell, List, Shape, Statement, WHOLE, cell, part, reaction_flag, statement_flag, status};

use crate::dag::Step;
use crate::net::{NetId, NetKind, NetSpec};
use crate::observe::{Obs, Observation};
use crate::plan::{Analysis, Binding, DigitalPlan, InstancePlan};
use crate::queue::{Instant, Pending, Queue, Writer};
use crate::settle::SettleCommits;
use crate::store::{Aligned, Store};
use crate::tick::{Due, Resolution, Tick};
use crate::error::DigitalError;

/// The values a call reads that are not pins: time, `STATIC`, `TICK`, `HORIZON`, and the
/// continuation values `GMIN` and `SOURCE_SCALE` (abi.md §4.2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ambient {
    pub tick: Tick,
    /// Seconds: the tick times the resolution.
    pub time: f64,
    pub is_static: bool,
    pub horizon: Tick,
    /// The `HORIZON` of a reaction a `CROSS` watch can reach.
    pub crossing_horizon: Tick,
    /// `GMIN` and `SOURCE_SCALE`: the values the owner set (`Digital::set_continuation`).
    pub gmin: f64,
    pub source_scale: f64,
}

impl Ambient {
    /// The ambient of an instant at `tick`, with `HORIZON` the tick itself, `GMIN` 0 and
    /// `SOURCE_SCALE` 1.
    pub fn at(tick: Tick, seconds_per_tick: f64) -> Ambient {
        Ambient {
            tick,
            time: tick.0 as f64 * seconds_per_tick,
            is_static: false,
            horizon: tick,
            crossing_horizon: tick,
            gmin: 0.0,
            source_scale: 1.0,
        }
    }
}

/// Which functions will run in the next evaluation: one flag per function of the dag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirtySet(Vec<bool>);

impl DirtySet {
    /// Every function dirty: the first instant of a run (F3).
    pub fn all(plan: &DigitalPlan) -> DirtySet {
        DirtySet(vec![true; plan.dag().len()])
    }

    /// No function dirty: an instant whose writes mark the functions they reach.
    pub fn none(plan: &DigitalPlan) -> DirtySet {
        DirtySet(vec![false; plan.dag().len()])
    }

    /// Marks every function that reads `net`.
    pub fn mark_net(&mut self, plan: &DigitalPlan, net: NetId) {
        for f in plan.dag().readers(net) {
            self.0[*f] = true;
        }
    }

    /// Whether a function is dirty.
    pub fn any(&self) -> bool {
        self.0.iter().any(|d| *d)
    }
}

/// Why a walk over the functions stopped.
enum Stop {
    Error(DigitalError),
    /// A function of the cone returned `RESHAPE`: the instance's path.
    Reshape(String),
}

impl From<DigitalError> for Stop {
    fn from(e: DigitalError) -> Stop {
        Stop::Error(e)
    }
}

/// The bytes of one element of an array net a cell reads.
struct ElementPart;

impl ElementPart {
    /// Hands `fill` the bytes of `part` of `element` of array net `net` as of the microstep the store is in.
    fn fill(store: &Store, net: NetId, element: u64, part: u32, fill: impl FnOnce(&[u8])) {
        match part {
            part::VALUE => fill(store.element_value(net, element)),
            part::PREVIOUS => fill(store.element_previous(net, element)),
            part::CHANGED => fill(&[store.element_changed(net, element) as u8]),
            _ => {}
        }
    }
}

/// An array pin's elements as a reaction's `WHOLE` read sees them: a window of an array net at the snapshot (R2).
struct WindowView<'a> {
    store: &'a Store,
    net: NetId,
    /// The array net's element the pin's element 0 is.
    first: u64,
    length: u64,
    size: u32,
}

impl ElementSource for WindowView<'_> {
    fn count(&self) -> u64 {
        self.length
    }

    fn size(&self) -> u32 {
        self.size
    }

    fn element(&self, element: u64, part: u32) -> &[u8] {
        let at = self.first + element;
        match part {
            part::VALUE => self.store.element_value(self.net, at),
            part::PREVIOUS => self.store.element_previous(self.net, at),
            _ if self.store.element_changed(self.net, at) => &[1],
            _ => &[0],
        }
    }
}

/// The `WHOLE` reads of one reaction.
struct WholeReads;

impl WholeReads {
    /// The windows of reaction `reaction`'s `WHOLE` reads, in the order the reads are listed: each is the array net an
    /// array pin is bound to, or the run of elements of it (R2: the store is the microstep's snapshot).
    fn windows<'s>(store: &'s Store, shape: &Shape, reaction: usize, pins: &[Binding]) -> Vec<WindowView<'s>> {
        let Some(reaction) = shape.reactions.get(reaction) else { return Vec::new() };
        Self::windows_of(store, shape, &reaction.reads, &reaction.elements, pins)
    }

    /// The windows of the `WHOLE` reads among `reads` (`elements` one entry per read, or empty), in their order.
    fn windows_of<'s>(store: &'s Store, shape: &Shape, reads: &List<'_, Cell>, elements: &List<'_, u64>, pins: &[Binding]) -> Vec<WindowView<'s>> {
        let mut windows = Vec::new();
        for (i, c) in reads.iter().enumerate() {
            if elements.get(i).copied() != Some(WHOLE) {
                continue;
            }
            let size = shape.pins.get(c.index as usize).map_or(0, |pin| pin.layout.size);
            match pins[c.index as usize] {
                Binding::Net(net) => {
                    let length = store.array(net).map_or(0, |array| array.count());
                    windows.push(WindowView { store, net, first: 0, length, size });
                }
                Binding::ArrayWindow { net, first, length } => windows.push(WindowView { store, net, first, length, size }),
                _ => {}
            }
        }
        windows
    }
}

/// What one call reads: the pins' bindings, the store's bytes, the ambient values and the analog samples.
struct CellReader<'r> {
    /// The bindings of the instance's pins.
    pins: &'r [Binding],
    /// The microstep's snapshot.
    store: &'r Store,
    /// The ambient values (time, horizon, `STATIC`).
    ambient: &'r Ambient,
    /// The continuous values the analog side sampled.
    samples: &'r [f64],
}

impl CellReader<'_> {
    /// One input or output buffer of `len` bytes, filled with what `c` names.
    fn bytes(&self, c: &Cell, element: u64, len: usize) -> Aligned {
        let (store, ambient) = (self.store, self.ambient);
        let mut buf = Aligned::zeroed(len);
        let fill = |buf: &mut Aligned, src: &[u8]| {
            let n = src.len().min(len);
            buf.bytes_mut()[..n].copy_from_slice(&src[..n]);
        };
        match c.kind {
            cell::PIN => match self.pins[c.index as usize] {
                Binding::Net(id) if store.array(id).is_none() => match c.part {
                    part::VALUE => fill(&mut buf, store.value(id)),
                    part::PREVIOUS => fill(&mut buf, store.previous(id)),
                    part::CHANGED => fill(&mut buf, &[store.changed(id) as u8]),
                    _ => {}
                },
                // an array pin read at a constant element, or a scalar pin on one element of an array net (a window)
                Binding::Net(id) => ElementPart::fill(store, id, element, c.part, |bytes| fill(&mut buf, bytes)),
                Binding::ArrayElement { net, element: first } => ElementPart::fill(store, net, first, c.part, |bytes| fill(&mut buf, bytes)),
                Binding::ArrayWindow { net, first, .. } => ElementPart::fill(store, net, first + element, c.part, |bytes| fill(&mut buf, bytes)),
                Binding::Sample(i) => fill(&mut buf, &self.samples.get(i as usize).copied().unwrap_or(0.0).to_le_bytes()),
                Binding::Absent => {}
            },
            cell::AMBIENT => match c.index {
                piperine_abi::raw::ambient::TIME => fill(&mut buf, &ambient.time.to_le_bytes()),
                piperine_abi::raw::ambient::STATIC => fill(&mut buf, &[ambient.is_static as u8]),
                piperine_abi::raw::ambient::TICK => fill(&mut buf, &ambient.tick.0.to_le_bytes()),
                piperine_abi::raw::ambient::HORIZON => fill(&mut buf, &ambient.horizon.0.to_le_bytes()),
                piperine_abi::raw::ambient::GMIN => fill(&mut buf, &ambient.gmin.to_le_bytes()),
                piperine_abi::raw::ambient::SOURCE_SCALE => fill(&mut buf, &ambient.source_scale.to_le_bytes()),
                _ => {}
            },
            _ => {}
        }
        buf
    }

    /// The input buffers of `reads` (`elements` one entry per read, or empty; `lens` their sizes): a `WHOLE` read
    /// takes the next of `views`, any other read its cell's bytes.
    fn inputs(&self, reads: &List<'_, Cell>, elements: &List<'_, u64>, lens: &[usize], views: &[ViewHandle<'_>]) -> Vec<Aligned> {
        let mut whole = views.iter();
        reads
            .iter()
            .zip(lens)
            .enumerate()
            .map(|(i, (c, len))| match elements.get(i).copied().unwrap_or(0) {
                WHOLE => {
                    let view = whole.next().expect("one view per WHOLE read").view();
                    let mut buffer = Aligned::zeroed(*len);
                    buffer.bytes_mut().copy_from_slice(&ArrayView::as_bytes(&view)[..*len]);
                    buffer
                }
                element => self.bytes(c, element, *len),
            })
            .collect()
    }
}

/// What a function wrote to one net.
struct Out {
    net: NetId,
    /// The element of an array net the write is to (a pin bound to a window of one element).
    element: Option<u64>,
    event: bool,
    bytes: Vec<u8>,
}

/// One relaxing cycle (abi.md §9 D1) as its passes run it.
struct RelaxGroup<'g> {
    /// The functions of the cycle: first those that write no `RELAX` net, then the drives.
    functions: Vec<usize>,
    /// The cycle's `RELAX` nets.
    nets: &'g [NetId],
    /// Every net a function of the cycle writes, once each, in the order the functions write them.
    written: Vec<NetId>,
}

impl<'g> RelaxGroup<'g> {
    /// The cycle of `functions` through `nets`, its functions ordered so that the first pass reads what `resolve` of
    /// the absent drives gives and not what an earlier relaxation left on those nets (§8.2.3 steps 1 and 2).
    fn new(plan: &DigitalPlan, functions: &[usize], nets: &'g [NetId]) -> RelaxGroup<'g> {
        let writes_relax = |f: &usize| plan.dag().function(*f).writes.iter().any(|w| nets.contains(w));
        let functions: Vec<usize> =
            functions.iter().copied().filter(|f| !writes_relax(f)).chain(functions.iter().copied().filter(|f| writes_relax(f))).collect();
        let mut written: Vec<NetId> = Vec::new();
        for f in &functions {
            for w in &plan.dag().function(*f).writes {
                if !written.contains(w) {
                    written.push(*w);
                }
            }
        }
        RelaxGroup { functions, nets, written }
    }

    /// The bytes of every written net, in `written` order.
    fn snapshot(&self, store: &Store) -> Vec<Vec<u8>> {
        self.written.iter().map(|n| store.value(*n).to_vec()).collect()
    }

    /// `E0511`: the cycle still moved after its bounded passes; the loop's drives in name order (spec §8.2.3 step 4).
    fn unsettled(&self, plan: &DigitalPlan, at: Instant) -> DigitalError {
        let mut drives: Vec<String> = self.nets.iter().map(|n| plan.nets[n.0 as usize].name.clone()).collect();
        drives.sort();
        DigitalError::Unsettled { code: "E0511", tick: at.0.0, drives }
    }
}

/// What the passes of a relaxing cycle settled on.
struct SettledCycle {
    /// The settled bytes of every written net, in the group's `written` order.
    after: Vec<Vec<u8>>,
    /// The event nets the cycle's functions fired, once each.
    fires: Vec<NetId>,
}

/// D, the discrete functions (abi.md §4): the plan, the store they read and write, the functions still to run, and
/// what the run observes. Every write through it observes a change and dirties the functions that read the net.
pub struct FunctionRun<'a> {
    /// The plan whose functions run.
    plan: &'a DigitalPlan,
    /// The nets the functions read and write.
    store: &'a mut Store,
    /// The functions still to run: a write dirties the readers of its net.
    dirty: &'a mut DirtySet,
    /// What the run observes: every change a write makes.
    observer: &'a mut Obs,
}

impl<'a> FunctionRun<'a> {
    /// A run of `plan`'s functions over `store`, with `dirty` the functions to run and `observer` what it observes.
    pub fn new(plan: &'a DigitalPlan, store: &'a mut Store, dirty: &'a mut DirtySet, observer: &'a mut Obs) -> FunctionRun<'a> {
        FunctionRun { plan, store, dirty, observer }
    }

    /// Writes `bytes` at `offset` of `net`: observes and dirties the readers on a change.
    pub fn write_net(&mut self, net: NetId, offset: u32, bytes: &[u8], at: Instant) -> bool {
        let changed = self.store.write(net, offset, bytes);
        if changed {
            // the pin of a record stream delivers the record (`Observation::Record`), not a value of a net: it may share its path
            // with a signal of the same size, and a host could not tell the two apart
            if !self.plan.nets[net.0 as usize].record {
                self.observer.push(Observation::Change { net: self.plan.nets[net.0 as usize].name.clone(), at, bytes: self.store.value(net).to_vec() });
            }
            self.dirty.mark_net(self.plan, net);
        }
        changed
    }

    /// Writes `bytes` at `offset` of `element` of array net `net`: observes and dirties the readers on a change.
    pub fn write_element(&mut self, net: NetId, element: u64, offset: u32, bytes: &[u8], at: Instant) -> bool {
        let changed = self.store.write_element(net, element, offset, bytes);
        if changed {
            let name = format!("{}[{element}]", self.plan.nets[net.0 as usize].name);
            self.observer.push(Observation::Change { net: name, at, bytes: self.store.element_value(net, element).to_vec() });
            self.dirty.mark_net(self.plan, net);
        }
        changed
    }

    /// Fires event net `net`: observes it and dirties the functions that read its `CHANGED`.
    pub fn fire_net(&mut self, net: NetId, at: Instant) {
        self.store.fire(net);
        self.observer.push(Observation::Fire { net: self.plan.nets[net.0 as usize].name.clone(), at });
        self.dirty.mark_net(self.plan, net);
    }

    /// D: runs the dirty functions in the dag's order, relaxing the cycles through `RELAX` nets,
    /// until the order is done (abi.md §3.1 P2, §9 D1). A fault, or a `RESHAPE` (which a committed
    /// store never produces, P6), is a `Fault`.
    pub fn run(&mut self, at: Instant, ambient: &Ambient) -> Result<(), DigitalError> {
        match self.walk(at, ambient, None) {
            Ok(()) => Ok(()),
            Err(Stop::Error(e)) => Err(e),
            Err(Stop::Reshape(instance)) => unreachable!("a walk without a cone filter turns RESHAPE into a fault: {instance}"),
        }
    }

    /// D restricted to the parameter cone, for a shadow of the store (abi.md P6): the path of the instance
    /// whose function returned `RESHAPE`, if one did.
    pub fn run_cone(&mut self, at: Instant, ambient: &Ambient) -> Result<Option<String>, DigitalError> {
        let plan = self.plan;
        match self.walk(at, ambient, Some(plan.cone())) {
            Ok(()) => Ok(None),
            Err(Stop::Error(e)) => Err(e),
            Err(Stop::Reshape(instance)) => Ok(Some(instance)),
        }
    }

    /// Runs the dirty functions the dag orders, those of `cone` only when it is given: a `RESHAPE` is a `Stop::Reshape`
    /// inside a cone and a fault outside it.
    fn walk(&mut self, at: Instant, ambient: &Ambient, cone: Option<&[bool]>) -> Result<(), Stop> {
        let allowed = |f: usize| cone.is_none_or(|c| c[f]);
        let plan = self.plan;
        for step in plan.dag().order() {
            match step {
                Step::Run(f) => {
                    if !self.dirty.0[*f] || !allowed(*f) {
                        continue;
                    }
                    self.dirty.0[*f] = false;
                    let outs = self.call_function(at, ambient, *f, cone.is_some())?;
                    self.apply(at, outs);
                }
                Step::Relax { fns, nets } => {
                    if !fns.iter().any(|f| self.dirty.0[*f]) || !fns.iter().all(|f| allowed(*f)) {
                        continue;
                    }
                    for f in fns {
                        self.dirty.0[*f] = false;
                    }
                    self.relax(at, ambient, fns, nets, cone.is_some())?;
                }
            }
        }
        Ok(())
    }

    /// Calls function `f` of the dag on the store's current bytes and returns what it wrote.
    fn call_function(&self, at: Instant, ambient: &Ambient, f: usize, cone: bool) -> Result<Vec<Out>, Stop> {
        let node = self.plan.dag().function(f);
        let inst = &self.plan.instances[node.instance as usize];
        let shape = inst.handle.shape();
        let func = shape.functions.get(node.k as usize).expect("bind took this function from the shape");
        let reads = shape.cell_lens_of(&func.reads, &func.elements, Access::FunctionRead).expect("bind validated the shape");
        let writes = shape.cell_lens_of(&func.writes, &List::empty(), Access::FunctionWrite).expect("bind validated the shape");
        // a function reads an array at a run-time element through its view (R2), as a reaction does
        let windows = WholeReads::windows_of(self.store, shape, &func.reads, &func.elements, &inst.pins);
        let handles: Vec<ViewHandle<'_>> = windows.iter().map(|window| ViewHandle::new(window)).collect();
        let reader = CellReader { pins: &inst.pins, store: self.store, ambient, samples: &[] };
        let inputs = reader.inputs(&func.reads, &func.elements, &reads, &handles);
        let mut outputs: Vec<Aligned> = func.writes.iter().zip(&writes).map(|(c, len)| reader.bytes(c, 0, *len)).collect();
        let input_refs: Vec<&[u8]> = inputs.iter().map(|b| b.bytes()).collect();
        let mut output_refs: Vec<&mut [u8]> = outputs.iter_mut().map(|b| b.bytes_mut()).collect();
        let partials = func.partials.len();
        let seed = vec![0u8; partials];
        let mut jac = vec![0.0f64; partials];
        if let Err(e) = inst.handle.eval(node.k, &input_refs, &mut output_refs, &seed, &mut jac) {
            if e.status == status::RESHAPE && cone {
                return Err(Stop::Reshape(inst.path.clone()));
            }
            return Err(Stop::Error(DigitalError::device_fault(e, at.0, &inst.path)));
        }
        let mut out = Vec::new();
        for (c, buf) in func.writes.iter().zip(&outputs) {
            let (net, element) = match inst.pins[c.index as usize] {
                Binding::Net(net) => (net, None),
                Binding::ArrayElement { net, element } => (net, Some(element)),
                _ => continue,
            };
            let event = self.plan.nets[net.0 as usize].kind == NetKind::Event;
            let bytes = buf.bytes().to_vec();
            out.push(Out { net, element, event, bytes });
        }
        Ok(out)
    }

    /// Applies a function's outputs the way a microstep sees them.
    fn apply(&mut self, at: Instant, outs: Vec<Out>) {
        for o in outs {
            if o.event {
                if o.bytes[0] != 0 {
                    self.fire_net(o.net, at);
                }
            } else if let Some(element) = o.element {
                self.write_element(o.net, element, 0, &o.bytes, at);
            } else {
                self.write_net(o.net, 0, &o.bytes, at);
            }
        }
    }

    /// The cycle of `fns` through the `RELAX` nets `nets` (abi.md §9 D1): those nets start at zero
    /// bytes (every drive absent: `driving` 0), the functions run in order until no byte changes, at
    /// most `nets.len() + 1` passes. A cycle that still moves after them is `E0511`, naming the loop's
    /// drives in name order (spec §8.2.3 step 4). Each pass runs first the functions that write no `RELAX` net (the
    /// resolvers and what derives from them), then the drives, so the first pass of a loop reads what `resolve` of the
    /// absent drives gives and not what an earlier relaxation left on those nets (§8.2.3 steps 1 and 2).
    fn relax(&mut self, at: Instant, ambient: &Ambient, fns: &[usize], nets: &[NetId], cone: bool) -> Result<(), Stop> {
        let group = RelaxGroup::new(self.plan, fns, nets);
        let saved: Vec<_> = group.written.iter().map(|n| self.store.save_net(*n)).collect();
        for n in nets {
            let zeros = vec![0u8; self.store.value(*n).len()];
            self.store.write(*n, 0, &zeros);
        }
        let relaxed = self.relax_passes(&group, at, ambient, cone)?;
        // the run sees one change per net: from the value it had before the group, to the settled one
        for (i, net) in group.written.iter().enumerate() {
            self.store.restore_net(*net, &saved[i]);
            self.write_net(*net, 0, &relaxed.after[i], at);
        }
        // the group has just settled on what its closing writes show: they are no reason to run it again
        for f in &group.functions {
            self.dirty.0[*f] = false;
        }
        for net in relaxed.fires {
            self.fire_net(net, at);
        }
        Ok(())
    }

    /// The passes of `group`, unobserved, until no written byte changes, at most `nets.len() + 1` of them.
    fn relax_passes(&mut self, group: &RelaxGroup<'_>, at: Instant, ambient: &Ambient, cone: bool) -> Result<SettledCycle, Stop> {
        let mut fires: Vec<NetId> = Vec::new();
        for _ in 0..group.nets.len() + 1 {
            let before = group.snapshot(self.store);
            for f in &group.functions {
                let outs = self.call_function(at, ambient, *f, cone)?;
                self.hold_pass(outs, &mut fires);
            }
            let after = group.snapshot(self.store);
            if before == after {
                return Ok(SettledCycle { after, fires });
            }
        }
        Err(Stop::Error(group.unsettled(self.plan, at)))
    }

    /// Writes a pass's outputs to the store with no observation, and gathers the events it fires into `fires`.
    fn hold_pass(&mut self, outs: Vec<Out>, fires: &mut Vec<NetId>) {
        for o in outs {
            if o.event {
                if o.bytes[0] != 0 && !fires.contains(&o.net) {
                    fires.push(o.net);
                }
                continue;
            }
            match o.element {
                Some(element) => self.store.write_element(o.net, element, 0, &o.bytes),
                None => self.store.write(o.net, 0, &o.bytes),
            };
        }
    }
}

/// Where in an instant a microstep runs (§15): an ordinary microstep, or the settle phase that
/// closes the instant of a time point (§15 step 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// A microstep of the loop.
    Microstep,
    /// The settle phase, after the loop is quiet.
    Settle,
}

/// A reaction that runs in the current microstep.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Firing {
    pub instance: usize,
    pub reaction: usize,
    /// It runs in the settle phase: §13 rule 14 says the phase decides, whatever event its head
    /// names, so its commits with no advancing delay are settle commits (§15 S1).
    pub on_settle: bool,
}

impl Firing {
    /// R1: the reactions with an event pin in `on` that fired in this microstep, in (instance, reaction) order.
    pub fn fired(instances: &[InstancePlan], phase: Phase, store: &Store) -> Vec<Firing> {
        let mut out = Vec::new();
        for (i, inst) in instances.iter().enumerate() {
            for (r, reaction) in inst.handle.shape().reactions.iter().enumerate() {
                let on: Vec<NetId> = reaction
                    .on
                    .iter()
                    .filter_map(|p| match inst.pins[*p as usize] {
                        Binding::Net(id) if store.fired(id) => Some(id),
                        _ => None,
                    })
                    .collect();
                if !on.is_empty() {
                    out.push(Firing { instance: i, reaction: r, on_settle: phase == Phase::Settle });
                }
            }
        }
        out
    }
}

/// What the reactions of one microstep asked for beyond their commits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fired {
    /// A reaction returned `FINISH`.
    pub finish: bool,
    /// A reaction that ran is flagged `IRREVERSIBLE`.
    pub irreversible: bool,
    /// How many reactions ran.
    pub ran: u32,
}

/// One commit a reaction made, validated and waiting to be applied to the queue.
struct Planned {
    statement: u32,
    offset: u32,
    bytes: Vec<u8>,
    net: Option<NetId>,
    /// The element of an array net the commit is to.
    element: Option<u64>,
    /// `None`: it never comes due, or (a settle commit) has no instant until it lands.
    due: Option<Instant>,
    replace: bool,
    record: Option<String>,
    /// A settle commit (§15 S1): made in the settle phase with no advancing delay, held by
    /// `SettleCommits` instead of queued.
    settle: bool,
}

/// What one call of a reaction made.
struct Ran {
    plan: Vec<Planned>,
    finish: bool,
}

/// What each reaction of a microstep made on its latest call: one slot per job, empty until the job runs.
struct LatestCalls(Vec<Option<Ran>>);

impl LatestCalls {
    /// `count` empty slots.
    fn new(count: usize) -> LatestCalls {
        LatestCalls((0..count).map(|_| None).collect())
    }

    /// The earliest tick a commit of any reaction but `except` is due at.
    fn earliest_other(&self, except: usize) -> Option<Tick> {
        self.0
            .iter()
            .enumerate()
            .filter(|(k, _)| *k != except)
            .filter_map(|(_, r)| r.as_ref())
            .flat_map(|r| r.plan.iter().filter_map(|p| p.due.map(|d| d.0)))
            .min()
    }
}

/// A reaction of the microstep.
struct Job {
    instance: usize,
    reaction: usize,
    on_settle: bool,
    crossing: bool,
    irreversible: bool,
}

impl Job {
    /// The reactions of `plan` that fired in `phase` on `store`, each with its reach of a `CROSS` watch and its
    /// `IRREVERSIBLE` flag.
    fn list(plan: &DigitalPlan, phase: Phase, store: &Store) -> Vec<Job> {
        Firing::fired(&plan.instances, phase, store)
            .into_iter()
            .map(|f| Job {
                instance: f.instance,
                reaction: f.reaction,
                on_settle: f.on_settle,
                crossing: plan.analysis().crossing_reach(f.instance as u32, f.reaction as u32),
                irreversible: plan.instances[f.instance]
                    .handle
                    .shape()
                    .reactions
                    .get(f.reaction)
                    .is_some_and(|r| r.flags & reaction_flag::IRREVERSIBLE != 0),
            })
            .collect()
    }
}

/// The commits of one call of a reaction as the engine checks them (statement range, `LATER` bound,
/// `Resolution::due`, the element a pin has, P1 per element), each kept as a `Planned` commit. The first
/// commit it refuses is the call's failure: every later one is refused too.
struct CommitCheck<'c> {
    /// The reaction call whose commits are checked.
    job: &'c Job,
    /// The instant of the microstep the call runs in.
    at: Instant,
    /// The instance's path, for the faults.
    path: &'c str,
    /// The bindings of the instance's pins.
    pins: &'c [Binding],
    /// The statements the reaction declares.
    statements: Vec<Statement>,
    /// The names of the instance's pins, in pin order.
    pin_names: Vec<String>,
    /// Every net's spec.
    nets: &'c [NetSpec],
    /// The analyzed plan: which nets have writers.
    analysis: &'c Analysis,
    /// The session resolution: seconds per tick.
    resolution: &'c Resolution,
    /// The commits that passed, in call order.
    made: Vec<Planned>,
    /// The first commit refused, if any.
    failure: Option<DigitalError>,
}

impl CommitCheck<'_> {
    /// The status the call's commit callback returns for `commit`: `OK` when it is kept, `FAULT` when refused.
    fn accept(&mut self, commit: Commit<'_>) -> u32 {
        if self.failure.is_some() {
            return status::FAULT;
        }
        match self.plan_commit(&commit) {
            Ok(planned) => {
                self.made.push(planned);
                status::OK
            }
            Err(e) => {
                self.failure = Some(e);
                status::FAULT
            }
        }
    }

    /// The fault of the instance that names `message`.
    fn reject(&self, message: String) -> DigitalError {
        DigitalError::Fault { code: 0, message, tick: self.at.0.0, instance: self.path.to_string() }
    }

    /// `commit` checked and planned: its due instant, its net and element, its record stream, whether it settles.
    fn plan_commit(&self, c: &Commit<'_>) -> Result<Planned, DigitalError> {
        let s = self.statement(c)?;
        let due = self.due(c)?;
        let (net, element) = self.target(c, s)?;
        let record = net
            .filter(|n| self.nets[n.0 as usize].record)
            .map(|_| format!("{}.{}", self.path, self.pin_names[s.pin as usize]));
        // a record is observed where it is made, `(t, n)` (§19 rules 7, 8): it is no settle commit,
        // which is a change of state due at the next time point (§15 S1); a commit with an advancing
        // delay is an ordinary one (S7)
        let settle = self.job.on_settle && due == Due::NextMicrostep && record.is_none();
        let due = match due {
            // a settle commit has no instant until the owner lands it
            Due::NextMicrostep if settle => None,
            Due::NextMicrostep => Some((self.at.0, self.at.1 + 1)),
            Due::At(t) => Some((t, 0)),
            Due::Never => None,
        };
        let replace = s.flags & statement_flag::REPLACE != 0;
        Ok(Planned { statement: c.statement, offset: c.offset, bytes: c.bytes.to_vec(), net, element, due, replace, record, settle })
    }

    /// The statement `c` commits through, which must exist, hold the committed bytes, and keep a `LATER` bound.
    fn statement(&self, c: &Commit<'_>) -> Result<&Statement, DigitalError> {
        let reaction = self.job.reaction;
        let Some(s) = self.statements.get(c.statement as usize) else {
            return Err(self.reject(format!("reaction {reaction} committed through statement {}, which it does not have", c.statement)));
        };
        let end = c.offset as u64 + c.bytes.len() as u64;
        if c.offset < s.offset || end > s.offset as u64 + s.len as u64 {
            return Err(self.reject(format!(
                "reaction {reaction} statement {} committed bytes {}..{} outside its declared range {}..{}",
                c.statement,
                c.offset,
                end,
                s.offset,
                s.offset as u64 + s.len as u64
            )));
        }
        let later = self.analysis.later(self.job.instance as u32, reaction as u32, c.statement);
        if later && c.after < s.min_after {
            return Err(self.reject(format!(
                "reaction {reaction} statement {} is LATER with a bound of {} s and committed after {} s",
                c.statement, s.min_after, c.after
            )));
        }
        Ok(s)
    }

    /// When `c` is due, a NaN delay naming the instance and statement.
    fn due(&self, c: &Commit<'_>) -> Result<Due, DigitalError> {
        match self.resolution.due(self.at.0, c.after) {
            Ok(d) => Ok(d),
            Err(DigitalError::NanDelay { code, .. }) => Err(DigitalError::NanDelay { code, instance: self.path.to_string(), statement: c.statement }),
            Err(e) => Err(e),
        }
    }

    /// The net and element `c` lands on through statement `s`'s pin (none for a sample or an absent pin): an
    /// element the pin has, and not one a function writes.
    fn target(&self, c: &Commit<'_>, s: &Statement) -> Result<(Option<NetId>, Option<u64>), DigitalError> {
        let nets = self.nets;
        let (net, element) = match (self.pins[s.pin as usize], c.element) {
            (Binding::Net(n), None) if nets[n.0 as usize].count == 0 => (Some(n), None),
            (Binding::Net(n), Some(e)) if e < nets[n.0 as usize].count => (Some(n), Some(e)),
            (Binding::ArrayElement { net, element }, None) => (Some(net), Some(element)),
            (Binding::ArrayWindow { net, first, length }, Some(e)) if e < length => (Some(net), Some(first + e)),
            (Binding::Sample(_) | Binding::Absent, _) => (None, None),
            (binding, element) => {
                return Err(self.reject(format!(
                    "reaction {} statement {} committed to pin {} ({binding:?}) with element {element:?}, which is not an element it has",
                    self.job.reaction, c.statement, self.pin_names[s.pin as usize]
                )));
            }
        };
        // P1 per element: a statement landing on an element a function writes is that element's second writer
        if let (Some(n), Some(e)) = (net, element)
            && self.analysis.function_writes(n, e)
        {
            return Err(self.reject(format!(
                "reaction {} statement {} committed element {e} of array net `{}`, which a function writes (P1: one writer per element)",
                self.job.reaction, c.statement, nets[n.0 as usize].name
            )));
        }
        Ok((net, element))
    }
}

/// R, the reactions of one microstep (abi.md §5): the plan whose devices react, the snapshot they all read, the
/// microstep's ambient values and analog samples, and where their commits go: the queue, the settle commits, and what
/// the run observes. `call` numbers the commits of the microstep (C2).
pub struct ReactionRun<'a> {
    /// The plan whose devices react.
    pub plan: &'a mut DigitalPlan,
    /// The microstep's snapshot (R2).
    pub store: &'a Store,
    /// The continuous values the analog side sampled.
    pub samples: &'a [f64],
    /// The microstep's ambient values.
    pub ambient: &'a Ambient,
    /// The instant of the microstep.
    pub at: Instant,
    /// Where the commits are queued.
    pub queue: &'a mut Queue,
    /// What the run observes.
    pub observer: &'a mut Obs,
    /// The commit counter of the microstep (C2).
    pub call: &'a mut u32,
    /// The settle commits the reactions hold for the settle phase.
    pub settle_commits: &'a mut SettleCommits,
}

impl ReactionRun<'_> {
    /// R: runs, once each, the reactions with an event pin in `on` that fired in this microstep.
    /// Every one reads the same snapshot (R2). `HORIZON` is bounded by what the other reactions of the
    /// microstep commit (abi.md R5 "other than as a consequence of the commits this call makes"): a
    /// reaction is called again with a smaller horizon while another's commits are due before it (a
    /// reaction has no memory, R3, so that is sound); an `IRREVERSIBLE` one runs last and once, with
    /// the horizon the others leave it (the instant itself when several are irreversible). Each commit
    /// then goes through the queue and is observed.
    pub fn run(&mut self, phase: Phase) -> Result<Fired, DigitalError> {
        let jobs = Job::list(self.plan, phase, self.store);
        let calls = self.call_all(&jobs)?;
        Ok(self.queue_commits(&jobs, calls))
    }

    /// The `HORIZON` a job starts from: the crossing horizon when a `CROSS` watch can reach it.
    fn base_horizon(&self, job: &Job) -> Tick {
        if job.crossing { self.ambient.crossing_horizon } else { self.ambient.horizon }
    }

    /// Calls every job: the reversible ones until no other's commit cuts their horizon, then the irreversible ones.
    fn call_all(&mut self, jobs: &[Job]) -> Result<LatestCalls, DigitalError> {
        let at = self.at;
        let reversible: Vec<usize> = (0..jobs.len()).filter(|j| !jobs[*j].irreversible).collect();
        let irreversible: Vec<usize> = (0..jobs.len()).filter(|j| jobs[*j].irreversible).collect();
        // an IRREVERSIBLE reaction runs last and once, so what it commits cannot bound the others: when one
        // fires together with other reactions every reaction of the microstep gets the instant itself
        let pinned = !irreversible.is_empty() && jobs.len() > 1;
        let mut horizon: Vec<Tick> = jobs.iter().map(|j| if pinned { at.0 } else { self.base_horizon(j) }).collect();
        let mut calls = LatestCalls::new(jobs.len());
        for j in &reversible {
            calls.0[*j] = Some(self.call_reaction(&jobs[*j], horizon[*j])?);
        }
        self.cut_horizon(jobs, &reversible, &mut horizon, &mut calls)?;
        for j in &irreversible {
            let base = self.base_horizon(&jobs[*j]);
            let cap = match irreversible.len() {
                _ if pinned => at.0,
                1 => calls.earliest_other(*j).map_or(base, |e| base.min(Tick(e.0.saturating_sub(1)).max(at.0))),
                _ => at.0,
            };
            calls.0[*j] = Some(self.call_reaction(&jobs[*j], cap)?);
        }
        Ok(calls)
    }

    /// Another reaction's commit due before a horizon cuts it down; a cut can move a commit, so it repeats, at most
    /// once per reversible job and then the instant itself, which is always sound.
    fn cut_horizon(&mut self, jobs: &[Job], reversible: &[usize], horizon: &mut [Tick], calls: &mut LatestCalls) -> Result<(), DigitalError> {
        let at = self.at;
        for _ in 0..=reversible.len() {
            let mut changed = false;
            for j in reversible {
                let Some(e) = calls.earliest_other(*j) else { continue };
                let cap = Tick(e.0.saturating_sub(1)).max(at.0);
                if cap < horizon[*j] {
                    horizon[*j] = cap;
                    calls.0[*j] = Some(self.call_reaction(&jobs[*j], cap)?);
                    changed = true;
                }
            }
            if !changed {
                return Ok(());
            }
        }
        // no fixed point within the bound: the instant itself is always sound
        for j in reversible {
            if horizon[*j] > at.0 {
                horizon[*j] = at.0;
                calls.0[*j] = Some(self.call_reaction(&jobs[*j], at.0)?);
            }
        }
        Ok(())
    }

    /// Calls reaction `job` once with `HORIZON` `horizon`, checking each commit it makes (`CommitCheck`).
    /// Nothing of the engine changes.
    fn call_reaction(&mut self, job: &Job, horizon: Tick) -> Result<Ran, DigitalError> {
        let ambient = Ambient { horizon, ..*self.ambient };
        let (store, at) = (self.store, self.at);
        let DigitalPlan { nets, instances, resolution, analysis, .. } = &mut *self.plan;
        let inst = &mut instances[job.instance];
        let path = inst.path.clone();
        let pins = inst.pins.clone();
        let windows = WholeReads::windows(store, inst.handle.shape(), job.reaction, &pins);
        let handles: Vec<ViewHandle<'_>> = windows.iter().map(|window| ViewHandle::new(window)).collect();
        let (statements, pin_names, inputs) = {
            let shape = inst.handle.shape();
            let reaction = shape.reactions.get(job.reaction).expect("listed from this shape");
            let lens = shape.cell_lens_of(&reaction.reads, &reaction.elements, Access::ReactionRead).expect("bind validated the shape");
            let reader = CellReader { pins: &pins, store, ambient: &ambient, samples: self.samples };
            (
                reaction.writes.iter().copied().collect::<Vec<Statement>>(),
                shape.pins.iter().map(|p| p.name.as_str().to_string()).collect::<Vec<String>>(),
                reader.inputs(&reaction.reads, &reaction.elements, &lens, &handles),
            )
        };
        let refs: Vec<&[u8]> = inputs.iter().map(|b| b.bytes()).collect();
        let mut check = CommitCheck { job, at, path: &path, pins: &pins, statements, pin_names, nets, analysis, resolution, made: Vec::new(), failure: None };
        let result = inst.handle.react(job.reaction as u32, &refs, &mut |commit| check.accept(commit));
        let finish = match result {
            Ok(Flow::Finish) => true,
            Ok(Flow::Continue) => false,
            Err(e) => return Err(check.failure.take().unwrap_or_else(|| DigitalError::device_fault(e, at.0, &path))),
        };
        if let Some(e) = check.failure.take() {
            return Err(e);
        }
        Ok(Ran { plan: check.made, finish })
    }

    /// Observes every commit of the microstep, then queues it, holds it as a settle commit or cancels with it.
    fn queue_commits(&mut self, jobs: &[Job], calls: LatestCalls) -> Fired {
        let mut fired = Fired::default();
        for (job, result) in jobs.iter().zip(calls.0) {
            let result = result.expect("every job ran");
            fired.ran += 1;
            fired.irreversible |= job.irreversible;
            fired.finish |= result.finish;
            let path = self.plan.instances[job.instance].path.clone();
            for p in result.plan {
                self.queue_commit(job, &path, p);
            }
        }
        fired
    }

    /// Observes commit `p` of `job` (of the instance at `path`) and its record, then queues it, holds it as a settle
    /// commit, or cancels the pending commit it replaces.
    fn queue_commit(&mut self, job: &Job, path: &str, p: Planned) {
        let at = self.at;
        self.observer.push(Observation::Commit { instance: path.to_string(), reaction: job.reaction as u32, statement: p.statement, made: at, due: p.due });
        let Some(net) = p.net else { return };
        if let Some(stream) = p.record {
            self.observer.push(Observation::Record { stream, reaction: job.reaction as u32, statement: p.statement, at, bytes: p.bytes.clone() });
        }
        *self.call += 1;
        let mut pending = Pending {
            net,
            element: p.element,
            offset: p.offset,
            bytes: p.bytes,
            writer: Writer::Reaction { instance: job.instance as u32, reaction: job.reaction as u32 },
            statement: p.statement,
            made: at,
            call: *self.call,
            due: (at.0, 0),
        };
        if p.settle {
            self.settle_commits.hold(pending, p.replace);
            return;
        }
        match p.due {
            Some(due) => {
                pending.due = due;
                self.queue.push(pending, p.replace);
            }
            None if p.replace => self.queue.cancel(&pending),
            None => {}
        }
    }
}
