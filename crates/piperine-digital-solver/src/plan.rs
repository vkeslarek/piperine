//! The input contract of the engine and the discrete half of bind (abi.md
//! §3.1 P1 and P4, §6 C1, §9): which net each pin is on, who writes each net,
//! which functions the engine runs and which statements keep `LATER`.
//!
//! The rules need shapes, not devices, so `analyze` takes bare shapes;
//! `DigitalPlan::bind` adds the live instances.

use piperine_abi::caller::Handle;
use piperine_abi::layout::layout_eq;
use piperine_abi::raw::{Layout, Shape, cell, global, kind, pin_flag, role, statement_flag};
use piperine_abi::validate::validate;

use crate::dag::Dag;
use crate::net::{NetId, NetKind, NetSpec};
use crate::tick::Resolution;
use crate::error::DigitalError;

/// What one pin of an instance is bound to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Binding {
    /// A discrete or event pin on this net; an array pin bound to a whole array net of equal `count` (P4).
    Net(NetId),
    /// A scalar discrete pin on one element of an array net (a window of one element, CV-23).
    ArrayElement { net: NetId, element: u64 },
    /// An array pin on `length` consecutive elements of an array net from `first` (a sub-array window, CV-23).
    ArrayWindow { net: NetId, first: u64, length: u64 },
    /// A continuous pin a reaction reads: an index into the owner's sample vector.
    Sample(u32),
    /// An unbound `OPTIONAL` pin: it reads as zero bytes.
    Absent,
}

impl Binding {
    /// The net a pin is on, whatever part of it.
    pub fn net(&self) -> Option<NetId> {
        match self {
            Binding::Net(net) | Binding::ArrayElement { net, .. } | Binding::ArrayWindow { net, .. } => Some(*net),
            Binding::Sample(_) | Binding::Absent => None,
        }
    }

    /// The elements of its array net a pin covers: `(first, length)` for a window or an element, `None` otherwise
    /// (a whole net is every element).
    pub fn elements(&self) -> Option<(u64, u64)> {
        match self {
            Binding::ArrayElement { element, .. } => Some((*element, 1)),
            Binding::ArrayWindow { first, length, .. } => Some((*first, *length)),
            _ => None,
        }
    }
}

/// An instance as the rules see it: its path, its shape, its pin bindings.
#[derive(Clone, Copy, Debug)]
pub struct ShapeInput<'a> {
    pub path: &'a str,
    pub shape: &'a Shape<'a>,
    pub pins: &'a [Binding],
}

/// Who writes a net (P1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetWriter {
    /// Nobody in the design: the host (also `START` and `SETTLE`, written by the owner).
    Host,
    /// The function whose `writes` names the net.
    Function { instance: u32, k: u32 },
    /// The instance whose reaction statements name the net.
    Statements { instance: u32 },
}

/// A discrete function and the nets it depends on and produces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FnNode {
    /// `<instance path>.<function name>`, for the errors that name it.
    pub name: String,
    pub instance: u32,
    pub k: u32,
    pub reads: Vec<NetId>,
    pub writes: Vec<NetId>,
}

/// The result of the discrete bind rules.
#[derive(Clone, Debug)]
pub struct Analysis {
    writers: Vec<NetWriter>,
    functions: Vec<FnNode>,
    /// `[instance][reaction][statement]`: whether the statement keeps `LATER`.
    later: Vec<Vec<Vec<bool>>>,
    start_nets: Vec<NetId>,
    settle_nets: Vec<NetId>,
    /// Per net: some pin on it has role `PARAM` or `GLOBAL`.
    param_nets: Vec<bool>,
    /// `[instance][reaction]`: running it makes the instant a time point (abi.md §5 R4).
    probes: Vec<Vec<bool>>,
    /// `[instance][reaction]`: the reaction reads a continuous pin itself (R4, second clause).
    samples: Vec<Vec<bool>>,
    /// `[instance][reaction]`: a `CROSS` watch can reach what the reaction listens to or reads.
    crossing: Vec<Vec<bool>>,
}

impl Analysis {
    /// Whether a `CROSS` watch can reach reaction `reaction` of `instance` (abi.md R5): its
    /// `HORIZON` is then bounded by the owner's `crossing` limit.
    pub fn crossing_reach(&self, instance: u32, reaction: u32) -> bool {
        self.crossing[instance as usize][reaction as usize]
    }

    /// Whether reaction `reaction` of `instance` reads a continuous pin, or has a statement without
    /// `LATER` whose net reaches, through reads and writes, the analog side or such a reaction.
    pub fn probes(&self, instance: u32, reaction: u32) -> bool {
        self.probes[instance as usize][reaction as usize]
    }

    /// Whether reaction `reaction` of `instance` reads a continuous pin itself, apart from the
    /// reach of its statements.
    pub fn samples(&self, instance: u32, reaction: u32) -> bool {
        self.samples[instance as usize][reaction as usize]
    }

    /// Whether a `PARAM` or `GLOBAL` pin is bound to `net`.
    pub fn is_param_net(&self, net: NetId) -> bool {
        self.param_nets[net.0 as usize]
    }

    /// The nets of the global event `START`.
    pub fn start_nets(&self) -> &[NetId] {
        &self.start_nets
    }

    /// The nets of the global event `SETTLE`.
    pub fn settle_nets(&self) -> &[NetId] {
        &self.settle_nets
    }

    /// The writer of `net`.
    pub fn writer(&self, net: NetId) -> NetWriter {
        self.writers[net.0 as usize]
    }

    /// The discrete functions, in (instance, function) order.
    pub fn functions(&self) -> &[FnNode] {
        &self.functions
    }

    /// Whether a statement still carries `LATER` after bind (C1, C18).
    pub fn later(&self, instance: u32, reaction: u32, statement: u32) -> bool {
        self.later
            .get(instance as usize)
            .and_then(|r| r.get(reaction as usize))
            .and_then(|s| s.get(statement as usize))
            .copied()
            .unwrap_or(false)
    }
}

fn bind_error(rule: &'static str, what: Vec<String>) -> DigitalError {
    DigitalError::Bind { rule, what }
}

/// The text naming pin `i` of an instance.
fn pin_name(input: &ShapeInput<'_>, i: usize) -> String {
    let name = input.shape.pins.get(i).map(|p| p.name.as_str()).unwrap_or("?");
    format!("{}.{}", input.path, name)
}

/// The nets a list of cells names, each once, in first-use order.
fn nets_of<'a>(input: &ShapeInput<'_>, cells: impl Iterator<Item = &'a piperine_abi::raw::Cell>) -> Vec<NetId> {
    let mut out = Vec::new();
    for c in cells.filter(|c| c.kind == cell::PIN) {
        if let Some(id) = input.pins.get(c.index as usize).and_then(Binding::net)
            && !out.contains(&id)
        {
            out.push(id);
        }
    }
    out
}

/// Runs the discrete bind rules over `inputs`: shape validation, the bindings
/// themselves, P4 and P1, and reads off the function list and the surviving
/// `LATER` flags.
pub fn analyze(nets: &[NetSpec], inputs: &[ShapeInput<'_>], resolution: Resolution) -> Result<Analysis, DigitalError> {
    for input in inputs {
        validate(input.shape).map_err(|e| bind_error("validate", vec![format!("{}: {e}", input.path)]))?;
        if input.pins.len() != input.shape.pins.len() {
            return Err(bind_error(
                "bindings",
                vec![format!("{} has {} pins and {} bindings", input.path, input.shape.pins.len(), input.pins.len())],
            ));
        }
    }
    check_bindings(nets, inputs)?;
    check_layouts(nets, inputs)?;
    let writers = find_writers(nets, inputs)?;
    let mut functions = Vec::new();
    let mut later = Vec::new();
    for (n, input) in inputs.iter().enumerate() {
        let instance = n as u32;
        for (k, f) in input.shape.functions.iter().enumerate() {
            let discrete = f.writes.len() > 0 && f.writes.iter().all(|c| c.kind == cell::PIN);
            if discrete {
                functions.push(FnNode {
                    name: format!("{}.{}", input.path, f.name.as_str()),
                    instance,
                    k: k as u32,
                    reads: nets_of(input, f.reads.iter()),
                    writes: nets_of(input, f.writes.iter()),
                });
            }
        }
        later.push(
            input
                .shape
                .reactions
                .iter()
                .map(|r| {
                    r.writes
                        .iter()
                        .map(|s| {
                            s.flags & statement_flag::LATER != 0 && s.min_after >= resolution.seconds_per_tick
                        })
                        .collect()
                })
                .collect(),
        );
    }
    let globals = |which: u32| -> Vec<NetId> {
        let mut found: Vec<NetId> = Vec::new();
        for input in inputs {
            for (i, pin) in input.shape.pins.iter().enumerate() {
                if let (role::GLOBAL, Some(id)) = (pin.role, input.pins[i].net())
                    && pin.global == which
                    && !found.contains(&id)
                {
                    found.push(id);
                }
            }
        }
        found
    };
    let mut param_nets = vec![false; nets.len()];
    for input in inputs {
        for (i, pin) in input.shape.pins.iter().enumerate() {
            if let Some(id) = input.pins[i].net()
                && (pin.role == role::PARAM || pin.role == role::GLOBAL)
            {
                param_nets[id.0 as usize] = true;
            }
        }
    }
    let samples: Vec<Vec<bool>> = inputs
        .iter()
        .map(|input| {
            input
                .shape
                .reactions
                .iter()
                .map(|reaction| {
                    reaction.reads.iter().any(|c| {
                        c.kind == cell::PIN
                            && input.shape.pins.get(c.index as usize).is_some_and(|p| p.kind == kind::CONTINUOUS)
                    })
                })
                .collect()
        })
        .collect();
    let probes = reachability(nets, inputs, &functions, &later, &samples);
    let crossing = crossing_reach(nets, inputs, &functions);
    Ok(Analysis {
        writers,
        functions,
        later,
        start_nets: globals(global::START),
        settle_nets: globals(global::SETTLE),
        param_nets,
        probes,
        samples,
        crossing,
    })
}

/// R4, static half: which reactions touch the analog side. A net reaches the analog side when the
/// analog side reads it, when a function that reads it writes a net that does, or when a reaction
/// that listens to or reads it touches the analog side; a reaction touches it when it reads a
/// continuous pin or has a statement without `LATER` on a net that reaches it (a `LATER` commit is
/// due at another instant, which is tested on its own). Least fixed point.
fn reachability(
    nets: &[NetSpec],
    inputs: &[ShapeInput<'_>],
    functions: &[FnNode],
    later: &[Vec<Vec<bool>>],
    samples: &[Vec<bool>],
) -> Vec<Vec<bool>> {
    let mut reach: Vec<bool> = nets.iter().map(|n| n.analog_reads).collect();
    let mut probes: Vec<Vec<bool>> = inputs.iter().map(|i| vec![false; i.shape.reactions.len()]).collect();
    let net_of = |input: &ShapeInput<'_>, pin: u32| input.pins.get(pin as usize).and_then(Binding::net);
    loop {
        let mut changed = false;
        for f in functions {
            if f.writes.iter().any(|w| reach[w.0 as usize]) {
                for r in &f.reads {
                    if !reach[r.0 as usize] {
                        reach[r.0 as usize] = true;
                        changed = true;
                    }
                }
            }
        }
        for (i, input) in inputs.iter().enumerate() {
            for (r, reaction) in input.shape.reactions.iter().enumerate() {
                let writes_reaching = reaction.writes.iter().enumerate().any(|(s, st)| {
                    !later[i][r][s] && net_of(input, st.pin).is_some_and(|n| reach[n.0 as usize])
                });
                if !(samples[i][r] || writes_reaching) {
                    continue;
                }
                probes[i][r] = true;
                let listened = reaction.on.iter().copied();
                let read = reaction.reads.iter().filter(|c| c.kind == cell::PIN).map(|c| c.index);
                for pin in listened.chain(read) {
                    if let Some(n) = net_of(input, pin)
                        && !reach[n.0 as usize]
                    {
                        reach[n.0 as usize] = true;
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            return probes;
        }
    }
}

/// R5: which reactions a `CROSS` watch can reach. Forward from the nets the watches fire: through a
/// function that reads a reached net to the nets it writes, through a reaction that listens to or
/// reads one to the nets its statements name. Least fixed point.
fn crossing_reach(nets: &[NetSpec], inputs: &[ShapeInput<'_>], functions: &[FnNode]) -> Vec<Vec<bool>> {
    let mut reached: Vec<bool> = nets.iter().map(|n| n.crossing).collect();
    let mut out: Vec<Vec<bool>> = inputs.iter().map(|i| vec![false; i.shape.reactions.len()]).collect();
    let net_of = |input: &ShapeInput<'_>, pin: u32| input.pins.get(pin as usize).and_then(Binding::net);
    loop {
        let mut changed = false;
        for f in functions {
            if f.reads.iter().any(|r| reached[r.0 as usize]) {
                for w in &f.writes {
                    if !reached[w.0 as usize] {
                        reached[w.0 as usize] = true;
                        changed = true;
                    }
                }
            }
        }
        for (i, input) in inputs.iter().enumerate() {
            for (r, reaction) in input.shape.reactions.iter().enumerate() {
                let listened = reaction.on.iter().copied();
                let read = reaction.reads.iter().filter(|c| c.kind == cell::PIN).map(|c| c.index);
                if !listened.chain(read).any(|p| net_of(input, p).is_some_and(|n| reached[n.0 as usize])) {
                    continue;
                }
                out[i][r] = true;
                for st in reaction.writes.iter() {
                    if let Some(n) = net_of(input, st.pin)
                        && !reached[n.0 as usize]
                    {
                        reached[n.0 as usize] = true;
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            return out;
        }
    }
}

/// The array net `net` an element or a window names.
fn array_net<'a>(nets: &'a [NetSpec], net: NetId, name: &str) -> Result<&'a NetSpec, DigitalError> {
    match nets.get(net.0 as usize) {
        Some(spec) if spec.count > 0 && spec.kind == NetKind::Discrete => Ok(spec),
        Some(spec) => Err(bind_error("P4", vec![format!("{name}: net `{}` is no array net", spec.name)])),
        None => Err(bind_error("net", vec![format!("{name}: net {} does not exist", net.0)])),
    }
}

/// Every pin is bound as its kind allows, to a net that exists and has its kind.
fn check_bindings(nets: &[NetSpec], inputs: &[ShapeInput<'_>]) -> Result<(), DigitalError> {
    for input in inputs {
        for (i, pin) in input.shape.pins.iter().enumerate() {
            let name = pin_name(input, i);
            match (input.pins[i], pin.kind) {
                (Binding::Absent, _) if pin.flags & pin_flag::OPTIONAL != 0 => {}
                (Binding::Absent, _) => {
                    let what = if pin.role == role::GLOBAL { "a global with no net and not OPTIONAL" } else { "no net" };
                    return Err(bind_error("unbound", vec![format!("{name}: {what}")]));
                }
                (Binding::Sample(_), kind::CONTINUOUS) => {}
                (Binding::Net(id), kind::DISCRETE | kind::EVENT) => {
                    let Some(spec) = nets.get(id.0 as usize) else {
                        return Err(bind_error("net", vec![format!("{name}: net {} does not exist", id.0)]));
                    };
                    let want = if pin.kind == kind::EVENT { NetKind::Event } else { NetKind::Discrete };
                    if spec.kind != want {
                        return Err(bind_error("P4", vec![format!("{name}: bound to net `{}` of another kind", spec.name)]));
                    }
                    // P4: an array pin and its net have equal `count` (0 on both for one carrier)
                    if pin.count != spec.count {
                        return Err(bind_error(
                            "P4",
                            vec![format!("{name}: an array of {} elements on net `{}` of {}", pin.count, spec.name, spec.count)],
                        ));
                    }
                }
                (Binding::ArrayElement { net, element }, kind::DISCRETE) => {
                    let spec = array_net(nets, net, &name)?;
                    if pin.count != 0 || element >= spec.count {
                        return Err(bind_error(
                            "P4",
                            vec![format!("{name}: element {element} of net `{}` of {} elements, on a pin of count {}", spec.name, spec.count, pin.count)],
                        ));
                    }
                }
                (Binding::ArrayWindow { net, first, length }, kind::DISCRETE) => {
                    let spec = array_net(nets, net, &name)?;
                    if pin.count != length || length == 0 || first.checked_add(length).is_none_or(|end| end > spec.count) {
                        return Err(bind_error(
                            "P4",
                            vec![format!(
                                "{name}: a window of {length} elements from {first} on net `{}` of {} elements, for a pin of count {}",
                                spec.name, spec.count, pin.count
                            )],
                        ));
                    }
                }
                (_, _) => return Err(bind_error("net", vec![format!("{name}: a binding its kind cannot take")])),
            }
        }
    }
    Ok(())
}

/// P4: the pins on one net have equal layouts, and the net holds what they say.
fn check_layouts(nets: &[NetSpec], inputs: &[ShapeInput<'_>]) -> Result<(), DigitalError> {
    let mut first: Vec<Option<(usize, usize)>> = vec![None; nets.len()];
    for (n, input) in inputs.iter().enumerate() {
        for (i, pin) in input.shape.pins.iter().enumerate() {
            let Some(id) = input.pins[i].net() else { continue };
            match first[id.0 as usize] {
                None => first[id.0 as usize] = Some((n, i)),
                Some((m, j)) => {
                    let other = inputs[m].shape.pins.get(j).expect("recorded from this shape");
                    if !layout_eq(&other.layout, &pin.layout) {
                        return Err(bind_error(
                            "P4",
                            vec![format!(
                                "unequal layouts on net `{}`: {} and {}",
                                nets[id.0 as usize].name,
                                pin_name(&inputs[m], j),
                                pin_name(input, i)
                            )],
                        ));
                    }
                }
            }
        }
    }
    for (n, input) in inputs.iter().enumerate() {
        for (i, pin) in input.shape.pins.iter().enumerate() {
            let Some(id) = input.pins[i].net() else { continue };
            let spec = &nets[id.0 as usize];
            if pin.layout.size != spec.size || pin.layout.align != spec.align {
                return Err(bind_error(
                    "P4",
                    vec![format!(
                        "{} has a layout of {} bytes aligned to {}; net `{}` holds {} aligned to {}",
                        pin_name(&inputs[n], i),
                        pin.layout.size,
                        pin.layout.align,
                        spec.name,
                        spec.size,
                        spec.align
                    )],
                ));
            }
        }
    }
    Ok(())
}

/// One writer of a net, with the elements it writes when the net is an array and the pin is a window of it.
struct WriterEntry {
    writer: NetWriter,
    who: String,
    /// `(first, length)` of the elements the writing pin covers; `None` is the whole net.
    elements: Option<(u64, u64)>,
}

impl WriterEntry {
    /// Whether the two writers can write one element: always for a scalar net or a whole pin, and for windows when
    /// their runs overlap.
    fn collides(&self, other: &WriterEntry) -> bool {
        match (self.elements, other.elements) {
            (Some((a, la)), Some((b, lb))) => a < b + lb && b < a + la,
            _ => true,
        }
    }
}

/// P1: one writer per net (per element of an array net), derived from the shapes; the host when none.
fn find_writers(nets: &[NetSpec], inputs: &[ShapeInput<'_>]) -> Result<Vec<NetWriter>, DigitalError> {
    let mut found: Vec<Vec<WriterEntry>> = (0..nets.len()).map(|_| Vec::new()).collect();
    let mut note = |binding: Binding, writer: NetWriter, who: String| {
        let Some(net) = binding.net() else { return };
        let list = &mut found[net.0 as usize];
        let elements = binding.elements();
        if !list.iter().any(|e| e.writer == writer && e.elements == elements) {
            list.push(WriterEntry { writer, who, elements });
        }
    };
    for (n, input) in inputs.iter().enumerate() {
        let instance = n as u32;
        for (k, f) in input.shape.functions.iter().enumerate() {
            for c in f.writes.iter().filter(|c| c.kind == cell::PIN) {
                if let Some(binding) = input.pins.get(c.index as usize) {
                    let who = format!("{} (function {})", pin_name(input, c.index as usize), f.name.as_str());
                    note(*binding, NetWriter::Function { instance, k: k as u32 }, who);
                }
            }
        }
        for r in input.shape.reactions.iter() {
            for s in r.writes.iter() {
                if let Some(binding) = input.pins.get(s.pin as usize) {
                    let who = format!("{} (statement of reaction {})", pin_name(input, s.pin as usize), r.name.as_str());
                    note(*binding, NetWriter::Statements { instance }, who);
                }
            }
        }
    }
    let mut writers = Vec::with_capacity(nets.len());
    for (i, list) in found.into_iter().enumerate() {
        for (a, first) in list.iter().enumerate() {
            let clash = list[a + 1..]
                .iter()
                .find(|second| first.writer != second.writer && (nets[i].count == 0 || first.collides(second)));
            if let Some(second) = clash {
                return Err(bind_error("P1", vec![format!("net `{}` has two writers: {} and {}", nets[i].name, first.who, second.who)]));
            }
        }
        writers.push(list.first().map_or(NetWriter::Host, |e| e.writer));
    }
    Ok(writers)
}

/// One instance of the plan: a live device and where its pins are.
#[derive(Debug)]
pub struct InstancePlan {
    pub path: String,
    pub handle: Handle,
    pub pins: Vec<Binding>,
}

/// A bound design: nets, instances and everything bind derived.
#[derive(Debug)]
pub struct DigitalPlan {
    pub nets: Vec<NetSpec>,
    pub instances: Vec<InstancePlan>,
    pub resolution: Resolution,
    pub(crate) analysis: Analysis,
    pub(crate) dag: Dag,
    cone: Vec<bool>,
}

impl DigitalPlan {
    /// Checks and binds `instances` over `nets`.
    pub fn bind(nets: Vec<NetSpec>, instances: Vec<InstancePlan>, resolution: Resolution) -> Result<DigitalPlan, DigitalError> {
        let analysis = {
            let inputs: Vec<ShapeInput<'_>> =
                instances.iter().map(|i| ShapeInput { path: &i.path, shape: i.handle.shape(), pins: &i.pins }).collect();
            analyze(&nets, &inputs, resolution)?
        };
        let dag = Dag::build(analysis.functions(), &nets)?;
        let cone = parameter_cone(&nets, &instances, &analysis, &dag);
        Ok(DigitalPlan { nets, instances, resolution, analysis, dag, cone })
    }

    /// The layout of the pins on `net` (P4 made them equal): the first pin's.
    pub fn net_layout(&self, net: NetId) -> Option<&Layout<'_>> {
        self.instances.iter().find_map(|inst| {
            inst.pins.iter().enumerate().find_map(|(i, b)| match b {
                Binding::Net(id) | Binding::ArrayElement { net: id, .. } | Binding::ArrayWindow { net: id, .. } if *id == net => {
                    inst.handle.shape().pins.get(i).map(|p| &p.layout)
                }
                _ => None,
            })
        })
    }

    /// P6: per function of the dag, whether it is in the parameter cone.
    pub fn cone(&self) -> &[bool] {
        &self.cone
    }

    /// Whether function `k` of `instance` is in the parameter cone.
    pub fn in_cone(&self, instance: u32, k: u32) -> bool {
        (0..self.dag.len()).any(|f| {
            let node = self.dag.function(f);
            node.instance == instance && node.k == k && self.cone[f]
        })
    }

    /// The order the discrete functions run in.
    pub fn dag(&self) -> &Dag {
        &self.dag
    }

    /// What the bind rules derived.
    pub fn analysis(&self) -> &Analysis {
        &self.analysis
    }
}

/// P6: the discrete functions all of whose inputs derive, through functions only, from nets the host
/// writes that carry a `PARAM` or `GLOBAL` pin and no `PORT` pin. A function fed by a reaction's
/// statement, or by a port, is outside it. Least fixed point over the dag's functions.
fn parameter_cone(nets: &[NetSpec], instances: &[InstancePlan], analysis: &Analysis, dag: &Dag) -> Vec<bool> {
    let mut derived = vec![false; nets.len()];
    let mut param = vec![false; nets.len()];
    let mut port = vec![false; nets.len()];
    for inst in instances {
        for (i, pin) in inst.handle.shape().pins.iter().enumerate() {
            if let Some(id) = inst.pins[i].net() {
                param[id.0 as usize] |= pin.role == role::PARAM || pin.role == role::GLOBAL;
                port[id.0 as usize] |= pin.role == role::PORT;
            }
        }
    }
    for (i, d) in derived.iter_mut().enumerate() {
        *d = param[i] && !port[i] && analysis.writer(NetId(i as u32)) == NetWriter::Host;
    }
    let mut cone = vec![false; dag.len()];
    loop {
        let mut changed = false;
        for (f, in_cone) in cone.iter_mut().enumerate() {
            let node = dag.function(f);
            if !*in_cone && node.reads.iter().all(|r| derived[r.0 as usize]) {
                *in_cone = true;
                changed = true;
                for w in &node.writes {
                    derived[w.0 as usize] = true;
                }
            }
        }
        if !changed {
            return cone;
        }
    }
}
