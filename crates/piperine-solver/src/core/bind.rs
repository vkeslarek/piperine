//! Bind (abi.md §3.1, §9; design `bind`): turns a netlist of device instances
//! into the two plans. Each instance's `PORT` pins are bound by position, its
//! `PARAM` pins by name to a net seeded with the value given or the pin's
//! default, its `GLOBAL` pins by constant (or by name for `NAMED`) to one
//! design-wide net, and its `PRIVATE` pins to nets and unknowns the solver
//! allocates. The analog half is an [`AnalogPlan`] over an [`Assembly`], the
//! discrete half a [`DigitalPlan`] over a [`Digital`] engine.
//!
//! The solver creates each placement once; the analog and the digital side
//! hold shares of the one instance.

use std::collections::HashMap;

use piperine_abi::caller::Handle;
use piperine_abi::raw::{
    Cell, DEFAULT, Layout, Module, Pin, cell, global, kind, leaf, pin_flag, role, watch,
};
use piperine_analog_solver::device::{Ambient, AnalogBinding, AnalogInstance, AnalogPlan, Assembly};
use piperine_analog_solver::error::{Error, SolverDomain};
use piperine_analog_solver::netlist::{AnalogReference, AnalogVariable, BranchIdentifier, Netlist, NodeIdentifier};
use piperine_analog_solver::tolerances::Tolerances;
use piperine_digital_solver::landing::TimePointRule;
use piperine_digital_solver::engine::Digital;
use piperine_digital_solver::net::{NetId, NetSpec};
use piperine_digital_solver::plan::{Binding, DigitalPlan, InstancePlan};
use piperine_digital_solver::tick::Resolution;
use piperine_digital_solver::limits::DigitalLimits;

use crate::core::result::Result;
use crate::protocol::{Accepted, Observation, ObservationKind};

/// What a netlist hands to one `PORT` pin.
#[derive(Clone, Debug, PartialEq)]
pub enum Port {
    /// A continuous pin on this analog node.
    Node(AnalogReference),
    /// A discrete or event pin on the design-wide net of this name.
    Net(NetName),
    /// Nothing: the pin is private (an `OPTIONAL` pin reads as zero bytes).
    Open,
    /// A discrete pin on `length` consecutive elements of the array net `net` from `first`: a scalar pin takes one
    /// element (`length` 1), an array pin of `length` elements takes that run (composite-values CV-23).
    Window { net: NetName, first: u64, length: u64 },
}

/// An array net the builder declared: `count` elements, each starting at `init` (one element's bytes, every
/// element's bytes, or none for all zero).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ArrayDecl {
    pub name: String,
    pub count: u64,
    pub init: Vec<u8>,
}

/// The name of a design-wide discrete or event net.
pub type NetName = String;

/// A value given for a `PARAM` pin, encoded in the pin's layout at bind.
#[derive(Clone, Debug, PartialEq)]
pub enum ParamValue {
    /// The pin's first field, an `F64` (or `F32`).
    Real(f64),
    /// The pin's first field, an integer leaf.
    Int(i64),
    /// The pin's first field, a `BOOL` or `U8` leaf.
    Bool(bool),
    /// Exactly the pin's bytes.
    Bytes(Vec<u8>),
    /// The pin is bound to the shared param net of this name
    /// ([`CircuitBuilder::param_net`](crate::core::builder::CircuitBuilder::param_net)).
    Shared(String),
}

impl ParamValue {
    /// The bytes of the value in `layout` (P3: the bytes of its type and nothing else).
    pub(crate) fn encode(&self, layout: &Layout) -> std::result::Result<Vec<u8>, String> {
        let value = self;
        if let ParamValue::Bytes(b) = value {
            return if b.len() == layout.size as usize {
                Ok(b.clone())
            } else {
                Err(format!("{} bytes given for a layout of {}", b.len(), layout.size))
            };
        }
        let field = layout.fields.iter().next().ok_or("a layout with no field takes no value")?;
        let mut bytes = vec![0u8; layout.size as usize];
        let at = field.offset as usize;
        let put = |bytes: &mut [u8], b: &[u8]| -> std::result::Result<(), String> {
            bytes.get_mut(at..at + b.len()).ok_or("the layout is too small for the value")?.copy_from_slice(b);
            Ok(())
        };
        match (value, field.leaf) {
            (ParamValue::Real(v), leaf::F64) => put(&mut bytes, &v.to_le_bytes())?,
            (ParamValue::Real(v), leaf::F32) => put(&mut bytes, &(*v as f32).to_le_bytes())?,
            (ParamValue::Bool(v), leaf::BOOL | leaf::U8) => put(&mut bytes, &[u8::from(*v)])?,
            (ParamValue::Int(v), l) => {
                let le = |n: usize| -> std::result::Result<Vec<u8>, String> {
                    let full = v.to_le_bytes();
                    let fits = match n {
                        1 => i8::try_from(*v).is_ok() || u8::try_from(*v).is_ok(),
                        2 => i16::try_from(*v).is_ok() || u16::try_from(*v).is_ok(),
                        4 => i32::try_from(*v).is_ok() || u32::try_from(*v).is_ok(),
                        _ => true,
                    };
                    if fits { Ok(full[..n].to_vec()) } else { Err(format!("{v} does not fit {n} bytes")) }
                };
                let n = match l {
                    leaf::U8 | leaf::I8 => 1,
                    leaf::U16 | leaf::I16 => 2,
                    leaf::U32 | leaf::I32 => 4,
                    leaf::U64 | leaf::I64 => 8,
                    _ => return Err("an integer needs an integer leaf".into()),
                };
                put(&mut bytes, &le(n)?)?;
            }
            _ => return Err("the value does not fit the first field of the layout".into()),
        }
        // a layout whose second field is a BOOL `known` flag (a session parameter's) says the value was given
        if let Some(flag) = layout.fields.iter().nth(1).filter(|f| f.name.as_str() == "known" && f.leaf == leaf::BOOL) {
            if let Some(byte) = bytes.get_mut(flag.offset as usize) {
                *byte = 1;
            }
        }
        Ok(bytes)
    }
}

/// One instance as the builder collects it.
#[derive(Clone, Debug)]
pub(crate) struct DeviceSpec {
    pub module: &'static Module,
    pub path: String,
    pub values: Vec<(String, ParamValue)>,
    pub ports: Vec<Port>,
}

/// What the owner bound one pin to.
#[derive(Clone, Debug, PartialEq)]
pub enum Site {
    /// A continuous pin on this unknown.
    Unknown(AnalogReference),
    /// A continuous pin on the reference node.
    Ground,
    /// A discrete or event pin on this net.
    Net(NetId),
    /// A scalar discrete pin on one element of an array net.
    ArrayElement { net: NetId, element: u64 },
    /// An array pin on a run of elements of an array net.
    ArrayWindow { net: NetId, first: u64, length: u64 },
    /// Nothing (an unbound `OPTIONAL` pin).
    Absent,
}

/// Where each thing of the design is, by name.
#[derive(Clone, Debug, Default)]
pub struct NameMap {
    /// Instance paths, in the order the plans hold them.
    pub instances: Vec<String>,
    /// `[instance][pin]`.
    pub pins: Vec<Vec<Site>>,
    /// `[instance][pin]`: each pin's name in its module's shape.
    pub pin_names: Vec<Vec<String>>,
    /// Design-wide net by name (named ports, globals, shared params).
    pub nets: HashMap<String, NetId>,
    /// The net of `(instance path, PARAM pin name)`.
    pub params: HashMap<(String, String), NetId>,
    /// Discrete slot of the analog copy, by net.
    pub slots: HashMap<NetId, u32>,
    /// The net behind each analog slot.
    pub slot_nets: Vec<NetId>,
    /// The elements of its array net each analog slot holds, `(first, length)`; `None` for a slot that holds a whole net.
    pub slot_windows: Vec<Option<(u32, u32)>>,
    /// Discrete slot of the analog copy of the window `(first, length)` of an array net.
    pub window_slots: HashMap<(NetId, u32, u32), u32>,
}

impl NameMap {
    /// The net of an instance's event pin (a `CROSS` watch's), as bind placed it.
    pub(crate) fn event_net(&self, instance: usize, pin: u32) -> Result<u32> {
        match self.pins.get(instance).and_then(|p| p.get(pin as usize)) {
            Some(Site::Net(n)) => Ok(n.0),
            _ => Err(Error::simple(
                SolverDomain::Device,
                format!("instance `{}`: crossing event pin {pin} is bound to no net", self.instances[instance]),
            )),
        }
    }

    /// The slot of `net` (the whole net, or a window of an array net) and its bytes within the net's own, created on first use.
    fn slot_of(&mut self, net: NetId, window: Option<(u32, u32)>) -> u32 {
        let slot = match window {
            None => self.slots.get(&net).copied(),
            Some((first, length)) => self.window_slots.get(&(net, first, length)).copied(),
        };
        if let Some(slot) = slot {
            return slot;
        }
        let slot = self.slot_nets.len() as u32;
        self.slot_nets.push(net);
        self.slot_windows.push(window);
        match window {
            None => self.slots.insert(net, slot),
            Some((first, length)) => self.window_slots.insert((net, first, length), slot),
        };
        slot
    }

    /// The bytes slot `slot` holds of `net_bytes`, the bytes of its whole net (`size` bytes an element) as the seed gives them. `net` names
    /// the net in the error of a window that falls outside them.
    pub fn slot_bytes(&self, slot: u32, net: &str, net_bytes: &[u8], size: u32) -> Result<Vec<u8>> {
        Self::window_bytes(net, self.slot_windows[slot as usize], net_bytes, size)
    }

    /// The bytes of `window` of `net_bytes`, the bytes of a whole net `net` of `size` bytes an element: all of them for no window. An
    /// array net's seed of one element's bytes seeds every element; no seed gives none. A window outside the bytes is an internal error naming the net and
    /// the window: the lengths of the seed, the element size and the window's own disagree.
    pub fn window_bytes(net: &str, window: Option<(u32, u32)>, net_bytes: &[u8], size: u32) -> Result<Vec<u8>> {
        let Some((first, length)) = window else { return Ok(net_bytes.to_vec()) };
        // a net with no seed has none to cut: its slots start with no bytes, and the net's own initial value is `Default`
        if net_bytes.is_empty() {
            return Ok(Vec::new());
        }
        let size = size as usize;
        if net_bytes.len() == size {
            return Ok(net_bytes.repeat(length as usize));
        }
        net_bytes.get(first as usize * size..(first + length) as usize * size).map(<[u8]>::to_vec).ok_or_else(|| {
            Error::simple(
                SolverDomain::Digital,
                format!("internal error: the window of elements {first}..{} of net `{net}` lies outside its {} bytes ({size} bytes an element)", first + length, net_bytes.len()),
            )
        })
    }

    /// The windows of the array net `net` of `count` elements that the analog copy holds, as `(slot, first, length)` in the order of the
    /// slots: a slot that holds the whole net holds all its elements. The engine hands the analog side these windows' bytes one after
    /// the other (`NetSpec::analog_windows`), so a slot's bytes are a range of them.
    pub fn array_slots(&self, net: NetId, count: u64) -> Vec<(u32, u64, u64)> {
        self.slot_nets
            .iter()
            .enumerate()
            .filter(|(_, slot_net)| **slot_net == net)
            .map(|(slot, _)| match self.slot_windows[slot] {
                Some((first, length)) => (slot as u32, u64::from(first), u64::from(length)),
                None => (slot as u32, 0, count),
            })
            .collect()
    }
}

/// How many unknowns a run error names before it says how many more.
const NAMED_UNKNOWNS: usize = 8;

impl NameMap {
    /// The unknown `unknown` by what the author wrote: a node by every pin
    /// bound to it (an instance once, its pins joined by `+`), a flow by the
    /// instance and the private pin that carries it. `None` when no pin is
    /// bound to that unknown.
    pub fn describe(&self, unknown: usize) -> Option<String> {
        let mut nodes: Vec<(&str, Vec<&str>)> = Vec::new();
        for (instance, sites) in self.pins.iter().enumerate() {
            for (pin, site) in sites.iter().enumerate() {
                let Site::Unknown(reference) = site else { continue };
                if reference.idx() != Some(unknown) {
                    continue;
                }
                let path = self.instances[instance].as_str();
                let pin_name = self.pin_names[instance][pin].as_str();
                if let AnalogVariable::Branch(_) = reference.variable().as_ref() {
                    return Some(format!("flow of {path} ({pin_name})"));
                }
                match nodes.last_mut() {
                    Some((last, pins)) if *last == path => pins.push(pin_name),
                    _ => nodes.push((path, vec![pin_name])),
                }
            }
        }
        if nodes.is_empty() {
            return None;
        }
        let pins: Vec<String> = nodes.iter().map(|(path, pins)| format!("{path}.{}", pins.join("+"))).collect();
        Some(format!("node of {}", pins.join(", ")))
    }
}

/// A continuous pin with `init`: Newton's start, held through a transient's
/// operating point.
#[derive(Clone, Debug, PartialEq)]
pub struct Hold {
    pub reference: AnalogReference,
    pub value: f64,
}

/// A circuit bound from device instances.
#[derive(Debug)]
pub struct Bound {
    pub analog: Assembly,
    pub digital: Digital,
    pub names: NameMap,
    /// The continuous pins with `init`.
    pub holds: Vec<Hold>,
    /// The smallest non-zero `abstol` of the pins on each node.
    pub vntol: HashMap<NodeIdentifier, f64>,
    /// The flow `abstol` of the pins of each branch unknown that has one.
    pub branch_abstol: HashMap<BranchIdentifier, f64>,
    /// The analysis-kind globals some pin reads (`DC`, `TRAN`, `AC`, `NOISE`):
    /// the constant, its net and the bytes that mean "this analysis runs".
    pub analyses: Vec<(u32, NetId, Vec<u8>)>,
    /// The host's writes to param nets between analyses: committed at the
    /// start of every analysis after the `init`s (abi.md §10 "the host's
    /// initial writes").
    pub host_writes: Vec<(NetId, Vec<u8>)>,
    /// The run's temperature and whether the run set it, each when it is not
    /// what its net was seeded with: committed at the start of every analysis
    /// (the `TEMPERATURE` and `TEMPERATURE_GIVEN` globals hold the session's
    /// values, abi.md §9).
    pub temperature: Vec<(NetId, Vec<u8>)>,
    /// The bound on the microsteps of one instant the digital engine was built with.
    pub microsteps: usize,
}

/// What bind ran on, kept so that an instance can be created and bound again
/// (abi.md P5, P6: a `STRUCTURAL` write or a `RESHAPE` between analyses).
#[derive(Clone, Debug)]
pub(crate) struct Recipe {
    pub specs: Vec<DeviceSpec>,
    pub arrays: Vec<ArrayDecl>,
    pub shared: Vec<(String, ParamValue)>,
    pub session: Vec<(String, ParamValue)>,
    pub resolution: f64,
    pub microsteps: usize,
    pub time_point_rule: TimePointRule,
    pub temperature: f64,
}

impl Bound {
    /// An accepted solution of the assembly, as the discrete side reads it.
    pub(crate) fn accepted(&mut self, x: &[f64], amb: Ambient) -> Result<(Accepted, f64)> {
        let acc = self.analog.accept(x, amb)?;
        let fired = acc.fired.iter().map(|(i, p)| self.names.event_net(*i, *p)).collect::<Result<Vec<u32>>>()?;
        Ok((Accepted { fired, samples: self.analog.samples(x) }, acc.cap))
    }

    /// The trace records of the assembly, as the sink reads them.
    pub(crate) fn drain_traces(&mut self, time: f64, out: &mut Vec<Observation>) {
        for t in self.analog.drain_traces() {
            out.push(Observation {
                path: format!("{}.{}", t.instance, t.watch),
                analysis: Default::default(),
                time,
                kind: ObservationKind::Trace { iteration: t.iteration as u32, bytes: t.bytes },
            });
        }
    }

    /// The param net of `(path, param)` and the bytes of `value` in its layout.
    pub(crate) fn param_bytes(&self, path: &str, param: &str, value: &ParamValue) -> Result<(NetId, Vec<u8>)> {
        let net = *self.names.params.get(&(path.to_string(), param.to_string())).ok_or_else(|| {
            let mut declared: Vec<&str> =
                self.names.params.keys().filter(|(p, _)| p == path).map(|(_, n)| n.as_str()).collect();
            declared.sort_unstable();
            if declared.is_empty() {
                Bind::error(path, format!("no PARAM pin `{param}`; the instance declares no writable parameters"))
            } else {
                Bind::error(path, format!("no PARAM pin `{param}`; available parameters: {}", declared.join(", ")))
            }
        })?;
        let layout = self.digital.plan().net_layout(net).ok_or_else(|| Bind::error(path, format!("`{param}` has no layout")))?;
        let bytes = value.encode(layout).map_err(|e| Bind::error(path, format!("`{param}`: {e}")))?;
        Ok((net, bytes))
    }

    /// The id of the discrete net `name` of the digital plan, and `value` in its layout.
    pub(crate) fn net_bytes(&self, name: &str, value: &ParamValue) -> Result<(NetId, Vec<u8>)> {
        let fail = |why: String| Error::simple(SolverDomain::Device, format!("net `{name}`: {why}"));
        let at = self.digital.plan().nets.iter().position(|n| n.name == name).ok_or_else(|| fail("no discrete net of the design has that name".into()))?;
        let net = NetId(at as u32);
        let layout = self.digital.plan().net_layout(net).ok_or_else(|| fail("no device pin reads it, so it has no layout".into()))?;
        let mut bytes = value.encode(layout).map_err(fail)?;
        // a write to a drive net of a resolved net is a drive that is present (abi.md §9 D1): its `driving` byte is 1
        if let Some(driving) = layout.fields.iter().find(|field| field.name.as_bytes() == b"driving")
            && let Some(byte) = bytes.get_mut(driving.offset as usize)
            && !matches!(value, ParamValue::Bytes(_))
        {
            *byte = 1;
        }
        Ok((net, bytes))
    }
}

impl Bound {
    /// Names the unknowns of a run error by their pins and branches (§17.1):
    /// every owner entry passes its error through here before returning it.
    pub fn name(&self, error: Error) -> Error {
        let Error::Run(run) = &error else { return error };
        if run.unknowns.is_empty() || !run.names.is_empty() {
            return error;
        }
        let mut names: Vec<String> = run
            .unknowns
            .iter()
            .take(NAMED_UNKNOWNS)
            .map(|&u| self.names.describe(u).unwrap_or_else(|| format!("unknown #{u}")))
            .collect();
        if run.unknowns.len() > NAMED_UNKNOWNS {
            names.push(format!("and {} more", run.unknowns.len() - NAMED_UNKNOWNS));
        }
        error.with_names(names)
    }

    /// The `TEMPERATURE` global holds `t` from the next analysis on, and the
    /// `TEMPERATURE_GIVEN` global says whether the run set it (`given`); a net
    /// seeded with its value and never moved needs no write.
    pub(crate) fn set_temperature(&mut self, t: f64, given: bool) {
        let temperature = |layout: &Layout| ParamValue::Real(t).encode(layout);
        self.write_temperature_global("global:temperature", temperature);
        self.write_temperature_global("global:temperature_given", |layout| Self::flag_bytes(layout, given));
    }

    /// The bytes of a one-bit flag in `layout`: a `bool` when it takes one, `1.0` or `0.0` otherwise.
    pub(crate) fn flag_bytes(layout: &Layout, on: bool) -> std::result::Result<Vec<u8>, String> {
        ParamValue::Bool(on).encode(layout).or_else(|_| ParamValue::Real(if on { 1.0 } else { 0.0 }).encode(layout))
    }

    /// Writes the global net named `key` with the bytes `encoded` gives for its layout, and records the write
    /// in `temperature` when the net is not at its seed or an earlier write is already recorded.
    fn write_temperature_global(&mut self, key: &str, encoded: impl FnOnce(&Layout) -> std::result::Result<Vec<u8>, String>) {
        let Some(net) = self.names.nets.get(key).copied() else { return };
        let Some(layout) = self.digital.plan().net_layout(net) else { return };
        let Ok(bytes) = encoded(layout) else { return };
        let seeded = self.digital.plan().nets[net.0 as usize].init == bytes;
        if let Some(slot) = self.names.slots.get(&net) {
            self.analog.set_discrete(*slot, &bytes);
        }
        let earlier = self.temperature.iter().position(|(n, _)| *n == net);
        if let Some(at) = earlier {
            self.temperature.remove(at);
        }
        if !seeded || earlier.is_some() {
            self.temperature.push((net, bytes));
        }
    }

    /// Tightens `tol` with the per-node tolerances of the pins: a node keeps
    /// the smaller of its present override and the pins' (design AC 4). A branch
    /// unknown takes the flow `abstol` of its pins (§17.1 rules 1 and 2).
    pub fn tighten(&self, tol: &mut Tolerances) {
        if !self.vntol.is_empty() {
            let mut map = (*tol.node_vntol_overrides).clone();
            for (id, v) in &self.vntol {
                let entry = map.entry(id.clone()).or_insert(*v);
                *entry = entry.min(*v);
            }
            tol.node_vntol_overrides = std::rc::Rc::new(map);
        }
        if !self.branch_abstol.is_empty() {
            let mut map = (*tol.branch_abstol_overrides).clone();
            map.extend(self.branch_abstol.iter().map(|(id, v)| (id.clone(), *v)));
            tol.branch_abstol_overrides = std::rc::Rc::new(map);
        }
    }
}


/// What the builder hands to [`Bind::run`].
pub(crate) struct BindInput<'a> {
    pub specs: Vec<DeviceSpec>,
    /// The array nets: name, element count and initial value.
    pub arrays: Vec<ArrayDecl>,
    /// Shared param nets: name and seed.
    pub shared: Vec<(String, ParamValue)>,
    /// Session parameters for `NAMED` globals.
    pub session: Vec<(String, ParamValue)>,
    pub netlist: &'a mut Netlist,
    pub resolution: f64,
    pub microsteps: usize,
    pub time_point_rule: TimePointRule,
    pub temperature: f64,
}

/// The design-wide nets, and who created each.
#[derive(Default)]
struct Nets {
    specs: Vec<NetSpec>,
    named: HashMap<String, usize>,
}

impl Nets {
    /// The net `key`, created with the shape of `pin` on first use.
    fn get_or_add(&mut self, key: &str, name: &str, pin: &Pin) -> usize {
        if let Some(i) = self.named.get(key) {
            return *i;
        }
        let i = self.add(name, pin);
        self.named.insert(key.to_string(), i);
        i
    }

    /// The array net `decl`, created with the element shape of `pin` on first use.
    fn get_or_add_array(&mut self, key: &str, decl: &ArrayDecl, pin: &Pin) -> usize {
        if let Some(i) = self.named.get(key) {
            return *i;
        }
        let spec = NetSpec { init: decl.init.clone(), ..NetSpec::array(&decl.name, pin.layout.size, pin.layout.align, decl.count) };
        self.specs.push(spec);
        let i = self.specs.len() - 1;
        self.named.insert(key.to_string(), i);
        i
    }

    /// A net of its own with the shape of `pin`.
    fn add(&mut self, name: &str, pin: &Pin) -> usize {
        let spec = if pin.kind == kind::EVENT {
            NetSpec::event(name)
        } else if pin.count > 0 {
            // an array pin no one shares: its own array net of the same element count (P4)
            NetSpec::array(name, pin.layout.size, pin.layout.align, pin.count)
        } else {
            NetSpec::discrete(name, pin.layout.size, pin.layout.align)
        };
        self.specs.push(spec);
        self.specs.len() - 1
    }

    /// Folds what `pin` says of its net (P7 flags, `STRUCTURAL`, `init`) in.
    fn merge(&mut self, net: usize, pin: &Pin) {
        let spec = &mut self.specs[net];
        spec.quiet |= pin.flags & pin_flag::QUIET != 0;
        spec.record |= pin.flags & pin_flag::RECORD != 0;
        spec.relax |= pin.flags & pin_flag::RELAX != 0;
        spec.structural |= pin.flags & pin_flag::STRUCTURAL != 0;
        // a scalar pin on one element of an array net says nothing of the other elements' initial value
        let element_pin = spec.count > 0 && pin.count == 0;
        if spec.init.is_empty() && pin.flags & pin_flag::HAS_INIT != 0 && pin.role != role::PARAM && !element_pin {
            spec.init = pin.init.as_slice().to_vec();
        }
    }

    /// Checks every net's seed against its size: one element's bytes, or (an array net) every element's.
    fn check_seeds(&self) -> Result<()> {
        for spec in &self.specs {
            let element = spec.size as usize;
            let allowed = spec.init.len() == element || (spec.count > 0 && spec.init.len() as u64 == element as u64 * spec.count);
            if !spec.init.is_empty() && !allowed {
                return Err(Error::simple(
                    SolverDomain::Device,
                    format!("net `{}` holds {} bytes and its seed {}", spec.name, spec.size, spec.init.len()),
                ));
            }
        }
        Ok(())
    }
}

/// One instance once its pins are bound: its two shares of the one placement, the site of each pin, and the net
/// of each discrete or event pin.
struct InstanceBinding {
    path: String,
    analog: Handle,
    digital: Handle,
    sites: Vec<Site>,
    /// The net index of each discrete or event pin.
    nets: Vec<Option<usize>>,
}

impl InstanceBinding {
    /// The pins of the instance's shape an analog function reads, a term's or watch's pin operand names: the
    /// discrete pins the analog copy holds.
    fn analog_pins(&self) -> Vec<bool> {
        let shape = self.analog.shape();
        let mut read = vec![false; shape.pins.len()];
        let mut mark = |p: u32| {
            if let Some(r) = read.get_mut(p as usize) {
                *r = true;
            }
        };
        for f in shape.functions.iter() {
            let analog = !f.writes.is_empty() && f.writes.iter().all(|c: &Cell| c.kind != cell::PIN);
            if analog {
                f.reads.iter().filter(|c| c.kind == cell::PIN).for_each(|c| mark(c.index));
            }
        }
        for t in shape.terms.iter() {
            for p in [t.max, t.abstol] {
                if p != DEFAULT {
                    mark(p);
                }
            }
        }
        for w in shape.watches.iter() {
            for p in [w.ttol, w.etol] {
                if p != DEFAULT {
                    mark(p);
                }
            }
        }
        read
    }

    /// The analog and the digital binding of the pin `pin` at `site`: `read` says the analog copy holds it, `sample`
    /// is the sample a continuous pin is read through, and `names` takes the analog slot of a discrete pin.
    fn site_bindings(site: &Site, pin: &Pin, read: bool, sample: u32, names: &mut NameMap) -> (AnalogBinding, Binding) {
        let held = pin.kind == kind::DISCRETE && read;
        match site {
            Site::Unknown(r) => (AnalogBinding::Unknown(r.clone()), Binding::Sample(sample)),
            Site::Ground => (AnalogBinding::Ground, Binding::Sample(sample)),
            Site::Net(id) if held => (AnalogBinding::Discrete(names.slot_of(*id, None)), Binding::Net(*id)),
            Site::Net(id) => (AnalogBinding::Absent, Binding::Net(*id)),
            Site::ArrayElement { net, element } => {
                let analog = if held { AnalogBinding::Discrete(names.slot_of(*net, Some((*element as u32, 1)))) } else { AnalogBinding::Absent };
                (analog, Binding::ArrayElement { net: *net, element: *element })
            }
            Site::ArrayWindow { net, first, length } => {
                let analog = if held { AnalogBinding::Discrete(names.slot_of(*net, Some((*first as u32, *length as u32)))) } else { AnalogBinding::Absent };
                (analog, Binding::ArrayWindow { net: *net, first: *first, length: *length })
            }
            Site::Absent => (AnalogBinding::Absent, Binding::Absent),
        }
    }

    /// The instance's analog and digital halves, the `index`-th of the design: `read` is its analog pins, `sample_of`
    /// the sample of each continuous pin a reaction reads; `names` records its path, pins and slots.
    fn into_plans(self, index: usize, read: &[bool], sample_of: &HashMap<(usize, usize), u32>, names: &mut NameMap) -> (AnalogInstance, InstancePlan) {
        let shape = self.analog.shape();
        let mut analog_bindings = Vec::with_capacity(self.sites.len());
        let mut digital_bindings = Vec::with_capacity(self.sites.len());
        for (pi, site) in self.sites.iter().enumerate() {
            let pin = shape.pins.get(pi).expect("one site per pin");
            let sample = sample_of.get(&(index, pi)).copied().unwrap_or(u32::MAX);
            let (analog, digital) = Self::site_bindings(site, pin, read[pi], sample, names);
            analog_bindings.push(analog);
            digital_bindings.push(digital);
        }
        names.instances.push(self.path.clone());
        names.pin_names.push(shape.pins.iter().map(|p| p.name.as_str().to_string()).collect());
        names.pins.push(self.sites);
        let analog = AnalogInstance { path: self.path.clone(), handle: self.analog, pins: analog_bindings };
        (analog, InstancePlan { path: self.path, handle: self.digital, pins: digital_bindings })
    }
}

/// What one instance hands its pins while they are bound: its path, its `PORT` connections in order (and how many
/// a pin took), and the bytes and shared nets of the `PARAM` values given, by prototype pin.
struct InstanceInput<'s> {
    path: &'s str,
    ports: std::slice::Iter<'s, Port>,
    /// How many of `ports` a `PORT` pin took.
    taken: usize,
    /// The bytes of each `PARAM` value given.
    seeds: HashMap<u32, Vec<u8>>,
    /// The shared param net each `PARAM` pin given one is bound to.
    shared_pins: HashMap<u32, String>,
}

impl InstanceInput<'_> {
    /// The next `PORT` connection, or `Port::Open` when the netlist gave no more.
    fn next_port(&mut self) -> Port {
        match self.ports.next() {
            Some(p) => {
                self.taken += 1;
                p.clone()
            }
            None => Port::Open,
        }
    }
}

/// Bind in progress (abi.md §3.1, §9): what the builder declared design-wide, and what binding the instances one
/// by one builds — the nets, the continuous pins' holds and tolerances, the param nets, the analysis flags. Each
/// pin is bound by its kind and its role (`PORT`, `PARAM`, `GLOBAL`, `PRIVATE`).
pub(crate) struct Bind<'a> {
    arrays: Vec<ArrayDecl>,
    /// The shared param nets by name, with their seeds.
    shared: HashMap<String, ParamValue>,
    /// The session parameters by name, for `NAMED` globals.
    session: HashMap<String, ParamValue>,
    netlist: &'a mut Netlist,
    resolution: f64,
    temperature: f64,
    nets: Nets,
    holds: Vec<Hold>,
    vntol: HashMap<NodeIdentifier, f64>,
    branch_abstol: HashMap<BranchIdentifier, f64>,
    /// The net of `(instance path, PARAM pin name)`.
    params: HashMap<(String, String), NetId>,
    analyses: Vec<(u32, NetId, Vec<u8>)>,
}

impl Bind<'_> {
    /// The error of the instance at `path`.
    fn error(path: &str, detail: impl std::fmt::Display) -> Error {
        Error::simple(SolverDomain::Device, format!("instance `{path}`: {detail}"))
    }

    /// Binds `input` to its two plans.
    pub(crate) fn run(input: BindInput<'_>) -> Result<Bound> {
        let BindInput { specs, arrays, shared, session, netlist, resolution, microsteps, time_point_rule, temperature } = input;
        let mut shared_by_name: HashMap<String, ParamValue> = HashMap::new();
        for (name, seed) in shared {
            if shared_by_name.insert(name.clone(), seed).is_some() {
                return Err(Error::simple(SolverDomain::Device, format!("shared param net `{name}` is declared twice")));
            }
        }
        let mut bind = Bind {
            arrays,
            shared: shared_by_name,
            session: session.into_iter().collect(),
            netlist,
            resolution,
            temperature,
            nets: Nets::default(),
            holds: Vec::new(),
            vntol: HashMap::new(),
            branch_abstol: HashMap::new(),
            params: HashMap::new(),
            analyses: Vec::new(),
        };
        let mut paths = std::collections::HashSet::new();
        let mut instances: Vec<InstanceBinding> = Vec::with_capacity(specs.len());
        for spec in &specs {
            if !paths.insert(spec.path.clone()) {
                return Err(Bind::error(&spec.path, "an instance of this path exists"));
            }
            instances.push(bind.instance(spec)?);
        }
        bind.finish(instances, microsteps, time_point_rule)
    }

    /// The values `spec` gives, by prototype pin (P5: `Value.pin` means the same pin in every shape), in the order
    /// given: their bytes, recorded with the shared nets they name in `input`.
    fn given_values(&self, spec: &DeviceSpec, input: &mut InstanceInput<'_>) -> Result<Vec<(u32, Vec<u8>)>> {
        let path = input.path;
        let proto = spec.module.prototype();
        let mut values: Vec<(u32, Vec<u8>)> = Vec::new();
        for (name, value) in &spec.values {
            let Some((pi, pin)) = proto.pins.iter().enumerate().find(|(_, p)| p.role == role::PARAM && p.name.as_str() == name) else {
                return Err(Bind::error(path, format!("no PARAM pin `{name}`")));
            };
            let resolved = match value {
                ParamValue::Shared(net) => {
                    let seed = self.shared.get(net).ok_or_else(|| Bind::error(path, format!("`{name}`: no shared param net `{net}`")))?;
                    input.shared_pins.insert(pi as u32, net.clone());
                    seed.clone()
                }
                v => v.clone(),
            };
            let bytes = resolved.encode(&pin.layout).map_err(|e| Bind::error(path, format!("`{name}`: {e}")))?;
            input.seeds.insert(pi as u32, bytes.clone());
            values.push((pi as u32, bytes));
        }
        Ok(values)
    }

    /// Creates the instance `spec` places and binds each of its pins.
    fn instance(&mut self, spec: &DeviceSpec) -> Result<InstanceBinding> {
        let path = spec.path.as_str();
        let mut input = InstanceInput { path, ports: spec.ports.iter(), taken: 0, seeds: HashMap::new(), shared_pins: HashMap::new() };
        let values = self.given_values(spec, &mut input)?;
        let given: Vec<(u32, &[u8])> = values.iter().map(|(p, b)| (*p, b.as_slice())).collect();
        let analog = Handle::create(spec.module, &given).map_err(|e| Bind::error(path, format!("create: {e}")))?;
        // One `create` per placement: both sides call the same instance (an external
        // device holds state; two copies would diverge). The assembly's evals and the
        // engine's reactions never run concurrently in this owner, and the handle's
        // lock serializes them anyway.
        let digital = analog.share();
        let shape = analog.shape();
        let mut sites = Vec::with_capacity(shape.pins.len());
        let mut pin_nets = Vec::with_capacity(shape.pins.len());
        for (pi, pin) in shape.pins.iter().enumerate() {
            let (site, net) = match pin.kind {
                kind::CONTINUOUS => (self.continuous_pin(&mut input, pin)?, None),
                kind::DISCRETE | kind::EVENT => self.discrete_pin(&mut input, pi as u32, pin)?,
                _ => return Err(Bind::error(path, format!("pin `{}` has an unknown kind", pin.name.as_str()))),
            };
            sites.push(site);
            pin_nets.push(net);
        }
        if input.taken < spec.ports.len() {
            return Err(Bind::error(path, format!("{} ports given for {} PORT pins", spec.ports.len(), input.taken)));
        }
        Ok(InstanceBinding { path: spec.path.clone(), analog, digital, sites, nets: pin_nets })
    }

    /// A continuous pin: a `PORT` on the node it is given (its own branch when open), a `PRIVATE` on a branch of its
    /// own; its `init` is a hold and its `abstol` tightens its unknown's tolerance.
    fn continuous_pin(&mut self, input: &mut InstanceInput<'_>, pin: &Pin) -> Result<Site> {
        let (path, pname) = (input.path, pin.name.as_str());
        let site = match pin.role {
            role::PORT => match input.next_port() {
                Port::Node(r) if r.variable().is_ground() => Site::Ground,
                Port::Node(r) => Site::Unknown(r),
                Port::Open => Site::Unknown(self.netlist.connect_branch(BranchIdentifier::new(path, pname))),
                Port::Net(n) | Port::Window { net: n, .. } => {
                    return Err(Bind::error(path, format!("continuous pin `{pname}` bound to net `{n}`")));
                }
            },
            role::PRIVATE => Site::Unknown(self.netlist.connect_branch(BranchIdentifier::new(path, pname))),
            _ => return Err(Bind::error(path, format!("continuous pin `{pname}` is neither PORT nor PRIVATE"))),
        };
        if let Site::Unknown(r) = &site {
            self.hold_and_tolerance(path, pin, r)?;
        }
        Ok(site)
    }

    /// Records the `init` of the continuous pin `pin` on unknown `r` as a hold, and its node or flow `abstol`.
    fn hold_and_tolerance(&mut self, path: &str, pin: &Pin, r: &AnalogReference) -> Result<()> {
        if pin.flags & pin_flag::HAS_INIT != 0 {
            let b = pin.init.as_slice();
            let v = b.get(..8).and_then(|b| b.try_into().ok()).map(f64::from_le_bytes);
            self.holds.push(Hold {
                reference: r.clone(),
                value: v.ok_or_else(|| Bind::error(path, format!("`{}`: init is not an F64", pin.name.as_str())))?,
            });
        }
        match r.variable().as_ref() {
            AnalogVariable::Node(id) if pin.abstol[0] > 0.0 => {
                let e = self.vntol.entry(id.clone()).or_insert(pin.abstol[0]);
                *e = e.min(pin.abstol[0]);
            }
            AnalogVariable::Branch(id) if pin.abstol[1] > 0.0 => {
                let e = self.branch_abstol.entry(id.clone()).or_insert(pin.abstol[1]);
                *e = e.min(pin.abstol[1]);
            }
            _ => {}
        }
        Ok(())
    }

    /// A discrete or event pin, by role: its site and its net (none for an absent pin).
    fn discrete_pin(&mut self, input: &mut InstanceInput<'_>, pi: u32, pin: &Pin) -> Result<(Site, Option<usize>)> {
        let (net, window) = match pin.role {
            role::PORT => self.discrete_port(input, pin)?,
            role::PARAM => (Some(self.parameter_pin(input, pi, pin)), None),
            role::GLOBAL => (self.global_pin(input.path, pin)?, None),
            _ => (Some(self.nets.add(&format!("{}.{}", input.path, pin.name.as_str()), pin)), None),
        };
        if let Some(n) = net {
            self.nets.merge(n, pin);
        }
        let site = match (net, window) {
            (None, _) => Site::Absent,
            (Some(n), None) => Site::Net(NetId(n as u32)),
            (Some(n), Some((element, 1))) if pin.count == 0 => Site::ArrayElement { net: NetId(n as u32), element },
            (Some(n), Some((first, length))) => Site::ArrayWindow { net: NetId(n as u32), first, length },
        };
        Ok((site, net))
    }

    /// A discrete `PORT` pin: the named net (or array net) it is given, a window of an array net, a net of its own
    /// when open, nothing when open and `OPTIONAL`. The window is `(first, length)` for a window.
    fn discrete_port(&mut self, input: &mut InstanceInput<'_>, pin: &Pin) -> Result<(Option<usize>, Option<(u64, u64)>)> {
        let (path, pname) = (input.path, pin.name.as_str());
        match input.next_port() {
            Port::Net(n) => match self.arrays.iter().find(|decl| decl.name == n) {
                Some(decl) if pin.count == decl.count => Ok((Some(self.nets.get_or_add_array(&format!("net:{n}"), decl, pin)), None)),
                Some(decl) => Err(Bind::error(path, format!("pin `{pname}` of {} elements bound to array net `{n}` of {} elements", pin.count, decl.count))),
                None => Ok((Some(self.nets.get_or_add(&format!("net:{n}"), &n, pin)), None)),
            },
            Port::Window { net: n, first, length } => {
                let Some(decl) = self.arrays.iter().find(|decl| decl.name == n) else {
                    return Err(Bind::error(path, format!("pin `{pname}` bound to a window of `{n}`, which is no array net")));
                };
                if first.checked_add(length).is_none_or(|end| end > decl.count) || length == 0 {
                    return Err(Bind::error(path, format!("pin `{pname}` bound to elements {first}..+{length} of array net `{n}` of {} elements", decl.count)));
                }
                if length != pin.count.max(1) {
                    return Err(Bind::error(path, format!("pin `{pname}` of {} elements bound to a window of {length} elements of `{n}`", pin.count.max(1))));
                }
                Ok((Some(self.nets.get_or_add_array(&format!("net:{n}"), decl, pin)), Some((first, length))))
            }
            Port::Open if pin.flags & pin_flag::OPTIONAL != 0 => Ok((None, None)),
            Port::Open => Ok((Some(self.nets.add(&format!("{path}.{pname}"), pin)), None)),
            Port::Node(_) => Err(Bind::error(path, format!("discrete pin `{pname}` bound to an analog node"))),
        }
    }

    /// A `PARAM` pin: the shared param net it is given or a net of its own, seeded with the value given or the pin's
    /// `init`.
    fn parameter_pin(&mut self, input: &InstanceInput<'_>, pi: u32, pin: &Pin) -> usize {
        let (path, pname) = (input.path, pin.name.as_str());
        let seed = input.seeds.get(&pi).cloned().or_else(|| (pin.flags & pin_flag::HAS_INIT != 0).then(|| pin.init.as_slice().to_vec()));
        let n = match input.shared_pins.get(&pi) {
            Some(name) => self.nets.get_or_add(&format!("param:{name}"), name, pin),
            None => self.nets.add(&format!("{path}.{pname}"), pin),
        };
        if self.nets.specs[n].init.is_empty() {
            self.nets.specs[n].init = seed.unwrap_or_default();
        }
        self.nets.specs[n].param = true;
        self.params.insert((path.to_string(), pname.to_string()), NetId(n as u32));
        n
    }

    /// A `GLOBAL` pin: the one design-wide net of its constant (of its name for `NAMED`), seeded with the session's
    /// value; an analysis-kind global records the bytes that mean "this analysis runs". `None` for an `OPTIONAL`
    /// named global no session parameter sets.
    fn global_pin(&mut self, path: &str, pin: &Pin) -> Result<Option<usize>> {
        let Some((key, name, seed)) = self.global_net(path, pin)? else { return Ok(None) };
        let n = self.nets.get_or_add(&key, &name, pin);
        if self.nets.specs[n].init.is_empty() {
            self.nets.specs[n].init = seed.unwrap_or_default();
        }
        let flag = matches!(pin.global, global::DC | global::TRAN | global::AC | global::NOISE);
        if flag && !self.analyses.iter().any(|(g, _, _)| *g == pin.global) {
            let on = ParamValue::Bool(true)
                .encode(&pin.layout)
                .or_else(|_| ParamValue::Real(1.0).encode(&pin.layout))
                .map_err(|e| Bind::error(path, format!("`{}`: {e}", pin.name.as_str())))?;
            self.analyses.push((pin.global, NetId(n as u32), on));
        }
        Ok(Some(n))
    }

    /// The key, name and seed of the net of the `GLOBAL` pin `pin`; `None` for an `OPTIONAL` named global no session
    /// parameter sets.
    fn global_net(&self, path: &str, pin: &Pin) -> Result<Option<(String, String, Option<Vec<u8>>)>> {
        let pname = pin.name.as_str();
        let seeded = |value: std::result::Result<Vec<u8>, String>| value.map(Some).map_err(|e| Bind::error(path, format!("`{pname}`: {e}")));
        let net = |key: &str, name: &str, seed: Option<Vec<u8>>| Ok(Some((key.to_string(), name.to_string(), seed)));
        match pin.global {
            global::TEMPERATURE => net("global:temperature", "$temperature", seeded(ParamValue::Real(self.temperature).encode(&pin.layout))?),
            global::TEMPERATURE_GIVEN => net("global:temperature_given", "$temperature_given", seeded(Bound::flag_bytes(&pin.layout, false))?),
            global::RESOLUTION => net("global:resolution", "$resolution", seeded(ParamValue::Real(self.resolution).encode(&pin.layout))?),
            global::DC => net("global:dc", "$dc", None),
            global::TRAN => net("global:tran", "$tran", None),
            global::AC => net("global:ac", "$ac", None),
            global::NOISE => net("global:noise", "$noise", None),
            global::START => net("global:start", "$start", None),
            global::SETTLE => net("global:settle", "$settle", None),
            _ => {
                let Some(value) = self.session.get(pname) else {
                    if pin.flags & pin_flag::OPTIONAL != 0 {
                        return Ok(None);
                    }
                    return Err(Bind::error(path, format!("global `{pname}` names a session parameter nobody set")));
                };
                let bytes = value.encode(&pin.layout).map_err(|e| Bind::error(path, format!("`{pname}`: {e}")))?;
                Ok(Some((format!("named:{pname}"), format!("${pname}"), Some(bytes))))
            }
        }
    }

    /// Marks the nets the analog side reads and the crossing nets: the analog pins of each instance, in order.
    fn mark_analog_reads(&mut self, instances: &[InstanceBinding]) -> Vec<Vec<bool>> {
        let mut analog_read: Vec<Vec<bool>> = Vec::with_capacity(instances.len());
        for b in instances {
            let shape = b.analog.shape();
            let read = b.analog_pins();
            for (pi, pin) in shape.pins.iter().enumerate() {
                if let Some(n) = b.nets[pi]
                    && pin.kind == kind::DISCRETE
                    && read[pi]
                {
                    self.nets.specs[n].analog_reads = true;
                }
            }
            for w in shape.watches.iter().filter(|w| w.kind == watch::CROSS) {
                if let Some(Some(n)) = b.nets.get(w.event as usize) {
                    self.nets.specs[*n].crossing = true;
                }
            }
            analog_read.push(read);
        }
        analog_read
    }

    /// The samples analog and digital agree on: by instance, reaction and read order, the number of each continuous
    /// pin a reaction reads.
    fn sample_numbers(instances: &[InstanceBinding]) -> HashMap<(usize, usize), u32> {
        let mut sample_count = 0u32;
        let mut sample_of: HashMap<(usize, usize), u32> = HashMap::new();
        for (i, b) in instances.iter().enumerate() {
            for r in b.analog.shape().reactions.iter() {
                for c in r.reads.iter() {
                    let continuous = b.analog.shape().pins.get(c.index as usize).is_some_and(|p| p.kind == kind::CONTINUOUS);
                    if c.kind == cell::PIN && continuous {
                        sample_of.entry((i, c.index as usize)).or_insert(sample_count);
                        sample_count += 1;
                    }
                }
            }
        }
        sample_of
    }

    /// Builds the two plans of the bound `instances`, the analog copy's slots and seeds, and the engines over them.
    fn finish(mut self, instances: Vec<InstanceBinding>, microsteps: usize, time_point_rule: TimePointRule) -> Result<Bound> {
        let mut names = NameMap::default();
        let analog_read = self.mark_analog_reads(&instances);
        let sample_of = Self::sample_numbers(&instances);
        let mut analog_instances = Vec::with_capacity(instances.len());
        let mut digital_instances = Vec::with_capacity(instances.len());
        for (i, b) in instances.into_iter().enumerate() {
            let (analog, digital) = b.into_plans(i, &analog_read[i], &sample_of, &mut names);
            analog_instances.push(analog);
            digital_instances.push(digital);
        }
        self.nets.check_seeds()?;
        names.nets = self.nets.named.iter().map(|(k, v)| (k.clone(), NetId(*v as u32))).collect();
        names.params = self.params;
        // an array net the analog side reads hands it the windows its slots hold, no more
        for (net, spec) in self.nets.specs.iter_mut().enumerate().filter(|(_, spec)| spec.count > 0) {
            spec.analog_windows = names.array_slots(NetId(net as u32), spec.count).into_iter().map(|(_, first, length)| (first, length)).collect();
        }
        let seeds: Vec<(u32, Vec<u8>)> = names
            .slot_nets
            .iter()
            .enumerate()
            .map(|(slot, id)| {
                let spec = &self.nets.specs[id.0 as usize];
                names.slot_bytes(slot as u32, &spec.name, &spec.init, spec.size).map(|bytes| (slot as u32, bytes))
            })
            .collect::<Result<_>>()?;
        let analog_plan = AnalogPlan::bind(analog_instances, self.netlist)?;
        let digital_plan = DigitalPlan::bind(self.nets.specs, digital_instances, Resolution { seconds_per_tick: self.resolution })
            .map_err(|e| Error::simple(SolverDomain::Digital, e.to_string()))?;
        let mut analog = Assembly::new(analog_plan);
        for (slot, init) in seeds {
            if !init.is_empty() {
                analog.set_discrete(slot, &init);
            }
        }
        let digital = Digital::new(digital_plan, DigitalLimits { max_delta_cycles: microsteps, ..DigitalLimits::default() }, time_point_rule);
        let (holds, vntol, branch_abstol, analyses) = (self.holds, self.vntol, self.branch_abstol, self.analyses);
        Ok(Bound { analog, digital, names, holds, vntol, branch_abstol, analyses, host_writes: Vec::new(), temperature: Vec::new(), microsteps })
    }
}
