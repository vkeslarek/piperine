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

use super::dag::Dag;
use super::net::{NetId, NetKind, NetSpec};
use super::tick::Resolution;
use crate::error::DigitalError;

/// What one pin of an instance is bound to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Binding {
    /// A discrete or event pin on this net.
    Net(NetId),
    /// A continuous pin a reaction reads: an index into the owner's sample vector.
    Sample(u32),
    /// An unbound `OPTIONAL` pin: it reads as zero bytes.
    Absent,
}

/// An instance as the rules see it: its path, its shape, its pin bindings.
#[derive(Clone, Copy, Debug)]
pub struct ShapeInput<'a> {
    pub path: &'a str,
    pub shape: &'a Shape,
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
}

impl Analysis {
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
        if let Some(Binding::Net(id)) = input.pins.get(c.index as usize)
            && !out.contains(id)
        {
            out.push(*id);
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
                if let (role::GLOBAL, Binding::Net(id)) = (pin.role, input.pins[i])
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
            if let Binding::Net(id) = input.pins[i]
                && (pin.role == role::PARAM || pin.role == role::GLOBAL)
            {
                param_nets[id.0 as usize] = true;
            }
        }
    }
    Ok(Analysis {
        writers,
        functions,
        later,
        start_nets: globals(global::START),
        settle_nets: globals(global::SETTLE),
        param_nets,
    })
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
            let Binding::Net(id) = input.pins[i] else { continue };
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
            let Binding::Net(id) = input.pins[i] else { continue };
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

/// P1: one writer per net, derived from the shapes; the host when none.
fn find_writers(nets: &[NetSpec], inputs: &[ShapeInput<'_>]) -> Result<Vec<NetWriter>, DigitalError> {
    let mut found: Vec<Vec<(NetWriter, String)>> = vec![Vec::new(); nets.len()];
    let mut note = |net: NetId, writer: NetWriter, who: String| {
        let list = &mut found[net.0 as usize];
        if !list.iter().any(|(w, _)| *w == writer) {
            list.push((writer, who));
        }
    };
    for (n, input) in inputs.iter().enumerate() {
        let instance = n as u32;
        for (k, f) in input.shape.functions.iter().enumerate() {
            for c in f.writes.iter().filter(|c| c.kind == cell::PIN) {
                if let Some(Binding::Net(id)) = input.pins.get(c.index as usize) {
                    let who = format!("{} (function {})", pin_name(input, c.index as usize), f.name.as_str());
                    note(*id, NetWriter::Function { instance, k: k as u32 }, who);
                }
            }
        }
        for r in input.shape.reactions.iter() {
            for s in r.writes.iter() {
                if let Some(Binding::Net(id)) = input.pins.get(s.pin as usize) {
                    let who = format!("{} (statement of reaction {})", pin_name(input, s.pin as usize), r.name.as_str());
                    note(*id, NetWriter::Statements { instance }, who);
                }
            }
        }
    }
    let mut writers = Vec::with_capacity(nets.len());
    for (i, list) in found.into_iter().enumerate() {
        match list.as_slice() {
            [] => writers.push(NetWriter::Host),
            [(w, _)] => writers.push(*w),
            [(_, a), (_, b), ..] => {
                return Err(bind_error("P1", vec![format!("net `{}` has two writers: {a} and {b}", nets[i].name)]));
            }
        }
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
        Ok(DigitalPlan { nets, instances, resolution, analysis, dag })
    }

    /// The layout of the pins on `net` (P4 made them equal): the first pin's.
    pub fn net_layout(&self, net: NetId) -> Option<&Layout> {
        self.instances.iter().find_map(|inst| {
            inst.pins.iter().enumerate().find_map(|(i, b)| match b {
                Binding::Net(id) if *id == net => inst.handle.shape().pins.get(i).map(|p| &p.layout),
                _ => None,
            })
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
