//! The analog half of bind (abi.md §9 "bind", design `device::plan`): each
//! instance's shape is validated, its analog functions are taken (F2: a
//! function that writes no pin), every term target is resolved to a row, and
//! the matrix pattern is built from the declarations alone: each term row
//! against each continuous pin a partial of that term names, directly or
//! through a `LIMIT` input's `PROPOSED` partials (`g·p`, abi.md A2). The
//! pattern depends on no value, so it is the same in every solve (C13).

use std::collections::HashSet;

use piperine_abi::caller::Handle;
use piperine_abi::raw::{Cell, GROUND, Shape, cell, input, kind, part};
use piperine_abi::validate::validate;

use super::device_error;
use crate::error::Error;
use crate::netlist::{AnalogReference, Netlist};

/// What the owner bound one pin of an instance to.
#[derive(Clone, Debug, PartialEq)]
pub enum AnalogBinding {
    /// A continuous pin's unknown (a ground reference counts as `Ground`).
    Unknown(AnalogReference),
    /// The reference node.
    Ground,
    /// A slot of the assembly's own copy of the discrete pins its functions
    /// read (abi.md §10: "its own copy, updated only by boundary events").
    Discrete(u32),
    /// Nothing: an unbound optional pin, or a pin the analog side never reads.
    Absent,
}

/// One instance handed to bind: its path, its handle and one binding per pin.
#[derive(Debug)]
pub struct AnalogInstance {
    pub path: String,
    pub handle: Handle,
    pub pins: Vec<AnalogBinding>,
}

/// One analog function taken from a shape.
#[derive(Debug)]
pub(crate) struct FnPlan {
    /// Its index in the shape's `functions`.
    pub k: u32,
}

/// One term: its rows and the columns its partials reach.
#[derive(Debug)]
pub(crate) struct TermPlan {
    pub plus: Option<AnalogReference>,
    pub minus: Option<AnalogReference>,
    /// The function (index into `InstancePlan::functions`) and write slot of
    /// its `VALUE`.
    pub value: Option<(usize, usize)>,
    /// `(partial, column)`: partials of the value against a continuous pin.
    pub direct: Vec<(usize, AnalogReference)>,
    /// `(partial, input)`: partials of the value against a held input's `VALUE`.
    pub via: Vec<(usize, usize)>,
}

/// One held input.
#[derive(Debug)]
pub(crate) struct InputPlan {
    pub kind: u32,
    /// `(partial, column)`: partials of `PROPOSED` (a `LIMIT`) against a
    /// continuous pin, in the function that writes it.
    pub p: Vec<(usize, AnalogReference)>,
}

/// One bound instance.
#[derive(Debug)]
pub(crate) struct InstancePlan {
    pub path: String,
    pub handle: Handle,
    pub functions: Vec<FnPlan>,
    pub terms: Vec<TermPlan>,
    pub inputs: Vec<InputPlan>,
}

/// The analog half of bind: every instance's analog functions, their cells'
/// sources and sinks, and the matrix pattern.
#[derive(Debug)]
pub struct AnalogPlan {
    pub(crate) instances: Vec<InstancePlan>,
    pattern: Vec<(AnalogReference, AnalogReference)>,
}

/// The error of one entry of one instance.
fn entry_error(path: &str, what: &str, name: &str, origin: &str, detail: String) -> Error {
    device_error(format!("instance `{path}`: {what} `{name}` ({origin}): {detail}"))
}

impl AnalogPlan {
    /// Binds `instances`. Fails naming the instance and the entry when a
    /// shape breaks a rule of abi.md §9 or a binding does not fit its pin.
    pub fn bind(instances: Vec<AnalogInstance>, netlist: &Netlist) -> Result<AnalogPlan, Error> {
        let mut bound = Vec::with_capacity(instances.len());
        for inst in instances {
            bound.push(bind_instance(inst, netlist)?);
        }
        let mut seen = HashSet::new();
        let mut pattern = Vec::new();
        for inst in &bound {
            for t in &inst.terms {
                let limits = t.via.iter().filter(|(_, i)| inst.inputs[*i].kind == input::LIMIT);
                let cols =
                    t.direct.iter().map(|(_, c)| c).chain(limits.flat_map(|(_, i)| inst.inputs[*i].p.iter().map(|(_, c)| c)));
                for col in cols {
                    for row in [&t.plus, &t.minus].into_iter().flatten() {
                        if seen.insert((row.idx(), col.idx())) {
                            pattern.push((row.clone(), col.clone()));
                        }
                    }
                }
            }
        }
        Ok(AnalogPlan { instances: bound, pattern })
    }

    /// The matrix positions the declared terms and partials can reach, each once.
    pub fn pattern(&self) -> Vec<(AnalogReference, AnalogReference)> {
        self.pattern.clone()
    }

    /// The number of bound instances, in the order `bind` received them.
    pub fn instances(&self) -> usize {
        self.instances.len()
    }

    /// The path of instance `i`.
    pub fn path(&self, i: usize) -> &str {
        &self.instances[i].path
    }

    /// The shape of instance `i`.
    pub fn shape(&self, i: usize) -> &Shape {
        self.instances[i].handle.shape()
    }

    /// The shape indices of the analog functions taken from instance `i`.
    pub fn analog_functions(&self, i: usize) -> Vec<u32> {
        self.instances[i].functions.iter().map(|f| f.k).collect()
    }
}

/// Whether a function is analog (F2): it writes, and writes no pin.
fn is_analog(writes: &[Cell]) -> bool {
    !writes.is_empty() && writes.iter().all(|c| c.kind != cell::PIN)
}

/// The column a continuous pin read reaches: `None` for ground.
fn column(pins: &[AnalogBinding], shape: &Shape, c: &Cell) -> Option<AnalogReference> {
    let pin = shape.pins.get(c.index as usize)?;
    if c.kind != cell::PIN || pin.kind != kind::CONTINUOUS {
        return None;
    }
    match &pins[c.index as usize] {
        AnalogBinding::Unknown(r) if r.idx().is_some() => Some(r.clone()),
        _ => None,
    }
}

fn bind_instance(inst: AnalogInstance, netlist: &Netlist) -> Result<InstancePlan, Error> {
    let AnalogInstance { path, handle, pins } = inst;
    let shape = handle.shape();
    validate(shape).map_err(|e| device_error(format!("instance `{path}`: {e}")))?;
    if pins.len() != shape.pins.len() {
        return Err(device_error(format!(
            "instance `{path}`: {} bindings for {} pins",
            pins.len(),
            shape.pins.len()
        )));
    }
    // Terms first, so a target bound to a discrete net is named by its term.
    let mut terms = Vec::with_capacity(shape.terms.len());
    for t in shape.terms.iter() {
        let side = |which: &str, target: u32| -> Result<Option<AnalogReference>, Error> {
            if target == GROUND {
                return Ok(None);
            }
            let pin = shape.pins.get(target as usize).expect("validated");
            match &pins[target as usize] {
                AnalogBinding::Unknown(r) if r.idx().is_some() => Ok(Some(r.clone())),
                AnalogBinding::Unknown(_) | AnalogBinding::Ground => Ok(None),
                b => Err(entry_error(
                    &path,
                    "term",
                    t.name.as_str(),
                    t.origin.as_str(),
                    format!("{which} is pin `{}`, bound to {b:?}: a term targets an unknown", pin.name.as_str()),
                )),
            }
        };
        let plus = side("plus", t.plus)?;
        let minus = side("minus", t.minus)?;
        terms.push(TermPlan { plus, minus, value: None, direct: Vec::new(), via: Vec::new() });
    }
    for (i, b) in pins.iter().enumerate() {
        let pin = shape.pins.get(i).expect("checked length");
        let fits = match b {
            AnalogBinding::Unknown(r) => {
                if r.idx().is_some() && netlist.variable_for(r).is_none() {
                    return Err(entry_error(
                        &path,
                        "pin",
                        pin.name.as_str(),
                        pin.origin.as_str(),
                        format!("bound to an unknown the netlist does not hold: {r:?}"),
                    ));
                }
                pin.kind == kind::CONTINUOUS
            }
            AnalogBinding::Ground => pin.kind == kind::CONTINUOUS,
            AnalogBinding::Discrete(_) => pin.kind == kind::DISCRETE,
            AnalogBinding::Absent => true,
        };
        if !fits {
            let what = if pin.kind == kind::CONTINUOUS { "continuous" } else { "discrete" };
            return Err(entry_error(
                &path,
                "pin",
                pin.name.as_str(),
                pin.origin.as_str(),
                format!("a {what} pin bound to {b:?}"),
            ));
        }
    }

    let mut inputs: Vec<InputPlan> = shape.inputs.iter().map(|x| InputPlan { kind: x.kind, p: Vec::new() }).collect();

    let mut functions = Vec::new();
    for (k, f) in shape.functions.iter().enumerate() {
        let writes: Vec<Cell> = f.writes.iter().copied().collect();
        if !is_analog(&writes) {
            continue;
        }
        let fi = functions.len();
        let reads: Vec<Cell> = f.reads.iter().copied().collect();
        for (w, c) in writes.iter().enumerate() {
            if c.kind == cell::TERM && c.part == part::VALUE {
                let t = &mut terms[c.index as usize];
                if t.value.is_some() {
                    let term = shape.terms.get(c.index as usize).expect("validated");
                    return Err(entry_error(
                        &path,
                        "term",
                        term.name.as_str(),
                        term.origin.as_str(),
                        "written by two functions".into(),
                    ));
                }
                t.value = Some((fi, w));
            }
        }
        for (pi, p) in f.partials.iter().enumerate() {
            let (w, r) = (writes[p.write as usize], reads[p.read as usize]);
            let col = column(&pins, shape, &r);
            let held = (r.kind == cell::INPUT && r.part == part::VALUE).then_some(r.index as usize);
            match (w.kind, w.part) {
                (cell::TERM, part::VALUE) => {
                    let t = &mut terms[w.index as usize];
                    if let Some(c) = col {
                        t.direct.push((pi, c));
                    } else if let Some(i) = held {
                        t.via.push((pi, i));
                    }
                }
                (cell::INPUT, part::PROPOSED) => {
                    if let Some(c) = col {
                        inputs[w.index as usize].p.push((pi, c));
                    }
                }
                _ => {}
            }
        }
        functions.push(FnPlan { k: k as u32 });
    }
    if functions.is_empty() {
        terms.clear();
        inputs.clear();
    }
    Ok(InstancePlan { path, handle, functions, terms, inputs })
}
