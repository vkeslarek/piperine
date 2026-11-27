//! Calling the devices (abi.md §4 functions, §5 reactions): the discrete
//! functions to a fixed point over the levelized order (D), and the reactions
//! of one microstep on one snapshot with the commit path (R).
//!
//! Both read and write through `Store` and call devices only through
//! `piperine_abi::caller::Handle`.

use piperine_abi::caller::{CallError, Commit, ElementSource, ViewHandle};
use piperine_abi::device::{Access, Flow};
use piperine_abi::raw::{ArrayView, Cell, Shape, Statement, WHOLE, cell, part, reaction_flag, statement_flag, status};

use crate::dag::Step;
use crate::net::{NetId, NetKind};
use crate::observe::{Obs, Observation};
use crate::plan::{Binding, DigitalPlan, InstancePlan};
use crate::queue::{Instant, Pending, Queue, Writer};
use crate::settle::SettleCommits;
use crate::store::{Aligned, Store};
use crate::tick::{Due, Tick};
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

    pub fn none(plan: &DigitalPlan) -> DirtySet {
        DirtySet(vec![false; plan.dag().len()])
    }

    /// Marks every function that reads `net`.
    pub fn mark_net(&mut self, plan: &DigitalPlan, net: NetId) {
        for f in plan.dag().readers(net) {
            self.0[*f] = true;
        }
    }

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

fn fault(e: CallError, tick: Tick, instance: &str) -> DigitalError {
    DigitalError::Fault { code: e.code, message: e.message, tick: tick.0, instance: instance.to_string() }
}

/// Writes `bytes` at `offset` of `net`: observes and dirties the readers on a change.
pub fn write_net(
    plan: &DigitalPlan,
    store: &mut Store,
    dirty: &mut DirtySet,
    obs: &mut Obs,
    net: NetId,
    offset: u32,
    bytes: &[u8],
    at: Instant,
) -> bool {
    let changed = store.write(net, offset, bytes);
    if changed {
        // the pin of a record stream delivers the record (`Observation::Record`), not a value of a net: it may share its path
        // with a signal of the same size, and a host could not tell the two apart
        if !plan.nets[net.0 as usize].record {
            obs.push(Observation::Change { net: plan.nets[net.0 as usize].name.clone(), at, bytes: store.value(net).to_vec() });
        }
        dirty.mark_net(plan, net);
    }
    changed
}

/// Writes `bytes` at `offset` of `element` of array net `net`: observes and dirties the readers on a change.
pub fn write_element(
    plan: &DigitalPlan,
    store: &mut Store,
    dirty: &mut DirtySet,
    obs: &mut Obs,
    net: NetId,
    element: u64,
    offset: u32,
    bytes: &[u8],
    at: Instant,
) -> bool {
    let changed = store.write_element(net, element, offset, bytes);
    if changed {
        let name = format!("{}[{element}]", plan.nets[net.0 as usize].name);
        obs.push(Observation::Change { net: name, at, bytes: store.element_value(net, element).to_vec() });
        dirty.mark_net(plan, net);
    }
    changed
}

/// Fires event net `net`: observes it and dirties the functions that read its `CHANGED`.
pub fn fire_net(plan: &DigitalPlan, store: &mut Store, dirty: &mut DirtySet, obs: &mut Obs, net: NetId, at: Instant) {
    store.fire(net);
    obs.push(Observation::Fire { net: plan.nets[net.0 as usize].name.clone(), at });
    dirty.mark_net(plan, net);
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
    fn windows_of<'s>(store: &'s Store, shape: &Shape, reads: &piperine_abi::raw::List<'_, Cell>, elements: &piperine_abi::raw::List<'_, u64>, pins: &[Binding]) -> Vec<WindowView<'s>> {
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

/// One input or output buffer of `len` bytes, filled with what `cell` names.
fn cell_bytes(
    pins: &[Binding],
    c: &Cell,
    element: u64,
    len: usize,
    store: &Store,
    ambient: &Ambient,
    samples: &[f64],
) -> Aligned {
    let mut buf = Aligned::zeroed(len);
    let fill = |buf: &mut Aligned, src: &[u8]| {
        let n = src.len().min(len);
        buf.bytes_mut()[..n].copy_from_slice(&src[..n]);
    };
    match c.kind {
        cell::PIN => match pins[c.index as usize] {
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
            Binding::Sample(i) => fill(&mut buf, &samples.get(i as usize).copied().unwrap_or(0.0).to_le_bytes()),
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

/// What a function wrote to one net.
struct Out {
    net: NetId,
    /// The element of an array net the write is to (a pin bound to a window of one element).
    element: Option<u64>,
    event: bool,
    bytes: Vec<u8>,
}

/// Calls function `f` of the dag on the store's current bytes and returns what it wrote.
fn call_function(
    plan: &DigitalPlan,
    store: &Store,
    at: Instant,
    ambient: &Ambient,
    f: usize,
    cone: bool,
) -> Result<Vec<Out>, Stop> {
    let node = plan.dag().function(f);
    let inst = &plan.instances[node.instance as usize];
    let shape = inst.handle.shape();
    let func = shape.functions.get(node.k as usize).expect("bind took this function from the shape");
    let reads = shape.cell_lens_of(&func.reads, &func.elements, Access::FunctionRead).expect("bind validated the shape");
    let writes = shape.cell_lens_of(&func.writes, &piperine_abi::raw::List::empty(), Access::FunctionWrite).expect("bind validated the shape");
    // a function reads an array at a run-time element through its view (R2), as a reaction does
    let windows = WholeReads::windows_of(store, shape, &func.reads, &func.elements, &inst.pins);
    let handles: Vec<ViewHandle<'_>> = windows.iter().map(|window| ViewHandle::new(window)).collect();
    let mut whole = handles.iter();
    let inputs: Vec<Aligned> = func
        .reads
        .iter()
        .zip(&reads)
        .enumerate()
        .map(|(i, (c, len))| match func.elements.get(i).copied().unwrap_or(0) {
            WHOLE => {
                let view = whole.next().expect("one view per WHOLE read").view();
                let mut buffer = Aligned::zeroed(*len);
                buffer.bytes_mut().copy_from_slice(&ArrayView::as_bytes(&view)[..*len]);
                buffer
            }
            element => cell_bytes(&inst.pins, c, element, *len, store, ambient, &[]),
        })
        .collect();
    let mut outputs: Vec<Aligned> = func
        .writes
        .iter()
        .zip(&writes)
        .map(|(c, len)| cell_bytes(&inst.pins, c, 0, *len, store, ambient, &[]))
        .collect();
    let input_refs: Vec<&[u8]> = inputs.iter().map(|b| b.bytes()).collect();
    let mut output_refs: Vec<&mut [u8]> = outputs.iter_mut().map(|b| b.bytes_mut()).collect();
    let partials = func.partials.len();
    let seed = vec![0u8; partials];
    let mut jac = vec![0.0f64; partials];
    if let Err(e) = inst.handle.eval(node.k, &input_refs, &mut output_refs, &seed, &mut jac) {
        if e.status == status::RESHAPE && cone {
            return Err(Stop::Reshape(inst.path.clone()));
        }
        return Err(Stop::Error(fault(e, at.0, &inst.path)));
    }
    let mut out = Vec::new();
    for (c, buf) in func.writes.iter().zip(&outputs) {
        let (net, element) = match inst.pins[c.index as usize] {
            Binding::Net(net) => (net, None),
            Binding::ArrayElement { net, element } => (net, Some(element)),
            _ => continue,
        };
        let event = plan.nets[net.0 as usize].kind == NetKind::Event;
        let bytes = buf.bytes().to_vec();
        out.push(Out { net, element, event, bytes });
    }
    Ok(out)
}

/// Applies a function's outputs the way a microstep sees them.
fn apply(plan: &DigitalPlan, store: &mut Store, dirty: &mut DirtySet, obs: &mut Obs, at: Instant, outs: Vec<Out>) {
    for o in outs {
        if o.event {
            if o.bytes[0] != 0 {
                fire_net(plan, store, dirty, obs, o.net, at);
            }
        } else if let Some(element) = o.element {
            write_element(plan, store, dirty, obs, o.net, element, 0, &o.bytes, at);
        } else {
            write_net(plan, store, dirty, obs, o.net, 0, &o.bytes, at);
        }
    }
}

/// D: runs the dirty functions in the dag's order, relaxing the cycles through `RELAX` nets,
/// until the order is done (abi.md §3.1 P2, §9 D1). A fault, or a `RESHAPE` (which a committed
/// store never produces, P6), is a `Fault`.
pub fn run_functions(
    plan: &DigitalPlan,
    store: &mut Store,
    dirty: &mut DirtySet,
    at: Instant,
    ambient: &Ambient,
    obs: &mut Obs,
) -> Result<(), DigitalError> {
    match walk(plan, store, dirty, at, ambient, obs, None) {
        Ok(()) => Ok(()),
        Err(Stop::Error(e)) => Err(e),
        Err(Stop::Reshape(instance)) => unreachable!("a walk without a cone filter turns RESHAPE into a fault: {instance}"),
    }
}

/// D restricted to the parameter cone, for a shadow of the store (abi.md P6): the path of the instance
/// whose function returned `RESHAPE`, if one did.
pub fn run_cone(
    plan: &DigitalPlan,
    store: &mut Store,
    dirty: &mut DirtySet,
    at: Instant,
    ambient: &Ambient,
    obs: &mut Obs,
) -> Result<Option<String>, DigitalError> {
    match walk(plan, store, dirty, at, ambient, obs, Some(plan.cone())) {
        Ok(()) => Ok(None),
        Err(Stop::Error(e)) => Err(e),
        Err(Stop::Reshape(instance)) => Ok(Some(instance)),
    }
}

fn walk(
    plan: &DigitalPlan,
    store: &mut Store,
    dirty: &mut DirtySet,
    at: Instant,
    ambient: &Ambient,
    obs: &mut Obs,
    cone: Option<&[bool]>,
) -> Result<(), Stop> {
    let allowed = |f: usize| cone.is_none_or(|c| c[f]);
    for step in plan.dag().order() {
        match step {
            Step::Run(f) => {
                if !dirty.0[*f] || !allowed(*f) {
                    continue;
                }
                dirty.0[*f] = false;
                let outs = call_function(plan, store, at, ambient, *f, cone.is_some())?;
                apply(plan, store, dirty, obs, at, outs);
            }
            Step::Relax { fns, nets } => {
                if !fns.iter().any(|f| dirty.0[*f]) || !fns.iter().all(|f| allowed(*f)) {
                    continue;
                }
                for f in fns {
                    dirty.0[*f] = false;
                }
                relax(plan, store, dirty, at, ambient, obs, fns, nets, cone.is_some())?;
            }
        }
    }
    Ok(())
}

/// The cycle of `fns` through the `RELAX` nets `nets` (abi.md §9 D1): those nets start at zero
/// bytes (every drive absent: `driving` 0), the functions run in order until no byte changes, at
/// most `nets.len() + 1` passes. A cycle that still moves after them is `E0511`, naming the loop's
/// drives in name order (spec §8.2.3 step 4). Each pass runs first the functions that write no `RELAX` net (the
/// resolvers and what derives from them), then the drives, so the first pass of a loop reads what `resolve` of the
/// absent drives gives and not what an earlier relaxation left on those nets (§8.2.3 steps 1 and 2).
fn relax(
    plan: &DigitalPlan,
    store: &mut Store,
    dirty: &mut DirtySet,
    at: Instant,
    ambient: &Ambient,
    obs: &mut Obs,
    fns: &[usize],
    nets: &[NetId],
    cone: bool,
) -> Result<(), Stop> {
    let writes_relax = |f: &usize| plan.dag().function(*f).writes.iter().any(|w| nets.contains(w));
    let fns: Vec<usize> = fns.iter().copied().filter(|f| !writes_relax(f)).chain(fns.iter().copied().filter(|f| writes_relax(f))).collect();
    let fns = fns.as_slice();
    let mut written: Vec<NetId> = Vec::new();
    for f in fns {
        for w in &plan.dag().function(*f).writes {
            if !written.contains(w) {
                written.push(*w);
            }
        }
    }
    let saved: Vec<_> = written.iter().map(|n| store.save_net(*n)).collect();
    for n in nets {
        let zeros = vec![0u8; store.value(*n).len()];
        store.write(*n, 0, &zeros);
    }
    let snapshot = |store: &Store| -> Vec<Vec<u8>> { written.iter().map(|n| store.value(*n).to_vec()).collect() };
    let mut fires: Vec<NetId> = Vec::new();
    let bound = nets.len() + 1;
    let mut settled = false;
    let mut after = snapshot(store);
    for _ in 0..bound {
        let before = snapshot(store);
        for f in fns {
            let outs = call_function(plan, store, at, ambient, *f, cone)?;
            for o in outs {
                if o.event {
                    if o.bytes[0] != 0 && !fires.contains(&o.net) {
                        fires.push(o.net);
                    }
                    continue;
                }
                match o.element {
                    Some(element) => store.write_element(o.net, element, 0, &o.bytes),
                    None => store.write(o.net, 0, &o.bytes),
                };
            }
        }
        after = snapshot(store);
        if before == after {
            settled = true;
            break;
        }
    }
    if !settled {
        let mut drives: Vec<String> = nets.iter().map(|n| plan.nets[n.0 as usize].name.clone()).collect();
        drives.sort();
        return Err(Stop::Error(DigitalError::Unsettled { code: "E0511", tick: at.0.0, drives }));
    }
    // the run sees one change per net: from the value it had before the group, to the settled one
    for (i, net) in written.iter().enumerate() {
        store.restore_net(*net, &saved[i]);
        write_net(plan, store, dirty, obs, *net, 0, &after[i], at);
    }
    // the group has just settled on what its closing writes show: they are no reason to run it again
    for f in fns {
        dirty.0[*f] = false;
    }
    for net in fires {
        fire_net(plan, store, dirty, obs, net, at);
    }
    Ok(())
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

/// R1: the reactions with an event pin in `on` that fired in this microstep, in (instance, reaction) order.
pub fn fired_reactions(instances: &[InstancePlan], phase: Phase, store: &Store) -> Vec<Firing> {
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

/// A reaction of the microstep.
struct Job {
    instance: usize,
    reaction: usize,
    on_settle: bool,
    crossing: bool,
    irreversible: bool,
}

/// Calls reaction `job` once with `ambient` and `samples`, validating each commit it makes (statement
/// range, `LATER` bound, `Resolution::due`) and marking a settle commit. Nothing of the engine changes.
fn call_reaction(
    plan: &mut DigitalPlan,
    store: &Store,
    samples: &[f64],
    ambient: &Ambient,
    at: Instant,
    job: &Job,
) -> Result<Ran, DigitalError> {
    let DigitalPlan { nets, instances, resolution, analysis, .. } = plan;
    let inst = &mut instances[job.instance];
    let path = inst.path.clone();
    let pins = inst.pins.clone();
    let windows = WholeReads::windows(store, inst.handle.shape(), job.reaction, &pins);
    let handles: Vec<ViewHandle<'_>> = windows.iter().map(|window| ViewHandle::new(window)).collect();
    let (statements, pin_names, inputs) = {
        let mut whole = handles.iter();
        let shape = inst.handle.shape();
        let reaction = shape.reactions.get(job.reaction).expect("listed from this shape");
        let lens = shape.cell_lens_of(&reaction.reads, &reaction.elements, Access::ReactionRead).expect("bind validated the shape");
        let inputs: Vec<Aligned> = reaction
            .reads
            .iter()
            .zip(&lens)
            .enumerate()
            .map(|(i, (c, len))| {
                let element = reaction.elements.get(i).copied().unwrap_or(0);
                if element == WHOLE {
                    let view = whole.next().expect("one view per WHOLE read").view();
                    let mut buffer = Aligned::zeroed(*len);
                    buffer.bytes_mut().copy_from_slice(&ArrayView::as_bytes(&view)[..*len]);
                    buffer
                } else {
                    cell_bytes(&pins, c, element, *len, store, ambient, samples)
                }
            })
            .collect();
        (
            reaction.writes.iter().copied().collect::<Vec<Statement>>(),
            shape.pins.iter().map(|p| p.name.as_str().to_string()).collect::<Vec<String>>(),
            inputs,
        )
    };
    let refs: Vec<&[u8]> = inputs.iter().map(|b| b.bytes()).collect();
    let mut made: Vec<Planned> = Vec::new();
    let mut failure: Option<DigitalError> = None;
    let mut sink = |c: Commit<'_>| -> u32 {
        if failure.is_some() {
            return status::FAULT;
        }
        let reject = |message: String| DigitalError::Fault { code: 0, message, tick: at.0.0, instance: path.clone() };
        let Some(s) = statements.get(c.statement as usize) else {
            failure = Some(reject(format!("reaction {} committed through statement {}, which it does not have", job.reaction, c.statement)));
            return status::FAULT;
        };
        let end = c.offset as u64 + c.bytes.len() as u64;
        if c.offset < s.offset || end > s.offset as u64 + s.len as u64 {
            failure = Some(reject(format!(
                "reaction {} statement {} committed bytes {}..{} outside its declared range {}..{}",
                job.reaction,
                c.statement,
                c.offset,
                end,
                s.offset,
                s.offset as u64 + s.len as u64
            )));
            return status::FAULT;
        }
        let later = analysis.later(job.instance as u32, job.reaction as u32, c.statement);
        if later && c.after < s.min_after {
            failure = Some(reject(format!(
                "reaction {} statement {} is LATER with a bound of {} s and committed after {} s",
                job.reaction, c.statement, s.min_after, c.after
            )));
            return status::FAULT;
        }
        let due = match resolution.due(at.0, c.after) {
            Ok(d) => d,
            Err(DigitalError::NanDelay { code, .. }) => {
                failure = Some(DigitalError::NanDelay { code, instance: path.clone(), statement: c.statement });
                return status::FAULT;
            }
            Err(e) => {
                failure = Some(e);
                return status::FAULT;
            }
        };
        let (net, element) = match (pins[s.pin as usize], c.element) {
            (Binding::Net(n), None) if nets[n.0 as usize].count == 0 => (Some(n), None),
            (Binding::Net(n), Some(e)) if e < nets[n.0 as usize].count => (Some(n), Some(e)),
            (Binding::ArrayElement { net, element }, None) => (Some(net), Some(element)),
            (Binding::ArrayWindow { net, first, length }, Some(e)) if e < length => (Some(net), Some(first + e)),
            (Binding::Sample(_) | Binding::Absent, _) => (None, None),
            (binding, element) => {
                failure = Some(reject(format!(
                    "reaction {} statement {} committed to pin {} ({binding:?}) with element {element:?}, which is not an element it has",
                    job.reaction, c.statement, pin_names[s.pin as usize]
                )));
                return status::FAULT;
            }
        };
        // P1 per element: a statement landing on an element a function writes is that element's second writer
        if let (Some(n), Some(e)) = (net, element)
            && analysis.function_writes(n, e)
        {
            failure = Some(reject(format!(
                "reaction {} statement {} committed element {e} of array net `{}`, which a function writes (P1: one writer per element)",
                job.reaction, c.statement, nets[n.0 as usize].name
            )));
            return status::FAULT;
        }
        let record = net
            .filter(|n| nets[n.0 as usize].record)
            .map(|_| format!("{path}.{}", pin_names[s.pin as usize]));
        // a record is observed where it is made, `(t, n)` (§19 rules 7, 8): it is no settle commit,
        // which is a change of state due at the next time point (§15 S1); a commit with an advancing
        // delay is an ordinary one (S7)
        let settle = job.on_settle && due == Due::NextMicrostep && record.is_none();
        let due = match due {
            // a settle commit has no instant until the owner lands it
            Due::NextMicrostep if settle => None,
            Due::NextMicrostep => Some((at.0, at.1 + 1)),
            Due::At(t) => Some((t, 0)),
            Due::Never => None,
        };
        made.push(Planned {
            statement: c.statement,
            offset: c.offset,
            bytes: c.bytes.to_vec(),
            net,
            element,
            due,
            replace: s.flags & statement_flag::REPLACE != 0,
            record,
            settle,
        });
        status::OK
    };
    let result = inst.handle.react(job.reaction as u32, &refs, &mut sink);
    let finish = match result {
        Ok(Flow::Finish) => true,
        Ok(Flow::Continue) => false,
        Err(e) => return Err(failure.take().unwrap_or_else(|| fault(e, at.0, &path))),
    };
    if let Some(e) = failure.take() {
        return Err(e);
    }
    Ok(Ran { plan: made, finish })
}

/// The earliest tick a commit of any reaction but `except` is due at.
fn earliest_other(ran: &[Option<Ran>], except: usize) -> Option<Tick> {
    ran.iter()
        .enumerate()
        .filter(|(k, _)| *k != except)
        .filter_map(|(_, r)| r.as_ref())
        .flat_map(|r| r.plan.iter().filter_map(|p| p.due.map(|d| d.0)))
        .min()
}

/// R: runs, once each, the reactions with an event pin in `on` that fired in this microstep.
/// Every one reads the same snapshot (R2). `HORIZON` is bounded by what the other reactions of the
/// microstep commit (abi.md R5 "other than as a consequence of the commits this call makes"): a
/// reaction is called again with a smaller horizon while another's commits are due before it (a
/// reaction has no memory, R3, so that is sound); an `IRREVERSIBLE` one runs last and once, with
/// the horizon the others leave it (the instant itself when several are irreversible). Each commit
/// then goes through the queue and is observed. `call` numbers the commits of the microstep (C2).
#[allow(clippy::too_many_arguments)]
pub fn run_reactions(
    plan: &mut DigitalPlan,
    store: &Store,
    samples: &[f64],
    ambient: &Ambient,
    queue: &mut Queue,
    obs: &mut Obs,
    at: Instant,
    call: &mut u32,
    settle: &mut SettleCommits,
    phase: Phase,
) -> Result<Fired, DigitalError> {
    let jobs: Vec<Job> = fired_reactions(&plan.instances, phase, store)
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
        .collect();
    let with = |horizon: Tick| Ambient { horizon, ..*ambient };
    let base = |j: &Job| if j.crossing { ambient.crossing_horizon } else { ambient.horizon };
    let reversible: Vec<usize> = (0..jobs.len()).filter(|j| !jobs[*j].irreversible).collect();
    let irreversible: Vec<usize> = (0..jobs.len()).filter(|j| jobs[*j].irreversible).collect();
    // an IRREVERSIBLE reaction runs last and once, so what it commits cannot bound the others: when one
    // fires together with other reactions every reaction of the microstep gets the instant itself
    let pinned = !irreversible.is_empty() && jobs.len() > 1;
    let mut horizon: Vec<Tick> = jobs.iter().map(|j| if pinned { at.0 } else { base(j) }).collect();
    let mut ran: Vec<Option<Ran>> = (0..jobs.len()).map(|_| None).collect();

    for j in &reversible {
        ran[*j] = Some(call_reaction(plan, store, samples, &with(horizon[*j]), at, &jobs[*j])?);
    }
    // another reaction's commit due before a horizon cuts it down; a cut can move a commit, so repeat
    let mut settled = false;
    for _ in 0..=reversible.len() {
        let mut changed = false;
        for j in &reversible {
            let Some(e) = earliest_other(&ran, *j) else { continue };
            let cap = Tick(e.0.saturating_sub(1)).max(at.0);
            if cap < horizon[*j] {
                horizon[*j] = cap;
                ran[*j] = Some(call_reaction(plan, store, samples, &with(cap), at, &jobs[*j])?);
                changed = true;
            }
        }
        if !changed {
            settled = true;
            break;
        }
    }
    if !settled {
        // no fixed point within the bound: the instant itself is always sound
        for j in &reversible {
            if horizon[*j] > at.0 {
                horizon[*j] = at.0;
                ran[*j] = Some(call_reaction(plan, store, samples, &with(at.0), at, &jobs[*j])?);
            }
        }
    }
    for j in &irreversible {
        let cap = match irreversible.len() {
            _ if pinned => at.0,
            1 => earliest_other(&ran, *j).map_or(base(&jobs[*j]), |e| base(&jobs[*j]).min(Tick(e.0.saturating_sub(1)).max(at.0))),
            _ => at.0,
        };
        ran[*j] = Some(call_reaction(plan, store, samples, &with(cap), at, &jobs[*j])?);
    }

    let mut fired = Fired::default();
    for (job, result) in jobs.iter().zip(ran) {
        let result = result.expect("every job ran");
        fired.ran += 1;
        fired.irreversible |= job.irreversible;
        fired.finish |= result.finish;
        let path = plan.instances[job.instance].path.clone();
        for p in result.plan {
            obs.push(Observation::Commit {
                instance: path.clone(),
                reaction: job.reaction as u32,
                statement: p.statement,
                made: at,
                due: p.due,
            });
            let Some(net) = p.net else { continue };
            if let Some(stream) = p.record {
                obs.push(Observation::Record { stream, reaction: job.reaction as u32, statement: p.statement, at, bytes: p.bytes.clone() });
            }
            *call += 1;
            let mut pending = Pending {
                net,
                element: p.element,
                offset: p.offset,
                bytes: p.bytes,
                writer: Writer::Reaction { instance: job.instance as u32, reaction: job.reaction as u32 },
                statement: p.statement,
                made: at,
                call: *call,
                due: (at.0, 0),
            };
            if p.settle {
                settle.hold(pending, p.replace);
                continue;
            }
            match p.due {
                Some(due) => {
                    pending.due = due;
                    queue.push(pending, p.replace);
                }
                None if p.replace => queue.cancel(&pending),
                None => {}
            }
        }
    }
    Ok(fired)
}
