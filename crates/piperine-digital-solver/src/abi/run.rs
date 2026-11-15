//! Calling the devices (abi.md §4 functions, §5 reactions): the discrete
//! functions to a fixed point over the levelized order (D), and the reactions
//! of one microstep on one snapshot with the commit path (R).
//!
//! Both read and write through `Store` and call devices only through
//! `piperine_abi::caller::Handle`.

use piperine_abi::caller::CallError;
use piperine_abi::device::{Access, cell_lens};
use piperine_abi::layout::{is_canonical, leaf_size};
use piperine_abi::raw::{Cell, cell, field_flag, status};

use super::dag::Step;
use super::net::{NetId, NetKind};
use super::observe::{Obs, Observation};
use super::plan::{Binding, DigitalPlan};
use super::queue::Instant;
use super::store::{Aligned, Store};
use super::tick::Tick;
use crate::error::DigitalError;

/// The values a call reads that are not pins: time, `STATIC`, `TICK`, `HORIZON`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ambient {
    pub tick: Tick,
    /// Seconds: the tick times the resolution.
    pub time: f64,
    pub is_static: bool,
    pub horizon: Tick,
}

impl Ambient {
    /// The ambient of an instant at `tick`, with `HORIZON` the tick itself.
    pub fn at(tick: Tick, seconds_per_tick: f64) -> Ambient {
        Ambient { tick, time: tick.0 as f64 * seconds_per_tick, is_static: false, horizon: tick }
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

    pub fn is_dirty(&self, f: usize) -> bool {
        self.0[f]
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
        obs.push(Observation::Change { net: plan.nets[net.0 as usize].name.clone(), at, bytes: store.value(net).to_vec() });
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

/// One input or output buffer of `len` bytes, filled with what `cell` names.
fn cell_bytes(
    plan: &DigitalPlan,
    instance: usize,
    c: &Cell,
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
        cell::PIN => match plan.instances[instance].pins[c.index as usize] {
            Binding::Net(id) => match c.part {
                piperine_abi::raw::part::VALUE => fill(&mut buf, store.value(id)),
                piperine_abi::raw::part::PREVIOUS => fill(&mut buf, store.previous(id)),
                piperine_abi::raw::part::CHANGED => fill(&mut buf, &[store.changed(id) as u8]),
                _ => {}
            },
            Binding::Sample(i) => fill(&mut buf, &samples.get(i as usize).copied().unwrap_or(0.0).to_le_bytes()),
            Binding::Absent => {}
        },
        cell::AMBIENT => match c.index {
            piperine_abi::raw::ambient::TIME => fill(&mut buf, &ambient.time.to_le_bytes()),
            piperine_abi::raw::ambient::STATIC => fill(&mut buf, &[ambient.is_static as u8]),
            piperine_abi::raw::ambient::TICK => fill(&mut buf, &ambient.tick.0.to_le_bytes()),
            piperine_abi::raw::ambient::HORIZON => fill(&mut buf, &ambient.horizon.0.to_le_bytes()),
            _ => {}
        },
        _ => {}
    }
    buf
}

/// What a function wrote to one net.
struct Out {
    net: NetId,
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
    let reads = cell_lens(shape, &func.reads, Access::FunctionRead).expect("bind validated the shape");
    let writes = cell_lens(shape, &func.writes, Access::FunctionWrite).expect("bind validated the shape");
    let inputs: Vec<Aligned> = func
        .reads
        .iter()
        .zip(&reads)
        .map(|(c, len)| cell_bytes(plan, node.instance as usize, c, *len, store, ambient, &[]))
        .collect();
    let mut outputs: Vec<Aligned> = func
        .writes
        .iter()
        .zip(&writes)
        .map(|(c, len)| cell_bytes(plan, node.instance as usize, c, *len, store, ambient, &[]))
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
        let Binding::Net(net) = inst.pins[c.index as usize] else { continue };
        let pin = shape.pins.get(c.index as usize).expect("bind validated the index");
        let event = plan.nets[net.0 as usize].kind == NetKind::Event;
        let bytes = buf.bytes().to_vec();
        if cfg!(debug_assertions) && !event && !is_canonical(&pin.layout, &bytes) {
            return Err(Stop::Error(DigitalError::Fault {
                code: 0,
                message: format!("function {} wrote non-canonical bytes to pin {} (P3)", func.name.as_str(), pin.name.as_str()),
                tick: at.0.0,
                instance: inst.path.clone(),
            }));
        }
        out.push(Out { net, event, bytes });
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
/// bytes, the functions run in order until no byte changes, at most `nets.len() + 1` passes; what
/// still moves is zeroed where it has a known byte and a fault where it has none.
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
    let (mut before, mut after) = (snapshot(store), snapshot(store));
    for _ in 0..bound {
        before = snapshot(store);
        for f in fns {
            let outs = call_function(plan, store, at, ambient, *f, cone)?;
            for o in outs {
                if o.event {
                    if o.bytes[0] != 0 && !fires.contains(&o.net) {
                        fires.push(o.net);
                    }
                    continue;
                }
                store.write(o.net, 0, &o.bytes);
            }
        }
        after = snapshot(store);
        if before == after {
            settled = true;
            break;
        }
    }
    if !settled {
        for (i, net) in written.iter().enumerate() {
            if before[i] == after[i] {
                continue;
            }
            let name = &plan.nets[net.0 as usize].name;
            let layout = plan.net_layout(*net).expect("a written net has a pin");
            let mut value = after[i].clone();
            for field in layout.fields.iter() {
                let width = leaf_size(field.leaf).expect("bind validated the layout");
                for e in 0..field.count as usize {
                    let at_v = field.offset as usize + e * width..field.offset as usize + (e + 1) * width;
                    let known = field.known as usize + e;
                    let has_known = field.flags & field_flag::HAS_KNOWN != 0;
                    let moved = before[i][at_v.clone()] != after[i][at_v.clone()]
                        || (has_known && before[i][known] != after[i][known]);
                    if !moved {
                        continue;
                    }
                    if !has_known {
                        let instance = plan.dag().function(fns[0]).name.clone();
                        return Err(Stop::Error(DigitalError::Fault {
                            code: 0,
                            message: format!("net `{name}` did not settle and its field holds no known byte (D1)"),
                            tick: at.0.0,
                            instance,
                        }));
                    }
                    value[at_v].fill(0);
                    value[known] = 0;
                }
            }
            store.write(*net, 0, &value);
            obs.push(Observation::Unsettled { net: name.clone(), at });
            after[i] = value;
        }
    }
    // the run sees one change per net: from the value it had before the group, to the settled one
    for (i, net) in written.iter().enumerate() {
        store.restore_net(*net, &saved[i]);
        write_net(plan, store, dirty, obs, *net, 0, &after[i], at);
    }
    for net in fires {
        fire_net(plan, store, dirty, obs, net, at);
    }
    Ok(())
}
