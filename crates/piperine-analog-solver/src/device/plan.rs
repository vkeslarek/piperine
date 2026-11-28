//! The analog half of bind (abi.md §9 "bind", design `device::plan`): each
//! instance's shape is validated, its analog functions are taken (F2: a
//! function that writes no pin), every term target is resolved to a row, and
//! the matrix pattern is built from the declarations alone: each term row
//! against each continuous pin a partial of that term names, directly or
//! through a `LIMIT` input's `PROPOSED` partials (`g·p`, abi.md A2). The
//! pattern depends on no value, so it is the same in every solve (C13).

use std::collections::HashSet;

use piperine_abi::caller::Handle;
use piperine_abi::device::Access;
use piperine_abi::raw::{Cell, DEFAULT, Function, GROUND, Shape, cell, input, kind, part, watch};
use piperine_abi::validate::Validator;

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

/// Where the bytes of one read cell come from in a solve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Source {
    /// The unknown at this index of the iterate.
    Unknown(usize),
    /// Ground, or an unbound discrete pin: all zero (P3).
    Zero,
    /// A discrete slot's bytes.
    Slot(usize),
    /// A discrete slot's bytes before its most recent change.
    Previous(usize),
    /// A held input's `VALUE` (abi.md §4.2).
    Held(usize),
    /// A `LIMIT` input's `LAST`.
    Last(usize),
    /// A `LIMIT` input's `AGE`.
    Age(usize),
    /// An ambient input, by its constant.
    Ambient(u32),
}

/// One analog function taken from a shape.
#[derive(Debug)]
pub(crate) struct FnPlan {
    /// Its index in the shape's `functions`.
    pub k: u32,
    pub name: String,
    pub origin: String,
    /// The source and byte length of each read cell.
    pub reads: Vec<(Source, usize)>,
    /// The byte length of each written cell.
    pub write_lens: Vec<usize>,
    /// One byte per partial: 1 when a Newton solve seeds it (against a
    /// continuous pin or a held input; never against a param).
    pub seed: Vec<u8>,
    /// Writes a term or a limit: needed by every Newton solve.
    pub rows: bool,
    /// Writes a limit's `PROPOSED` or `LIMITED`: runs before the readers.
    pub limits: bool,
    /// Writes a `STIM` input's `RE` or `IM`: needed by a small-signal sweep.
    pub stim: bool,
    /// Writes a `NOISE` input's `PSD`: needed by a noise sweep only.
    pub noise: bool,
    /// Writes a `VAR`, `CROSS` or `CAP` watch: needed at an accepted solution.
    pub watch: bool,
    /// `(write slot, watch)` of each `TRACE` watch it writes.
    pub traces: Vec<(usize, usize)>,
}

/// One term: its rows and the columns its partials reach.
#[derive(Debug)]
pub(crate) struct TermPlan {
    pub name: String,
    pub origin: String,
    /// `FLOW`, `DDT` or `DELAY`.
    pub op: u32,
    pub plus: Option<AnalogReference>,
    pub minus: Option<AnalogReference>,
    /// The function (index into `InstancePlan::functions`) and write slot of
    /// its `VALUE`.
    pub value: Option<(usize, usize)>,
    /// The function and write slot of a `DELAY` term's `TIME` (its `td`).
    pub td: Option<(usize, usize)>,
    /// The discrete slot of a `DELAY` term's `max`; `None`: unbounded.
    pub max: Option<usize>,
    /// The discrete slot of a `DDT` term's own `abstol`; `None`: the run's flow `abstol`.
    pub abstol: Option<usize>,
    /// `(partial, column)`: partials of the value against a continuous pin.
    pub direct: Vec<(usize, AnalogReference)>,
    /// `(partial, input)`: partials of the value against a held input's `VALUE`.
    pub via: Vec<(usize, usize)>,
}

/// One held input.
#[derive(Debug)]
pub(crate) struct InputPlan {
    pub name: String,
    pub origin: String,
    pub kind: u32,
    /// The function and write slot of `PROPOSED` and of `LIMITED`.
    pub proposed: Option<(usize, usize)>,
    pub limited: Option<(usize, usize)>,
    /// The function and write slot of a `STIM` input's `RE` and `IM`.
    pub re: Option<(usize, usize)>,
    pub im: Option<(usize, usize)>,
    /// The function and write slot of a `NOISE` input's `PSD`.
    pub psd: Option<(usize, usize)>,
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
    pub watches: Vec<WatchPlan>,
}

/// One watch (abi.md §4.3).
#[derive(Debug)]
pub(crate) struct WatchPlan {
    pub name: String,
    pub origin: String,
    /// `VAR`, `CROSS`, `CAP` or `TRACE`.
    pub kind: u32,
    /// A `CROSS` watch's event pin.
    pub event: u32,
    /// The discrete slots of a `CROSS` watch's `ttol` and `etol` pins; `None`: `DEFAULT`.
    pub ttol: Option<usize>,
    pub etol: Option<usize>,
    /// The function and write slot of its `VALUE`.
    pub value: Option<(usize, usize)>,
}

/// The analog half of bind: every instance's analog functions, their cells'
/// sources and sinks, and the matrix pattern.
#[derive(Debug)]
pub struct AnalogPlan {
    pub(crate) instances: Vec<InstancePlan>,
    /// The byte size of each discrete slot.
    pub(crate) slots: Vec<usize>,
    /// The unknown each continuous pin read by a reaction samples, in
    /// instance, reaction and read order; `None` for ground.
    pub(crate) samples: Vec<Option<usize>>,
    pattern: Vec<(AnalogReference, AnalogReference)>,
}

/// A lookup into a shape that passed `validate`: an index the validator vouched for that still
/// misses is the shape's error, not a panic.
struct ShapeLookup;

impl ShapeLookup {
    /// `found`, or the error naming the instance, the kind of entry and its index.
    fn get<T>(found: Option<T>, path: &str, what: &str, index: usize) -> Result<T, Error> {
        found.ok_or_else(|| Error::device(format!("instance `{path}`: {what} {index} is outside its shape")))
    }
}

impl AnalogPlan {
    /// Binds `instances`. Fails naming the instance and the entry when a
    /// shape breaks a rule of abi.md §9 or a binding does not fit its pin.
    pub fn bind(instances: Vec<AnalogInstance>, netlist: &Netlist) -> Result<AnalogPlan, Error> {
        let mut bound = Vec::with_capacity(instances.len());
        let mut slots = Vec::new();
        let mut samples = Vec::new();
        for inst in instances {
            bound.push(Self::bind_instance(inst, netlist, &mut slots, &mut samples)?);
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
        let slots = slots.into_iter().map(|s| s.unwrap_or(0)).collect();
        Ok(AnalogPlan { instances: bound, slots, samples, pattern })
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
    pub fn shape(&self, i: usize) -> &Shape<'_> {
        self.instances[i].handle.shape()
    }

    /// The shape indices of the analog functions taken from instance `i`.
    /// Kept for a host that lists the analog functions of an instance, as the plan tests do.
    pub fn analog_functions(&self, i: usize) -> Vec<u32> {
        self.instances[i].functions.iter().map(|f| f.k).collect()
    }
}

impl AnalogPlan {
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

    /// The source of read cell `c` of function `f`.
    fn source(path: &str, shape: &Shape, pins: &[AnalogBinding], f: &Function, c: &Cell) -> Result<Source, Error> {
        Ok(match c.kind {
            cell::PIN => {
                let pin = ShapeLookup::get(shape.pins.get(c.index as usize), path, "pin", c.index as usize)?;
                let slot = match &pins[c.index as usize] {
                    AnalogBinding::Discrete(s) => *s as usize,
                    AnalogBinding::Unknown(r) => return Ok(r.idx().map_or(Source::Zero, Source::Unknown)),
                    AnalogBinding::Ground => return Ok(Source::Zero),
                    AnalogBinding::Absent if pin.kind == kind::CONTINUOUS => {
                        return Err(Error::device_entry(
                            path,
                            "function",
                            f.name.as_str(),
                            f.origin.as_str(),
                            format!("reads continuous pin `{}`, which is bound to no unknown", pin.name.as_str()),
                        ));
                    }
                    AnalogBinding::Absent => return Ok(Source::Zero),
                };
                if c.part == part::PREVIOUS { Source::Previous(slot) } else { Source::Slot(slot) }
            }
            cell::INPUT => match c.part {
                part::LAST => Source::Last(c.index as usize),
                part::AGE => Source::Age(c.index as usize),
                _ => Source::Held(c.index as usize),
            },
            _ => Source::Ambient(c.index),
        })
    }

    /// Binds one instance: validates its shape, resolves its terms, pins, watches and inputs, and takes
    /// its analog functions.
    fn bind_instance(
        inst: AnalogInstance,
        netlist: &Netlist,
        slots: &mut Vec<Option<usize>>,
        samples: &mut Vec<Option<usize>>,
    ) -> Result<InstancePlan, Error> {
        let AnalogInstance { path, handle, pins } = inst;
        let shape = handle.shape();
        Validator::validate(shape).map_err(|e| Error::device(format!("instance `{path}`: {e}")))?;
        if pins.len() != shape.pins.len() {
            return Err(Error::device(format!(
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
                let pin = ShapeLookup::get(shape.pins.get(target as usize), &path, "pin", target as usize)?;
                match &pins[target as usize] {
                    AnalogBinding::Unknown(r) if r.idx().is_some() => Ok(Some(r.clone())),
                    AnalogBinding::Unknown(_) | AnalogBinding::Ground => Ok(None),
                    b => Err(Error::device_entry(
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
            let max = match pins.get(t.max as usize) {
                Some(AnalogBinding::Discrete(s)) if t.max != DEFAULT => Some(*s as usize),
                _ => None,
            };
            let abstol = match pins.get(t.abstol as usize) {
                Some(AnalogBinding::Discrete(s)) if t.abstol != DEFAULT => Some(*s as usize),
                _ => None,
            };
            terms.push(TermPlan {
                name: t.name.as_str().into(),
                origin: t.origin.as_str().into(),
                op: t.op,
                plus,
                minus,
                value: None,
                td: None,
                max,
                abstol,
                direct: Vec::new(),
                via: Vec::new(),
            });
        }
        for (i, b) in pins.iter().enumerate() {
            let pin = ShapeLookup::get(shape.pins.get(i), &path, "pin", i)?;
            let fits = match b {
                AnalogBinding::Unknown(r) => {
                    if r.idx().is_some() && netlist.variable_for(r).is_none() {
                        return Err(Error::device_entry(
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
                AnalogBinding::Discrete(slot) if pin.kind == kind::DISCRETE => {
                    let (slot, size) = (*slot as usize, pin.layout.size as usize);
                    if slots.len() <= slot {
                        slots.resize(slot + 1, None);
                    }
                    match slots[slot] {
                        Some(other) if other != size => {
                            return Err(Error::device_entry(
                                &path,
                                "pin",
                                pin.name.as_str(),
                                pin.origin.as_str(),
                                format!("{size} bytes on discrete slot {slot}, which another pin binds with {other} (P4)"),
                            ));
                        }
                        _ => slots[slot] = Some(size),
                    }
                    true
                }
                AnalogBinding::Discrete(_) => false,
                AnalogBinding::Absent => true,
            };
            if !fits {
                let what = if pin.kind == kind::CONTINUOUS { "continuous" } else { "discrete" };
                return Err(Error::device_entry(
                    &path,
                    "pin",
                    pin.name.as_str(),
                    pin.origin.as_str(),
                    format!("a {what} pin bound to {b:?}"),
                ));
            }
        }

        // The continuous pins the digital side's reactions read (abi.md R2).
        for r in shape.reactions.iter() {
            for c in r.reads.iter() {
                let continuous = shape.pins.get(c.index as usize).is_some_and(|p| p.kind == kind::CONTINUOUS);
                if c.kind == cell::PIN && continuous {
                    samples.push(match &pins[c.index as usize] {
                        AnalogBinding::Unknown(r) => r.idx(),
                        _ => None,
                    });
                }
            }
        }
        let mut watches: Vec<WatchPlan> = shape
            .watches
            .iter()
            .map(|w| {
                let slot = |p: u32| match pins.get(p as usize) {
                    Some(AnalogBinding::Discrete(s)) if p != DEFAULT && w.kind == watch::CROSS => Some(*s as usize),
                    _ => None,
                };
                WatchPlan {
                    name: w.name.as_str().into(),
                    origin: w.origin.as_str().into(),
                    kind: w.kind,
                    event: w.event,
                    ttol: slot(w.ttol),
                    etol: slot(w.etol),
                    value: None,
                }
            })
            .collect();
        let mut inputs: Vec<InputPlan> = shape
            .inputs
            .iter()
            .map(|x| InputPlan {
                name: x.name.as_str().into(),
                origin: x.origin.as_str().into(),
                kind: x.kind,
                proposed: None,
                limited: None,
                re: None,
                im: None,
                psd: None,
                p: Vec::new(),
            })
            .collect();

        let mut functions = Vec::new();
        for (k, f) in shape.functions.iter().enumerate() {
            let writes: Vec<Cell> = f.writes.iter().copied().collect();
            if !Self::is_analog(&writes) {
                continue;
            }
            let fi = functions.len();
            let reads: Vec<Cell> = f.reads.iter().copied().collect();
            for (w, c) in writes.iter().enumerate() {
                match (c.kind, c.part) {
                    (cell::INPUT, part::PROPOSED) => inputs[c.index as usize].proposed = Some((fi, w)),
                    (cell::INPUT, part::LIMITED) => inputs[c.index as usize].limited = Some((fi, w)),
                    (cell::INPUT, part::RE) => inputs[c.index as usize].re = Some((fi, w)),
                    (cell::INPUT, part::IM) => inputs[c.index as usize].im = Some((fi, w)),
                    (cell::INPUT, part::PSD) => inputs[c.index as usize].psd = Some((fi, w)),
                    (cell::WATCH, _) => watches[c.index as usize].value = Some((fi, w)),
                    (cell::TERM, part::TIME) => terms[c.index as usize].td = Some((fi, w)),
                    _ => {}
                }
                if c.kind == cell::TERM && c.part == part::VALUE {
                    let t = &mut terms[c.index as usize];
                    if t.value.is_some() {
                        let term = ShapeLookup::get(shape.terms.get(c.index as usize), &path, "term", c.index as usize)?;
                        return Err(Error::device_entry(
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
                let col = Self::column(&pins, shape, &r);
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
            let mut sources = Vec::with_capacity(reads.len());
            for c in &reads {
                sources.push(Self::source(&path, shape, &pins, f, c)?);
            }
            let read_lens = ShapeLookup::get(shape.cell_lens(&f.reads, Access::FunctionRead), &path, "read of function", k)?;
            let seed = f
                .partials
                .iter()
                .map(|p| {
                    let r = reads[p.read as usize];
                    let continuous = shape.pins.get(r.index as usize).is_some_and(|x| x.kind == kind::CONTINUOUS);
                    let wanted = (r.kind == cell::PIN && continuous) || (r.kind == cell::INPUT && r.part == part::VALUE);
                    u8::from(wanted)
                })
                .collect();
            let limits = writes.iter().any(|c| c.kind == cell::INPUT && matches!(c.part, part::PROPOSED | part::LIMITED));
            let rows = limits || writes.iter().any(|c| c.kind == cell::TERM);
            functions.push(FnPlan {
                k: k as u32,
                name: f.name.as_str().into(),
                origin: f.origin.as_str().into(),
                reads: sources.into_iter().zip(read_lens).collect(),
                write_lens: ShapeLookup::get(shape.cell_lens(&f.writes, Access::FunctionWrite), &path, "write of function", k)?,
                seed,
                rows,
                limits,
                stim: writes.iter().any(|c| c.kind == cell::INPUT && matches!(c.part, part::RE | part::IM)),
                noise: writes.iter().any(|c| c.kind == cell::INPUT && c.part == part::PSD),
                watch: writes.iter().any(|c| {
                    c.kind == cell::WATCH && shape.watches.get(c.index as usize).is_some_and(|w| w.kind != watch::TRACE)
                }),
                traces: writes
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| {
                        c.kind == cell::WATCH && shape.watches.get(c.index as usize).is_some_and(|w| w.kind == watch::TRACE)
                    })
                    .map(|(w, c)| (w, c.index as usize))
                    .collect(),
            });
        }
        if functions.is_empty() {
            terms.clear();
            inputs.clear();
            watches.clear();
        }
        Ok(InstancePlan { path, handle, functions, terms, inputs, watches })
    }
}
