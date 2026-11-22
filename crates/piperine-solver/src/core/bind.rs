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
    Cell, DEFAULT, Layout, Module, Pin, Shape, cell, global, kind, leaf, pin_flag, role, watch,
};
use piperine_analog_solver::device::{AnalogBinding, AnalogInstance, AnalogPlan, Assembly};
use piperine_analog_solver::error::{Error, SolverDomain};
use piperine_analog_solver::netlist::{AnalogReference, AnalogVariable, BranchIdentifier, Netlist, NodeIdentifier};
use piperine_analog_solver::tolerances::Tolerances;
use piperine_digital_solver::engine::Digital;
use piperine_digital_solver::net::{NetId, NetSpec};
use piperine_digital_solver::plan::{Binding, DigitalPlan, InstancePlan};
use piperine_digital_solver::tick::Resolution;
use piperine_digital_solver::limits::DigitalLimits;

use crate::core::result::Result;

/// What a netlist hands to one `PORT` pin.
#[derive(Clone, Debug, PartialEq)]
pub enum Port {
    /// A continuous pin on this analog node.
    Node(AnalogReference),
    /// A discrete or event pin on the design-wide net of this name.
    Net(NetName),
    /// Nothing: the pin is private (an `OPTIONAL` pin reads as zero bytes).
    Open,
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
    /// The analysis-kind globals some pin reads (`DC`, `TRAN`, `AC`, `NOISE`):
    /// the constant, its net and the bytes that mean "this analysis runs".
    pub analyses: Vec<(u32, NetId, Vec<u8>)>,
    /// The host's writes to param nets between analyses: committed at the
    /// start of every analysis after the `init`s (abi.md §10 "the host's
    /// initial writes").
    pub host_writes: Vec<(NetId, Vec<u8>)>,
    /// The run's temperature when it is not the one the net was seeded with:
    /// committed at the start of every analysis (the `TEMPERATURE` global
    /// holds the session's value, abi.md §9).
    pub temperature: Option<(NetId, Vec<u8>)>,
    /// The bound on the microsteps of one instant the digital engine was built with.
    pub microsteps: usize,
}

/// What bind ran on, kept so that an instance can be created and bound again
/// (abi.md P5, P6: a `STRUCTURAL` write or a `RESHAPE` between analyses).
#[derive(Clone, Debug)]
pub(crate) struct Recipe {
    pub specs: Vec<DeviceSpec>,
    pub shared: Vec<(String, ParamValue)>,
    pub session: Vec<(String, ParamValue)>,
    pub resolution: f64,
    pub microsteps: usize,
    pub temperature: f64,
}

impl Bound {
    /// The param net of `(path, param)` and the bytes of `value` in its layout.
    pub(crate) fn param_bytes(&self, path: &str, param: &str, value: &ParamValue) -> Result<(NetId, Vec<u8>)> {
        let net = *self.names.params.get(&(path.to_string(), param.to_string())).ok_or_else(|| {
            let mut declared: Vec<&str> =
                self.names.params.keys().filter(|(p, _)| p == path).map(|(_, n)| n.as_str()).collect();
            declared.sort_unstable();
            if declared.is_empty() {
                bind_error(path, format!("no PARAM pin `{param}`; the instance declares no writable parameters"))
            } else {
                bind_error(path, format!("no PARAM pin `{param}`; available parameters: {}", declared.join(", ")))
            }
        })?;
        let layout = self.digital.plan().net_layout(net).ok_or_else(|| bind_error(path, format!("`{param}` has no layout")))?;
        let bytes = encode(layout, value).map_err(|e| bind_error(path, format!("`{param}`: {e}")))?;
        Ok((net, bytes))
    }

    /// The id of the discrete net `name` of the digital plan, and `value` in its layout.
    pub(crate) fn net_bytes(&self, name: &str, value: &ParamValue) -> Result<(NetId, Vec<u8>)> {
        let fail = |why: String| Error::simple(SolverDomain::Device, format!("net `{name}`: {why}"));
        let at = self.digital.plan().nets.iter().position(|n| n.name == name).ok_or_else(|| fail("no discrete net of the design has that name".into()))?;
        let net = NetId(at as u32);
        let layout = self.digital.plan().net_layout(net).ok_or_else(|| fail("no device pin reads it, so it has no layout".into()))?;
        let bytes = encode(layout, value).map_err(fail)?;
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

    /// The `TEMPERATURE` global holds `t` from the next analysis on; a net
    /// seeded with `t` and never moved needs no write.
    pub(crate) fn set_temperature(&mut self, t: f64) {
        let Some(net) = self.names.nets.get("global:temperature").copied() else { return };
        let Some(layout) = self.digital.plan().net_layout(net) else { return };
        let Ok(bytes) = encode(layout, &ParamValue::Real(t)) else { return };
        let seeded = self.digital.plan().nets[net.0 as usize].init == bytes;
        if let Some(slot) = self.names.slots.get(&net) {
            self.analog.set_discrete(*slot, &bytes);
        }
        self.temperature = (!seeded || self.temperature.is_some()).then_some((net, bytes));
    }

    /// Tightens `tol` with the per-node tolerances of the pins: a node keeps
    /// the smaller of its present override and the pins' (design AC 4).
    pub fn tighten(&self, tol: &mut Tolerances) {
        if self.vntol.is_empty() {
            return;
        }
        let mut map = (*tol.node_vntol_overrides).clone();
        for (id, v) in &self.vntol {
            let entry = map.entry(id.clone()).or_insert(*v);
            *entry = entry.min(*v);
        }
        tol.node_vntol_overrides = std::rc::Rc::new(map);
    }
}

/// What the builder hands to [`bind`].
pub(crate) struct BindInput<'a> {
    pub specs: Vec<DeviceSpec>,
    /// Shared param nets: name and seed.
    pub shared: Vec<(String, ParamValue)>,
    /// Session parameters for `NAMED` globals.
    pub session: Vec<(String, ParamValue)>,
    pub netlist: &'a mut Netlist,
    pub resolution: f64,
    pub microsteps: usize,
    pub temperature: f64,
}

fn bind_error(path: &str, detail: impl std::fmt::Display) -> Error {
    Error::simple(SolverDomain::Device, format!("instance `{path}`: {detail}"))
}

/// The bytes of `value` in `layout` (P3: the bytes of its type and nothing else).
pub(crate) fn encode(layout: &Layout, value: &ParamValue) -> std::result::Result<Vec<u8>, String> {
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
    Ok(bytes)
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

    /// A net of its own with the shape of `pin`.
    fn add(&mut self, name: &str, pin: &Pin) -> usize {
        let spec = if pin.kind == kind::EVENT {
            NetSpec::event(name)
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
        if spec.init.is_empty() && pin.flags & pin_flag::HAS_INIT != 0 && pin.role != role::PARAM {
            spec.init = pin.init.as_slice().to_vec();
        }
    }
}

/// The pins of `shape` an analog function reads, a term's or watch's pin
/// operand names: the discrete pins the analog copy holds.
fn analog_pins(shape: &Shape) -> Vec<bool> {
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

/// One instance while it is being bound.
struct Bind {
    path: String,
    analog: Handle,
    digital: Handle,
    sites: Vec<Site>,
    /// The net index of each discrete or event pin.
    nets: Vec<Option<usize>>,
}

fn create(module: &'static Module, path: &str, values: &[(u32, Vec<u8>)]) -> Result<Handle> {
    let given: Vec<(u32, &[u8])> = values.iter().map(|(p, b)| (*p, b.as_slice())).collect();
    Handle::create(module, &given).map_err(|e| bind_error(path, format!("create: {e}")))
}

/// Binds `input` to its two plans.
pub(crate) fn bind(input: BindInput<'_>) -> Result<Bound> {
    let BindInput { specs, shared, session, netlist, resolution, microsteps, temperature } = input;
    let mut nets = Nets::default();
    let mut shared_by_name: HashMap<String, ParamValue> = HashMap::new();
    for (name, seed) in shared {
        if shared_by_name.insert(name.clone(), seed).is_some() {
            return Err(Error::simple(SolverDomain::Device, format!("shared param net `{name}` is declared twice")));
        }
    }
    let session: HashMap<String, ParamValue> = session.into_iter().collect();

    let mut paths = std::collections::HashSet::new();
    let mut binds: Vec<Bind> = Vec::with_capacity(specs.len());
    let mut holds = Vec::new();
    let mut vntol: HashMap<NodeIdentifier, f64> = HashMap::new();
    let mut params = HashMap::new();
    let mut analyses: Vec<(u32, NetId, Vec<u8>)> = Vec::new();

    for spec in &specs {
        let path = spec.path.as_str();
        if !paths.insert(path.to_string()) {
            return Err(bind_error(path, "an instance of this path exists"));
        }
        // The values given, by prototype pin (P5: `Value.pin` means the same pin in every shape).
        let proto = spec.module.prototype();
        let mut values: Vec<(u32, Vec<u8>)> = Vec::new();
        let mut seeds: HashMap<u32, Vec<u8>> = HashMap::new();
        let mut shared_pins: HashMap<u32, String> = HashMap::new();
        for (name, value) in &spec.values {
            let Some((pi, pin)) = proto
                .pins
                .iter()
                .enumerate()
                .find(|(_, p)| p.role == role::PARAM && p.name.as_str() == name)
            else {
                return Err(bind_error(path, format!("no PARAM pin `{name}`")));
            };
            let resolved = match value {
                ParamValue::Shared(net) => {
                    let seed = shared_by_name
                        .get(net)
                        .ok_or_else(|| bind_error(path, format!("`{name}`: no shared param net `{net}`")))?;
                    shared_pins.insert(pi as u32, net.clone());
                    seed.clone()
                }
                v => v.clone(),
            };
            let bytes = encode(&pin.layout, &resolved).map_err(|e| bind_error(path, format!("`{name}`: {e}")))?;
            seeds.insert(pi as u32, bytes.clone());
            values.push((pi as u32, bytes));
        }
        let analog = create(spec.module, path, &values)?;
        // One `create` per placement: both sides call the same instance (an external
        // device holds state; two copies would diverge). The assembly's evals and the
        // engine's reactions never run concurrently in this owner, and the handle's
        // lock serializes them anyway.
        let digital = analog.share();
        let shape = analog.shape();
        let mut sites = Vec::with_capacity(shape.pins.len());
        let mut pin_nets = Vec::with_capacity(shape.pins.len());
        let mut ports = spec.ports.iter();
        let mut given = 0usize;
        for (pi, pin) in shape.pins.iter().enumerate() {
            let pname = pin.name.as_str();
            let optional = pin.flags & pin_flag::OPTIONAL != 0;
            let take = |ports: &mut std::slice::Iter<'_, Port>, given: &mut usize| -> Port {
                match ports.next() {
                    Some(p) => {
                        *given += 1;
                        p.clone()
                    }
                    None => Port::Open,
                }
            };
            let (site, net) = match pin.kind {
                kind::CONTINUOUS => {
                    let site = match pin.role {
                        role::PORT => match take(&mut ports, &mut given) {
                            Port::Node(r) if r.variable().is_ground() => Site::Ground,
                            Port::Node(r) => Site::Unknown(r),
                            Port::Open => Site::Unknown(netlist.connect_branch(BranchIdentifier::new(path, pname))),
                            Port::Net(n) => {
                                return Err(bind_error(path, format!("continuous pin `{pname}` bound to net `{n}`")));
                            }
                        },
                        role::PRIVATE => Site::Unknown(netlist.connect_branch(BranchIdentifier::new(path, pname))),
                        _ => return Err(bind_error(path, format!("continuous pin `{pname}` is neither PORT nor PRIVATE"))),
                    };
                    if let Site::Unknown(r) = &site {
                        if pin.flags & pin_flag::HAS_INIT != 0 {
                            let b = pin.init.as_slice();
                            let v = b.get(..8).and_then(|b| b.try_into().ok()).map(f64::from_le_bytes);
                            holds.push(Hold {
                                reference: r.clone(),
                                value: v.ok_or_else(|| bind_error(path, format!("`{pname}`: init is not an F64")))?,
                            });
                        }
                        if pin.abstol[0] > 0.0
                            && let AnalogVariable::Node(id) = r.variable().as_ref()
                        {
                            let e = vntol.entry(id.clone()).or_insert(pin.abstol[0]);
                            *e = e.min(pin.abstol[0]);
                        }
                    }
                    (site, None)
                }
                kind::DISCRETE | kind::EVENT => {
                    let private = format!("{path}.{pname}");
                    let net = match pin.role {
                        role::PORT => match take(&mut ports, &mut given) {
                            Port::Net(n) => Some(nets.get_or_add(&format!("net:{n}"), &n, pin)),
                            Port::Open if optional => None,
                            Port::Open => Some(nets.add(&private, pin)),
                            Port::Node(_) => {
                                return Err(bind_error(path, format!("discrete pin `{pname}` bound to an analog node")));
                            }
                        },
                        role::PARAM => {
                            let seed = seeds.get(&(pi as u32)).cloned().or_else(|| {
                                (pin.flags & pin_flag::HAS_INIT != 0).then(|| pin.init.as_slice().to_vec())
                            });
                            let n = match shared_pins.get(&(pi as u32)) {
                                Some(name) => nets.get_or_add(&format!("param:{name}"), name, pin),
                                None => nets.add(&private, pin),
                            };
                            if nets.specs[n].init.is_empty() {
                                nets.specs[n].init = seed.unwrap_or_default();
                            }
                            nets.specs[n].param = true;
                            params.insert((path.to_string(), pname.to_string()), NetId(n as u32));
                            Some(n)
                        }
                        role::GLOBAL => {
                            let (key, name, seed): (String, String, Option<Vec<u8>>) = match pin.global {
                                global::TEMPERATURE => (
                                    "global:temperature".into(),
                                    "$temperature".into(),
                                    Some(encode(&pin.layout, &ParamValue::Real(temperature)).map_err(|e| bind_error(path, format!("`{pname}`: {e}")))?),
                                ),
                                global::RESOLUTION => (
                                    "global:resolution".into(),
                                    "$resolution".into(),
                                    Some(encode(&pin.layout, &ParamValue::Real(resolution)).map_err(|e| bind_error(path, format!("`{pname}`: {e}")))?),
                                ),
                                global::DC => ("global:dc".into(), "$dc".into(), None),
                                global::TRAN => ("global:tran".into(), "$tran".into(), None),
                                global::AC => ("global:ac".into(), "$ac".into(), None),
                                global::NOISE => ("global:noise".into(), "$noise".into(), None),
                                global::START => ("global:start".into(), "$start".into(), None),
                                global::SETTLE => ("global:settle".into(), "$settle".into(), None),
                                _ => {
                                    let Some(value) = session.get(pname) else {
                                        if optional {
                                            sites.push(Site::Absent);
                                            pin_nets.push(None);
                                            continue;
                                        }
                                        return Err(bind_error(
                                            path,
                                            format!("global `{pname}` names a session parameter nobody set"),
                                        ));
                                    };
                                    let bytes = encode(&pin.layout, value)
                                        .map_err(|e| bind_error(path, format!("`{pname}`: {e}")))?;
                                    (format!("named:{pname}"), format!("${pname}"), Some(bytes))
                                }
                            };
                            let n = nets.get_or_add(&key, &name, pin);
                            if nets.specs[n].init.is_empty() {
                                nets.specs[n].init = seed.unwrap_or_default();
                            }
                            let flag = matches!(pin.global, global::DC | global::TRAN | global::AC | global::NOISE);
                            if flag && !analyses.iter().any(|(g, _, _)| *g == pin.global) {
                                let on = encode(&pin.layout, &ParamValue::Bool(true))
                                    .or_else(|_| encode(&pin.layout, &ParamValue::Real(1.0)))
                                    .map_err(|e| bind_error(path, format!("`{pname}`: {e}")))?;
                                analyses.push((pin.global, NetId(n as u32), on));
                            }
                            Some(n)
                        }
                        _ => Some(nets.add(&private, pin)),
                    };
                    if let Some(n) = net {
                        nets.merge(n, pin);
                    }
                    (net.map_or(Site::Absent, |n| Site::Net(NetId(n as u32))), net)
                }
                _ => return Err(bind_error(path, format!("pin `{pname}` has an unknown kind"))),
            };
            sites.push(site);
            pin_nets.push(net);
        }
        if given < spec.ports.len() {
            return Err(bind_error(path, format!("{} ports given for {given} PORT pins", spec.ports.len())));
        }
        binds.push(Bind { path: spec.path.clone(), analog, digital, sites, nets: pin_nets });
    }

    // The nets the analog side reads, the crossing nets, and the analog copy's slots.
    let mut names = NameMap::default();
    let mut analog_read: Vec<Vec<bool>> = Vec::with_capacity(binds.len());
    for b in &binds {
        let shape = b.analog.shape();
        let read = analog_pins(shape);
        for (pi, pin) in shape.pins.iter().enumerate() {
            if let Some(n) = b.nets[pi]
                && pin.kind == kind::DISCRETE
                && read[pi]
            {
                nets.specs[n].analog_reads = true;
            }
        }
        for w in shape.watches.iter().filter(|w| w.kind == watch::CROSS) {
            if let Some(Some(n)) = b.nets.get(w.event as usize) {
                nets.specs[*n].crossing = true;
            }
        }
        analog_read.push(read);
    }
    let mut analog_instances = Vec::with_capacity(binds.len());
    let mut digital_instances = Vec::with_capacity(binds.len());
    let mut sample_count = 0u32;
    let mut sample_of: HashMap<(usize, usize), u32> = HashMap::new();
    for (i, b) in binds.iter().enumerate() {
        // The samples analog and digital agree on: instance, reaction and read order.
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
    for (i, b) in binds.into_iter().enumerate() {
        let shape = b.analog.shape();
        let mut abinds = Vec::with_capacity(b.sites.len());
        let mut dbinds = Vec::with_capacity(b.sites.len());
        for (pi, site) in b.sites.iter().enumerate() {
            let pin = shape.pins.get(pi).expect("one site per pin");
            match site {
                Site::Unknown(r) => {
                    abinds.push(AnalogBinding::Unknown(r.clone()));
                    dbinds.push(Binding::Sample(sample_of.get(&(i, pi)).copied().unwrap_or(u32::MAX)));
                }
                Site::Ground => {
                    abinds.push(AnalogBinding::Ground);
                    dbinds.push(Binding::Sample(sample_of.get(&(i, pi)).copied().unwrap_or(u32::MAX)));
                }
                Site::Net(id) => {
                    dbinds.push(Binding::Net(*id));
                    if pin.kind == kind::DISCRETE && analog_read[i][pi] {
                        let slot = *names.slots.entry(*id).or_insert_with(|| {
                            names.slot_nets.push(*id);
                            names.slot_nets.len() as u32 - 1
                        });
                        abinds.push(AnalogBinding::Discrete(slot));
                    } else {
                        abinds.push(AnalogBinding::Absent);
                    }
                }
                Site::Absent => {
                    abinds.push(AnalogBinding::Absent);
                    dbinds.push(Binding::Absent);
                }
            }
        }
        names.instances.push(b.path.clone());
        names.pin_names.push(shape.pins.iter().map(|p| p.name.as_str().to_string()).collect());
        names.pins.push(b.sites);
        analog_instances.push(AnalogInstance { path: b.path.clone(), handle: b.analog, pins: abinds });
        digital_instances.push(InstancePlan { path: b.path, handle: b.digital, pins: dbinds });
    }
    for spec in &nets.specs {
        if !spec.init.is_empty() && spec.init.len() != spec.size as usize {
            return Err(Error::simple(
                SolverDomain::Device,
                format!("net `{}` holds {} bytes and its seed {}", spec.name, spec.size, spec.init.len()),
            ));
        }
    }
    names.nets = nets.named.iter().map(|(k, v)| (k.clone(), NetId(*v as u32))).collect();
    names.params = params;

    let seeds: Vec<(u32, Vec<u8>)> = names
        .slot_nets
        .iter()
        .enumerate()
        .map(|(slot, id)| (slot as u32, nets.specs[id.0 as usize].init.clone()))
        .collect();
    let analog_plan = AnalogPlan::bind(analog_instances, netlist)?;
    let digital_plan = DigitalPlan::bind(nets.specs, digital_instances, Resolution { seconds_per_tick: resolution })
        .map_err(|e| Error::simple(SolverDomain::Digital, e.to_string()))?;
    let mut analog = Assembly::new(analog_plan);
    for (slot, init) in seeds {
        if !init.is_empty() {
            analog.set_discrete(slot, &init);
        }
    }
    let digital = Digital::new(digital_plan, DigitalLimits { max_delta_cycles: microsteps, ..DigitalLimits::default() });
    Ok(Bound { analog, digital, names, holds, vntol, analyses, host_writes: Vec::new(), temperature: None, microsteps })
}
