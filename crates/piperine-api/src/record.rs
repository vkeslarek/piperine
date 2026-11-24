//! `Record`: an `emit!` record as the host receives it; `Streams`, which decodes the solver's
//! record observations into records; and `RecordSink`, where a session hands each one over as
//! the solver delivers it (CLI-06; contract D1, D2, D7).
//!
//! A record names its site: `ObservationKind::Record { site: Some((reaction, statement)), bytes }`
//! on the pin path `<instance path>.<stream>`. `Ir::record_at` gives the instance and the site;
//! the site's `emit!` statement gives the fields in source order; each value field is read from
//! the bytes through the stream's `RecordLayout` (a carrier of the field's type at its offset,
//! the value only), each text field is the site's literal, which never crosses the
//! ABI. A record is never paired with a commit by order. A record with no site has every
//! value field of its stream and no text field.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use piperine_ir::{EmitField, Ir, Stmt, Stream};
use piperine_plan::RecordLayout;
use piperine_solver::protocol::{AnalysisKind, Observation, ObservationKind, Sink};

use crate::composite::{CellShape, CompositeCell};
use crate::failure::Failure;
use crate::recorder::Recorder;
use crate::table::Cell;

/// One `emit!` record: when, which instance's site, which stream, and its fields in source
/// order.
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub time: f64,
    pub instance: String,
    pub stream: String,
    pub fields: Vec<(String, Field)>,
    /// The Newton iteration that wrote the record: `Some` for a trace (`emit!` in `analog`, §19 rule
    /// 3), `None` for the record of a reaction.
    pub iteration: Option<u32>,
    /// The analysis that wrote the record, when the solver says which.
    pub analysis: Option<AnalysisKind>,
}

impl Record {
    /// Whether the field `name` is the `bool` true on this record: a `sim` record's `finish` or
    /// `failed`.
    pub fn holds(&self, name: &str) -> bool {
        self.fields.iter().any(|(field, value)| field == name && *value == Field::Bool(true))
    }
}

/// The value of one field of a record.
#[derive(Debug, Clone, PartialEq)]
pub enum Field {
    Real(f64),
    Bool(bool),
    Int(i128),
    /// An `f16`, `f32` or `f128`, exact.
    Float(crate::number_cell::FloatCell),
    /// An integer wider than 64 bits, exact.
    Wide(crate::number_cell::WideCell),
    /// A text literal of the site.
    Text(String),
    /// The name of the variant an enum with no payload holds (CV-22).
    Variant(String),
    /// A tuple, an array, a struct, an enum or an `Option`: the tree of its parts.
    Composite(Box<CompositeCell>),
}

/// Where a session hands the records of a run, as the solver delivers them.
pub trait RecordSink {
    fn record(&mut self, record: &Record);
}

/// The record streams of a design, by pin path, each with the layout of its bytes.
#[derive(Debug, Clone)]
pub struct Streams {
    ir: Arc<Ir>,
    /// `<instance path>.<stream>` to the instance path, the stream and its layout.
    pins: HashMap<String, (String, Stream, RecordLayout)>,
    /// The pin paths `decode` gives records for; `None` keeps every stream.
    kept: Option<HashSet<String>>,
    /// `<instance path>.trace:<k>` to the instance path, the site and the site's stream, for every
    /// trace site (`emit!` in `analog`).
    traces: HashMap<String, (String, u32, String)>,
}

impl Streams {
    /// Every stream of every instance of `ir`.
    pub fn of(ir: Arc<Ir>) -> Streams {
        let mut pins = HashMap::new();
        let mut traces = HashMap::new();
        for instance in ir.instances() {
            for (site, trace) in instance.body().traces().iter().enumerate() {
                if let Some(stream) = instance.body().streams().get(trace.stream as usize) {
                    traces.insert(format!("{}.trace:{site}", instance.path()), (instance.path().to_string(), site as u32, stream.name.clone()));
                }
            }
            for stream in instance.body().streams() {
                // `check()` refuses a stream field no carrier holds, so every checked stream has
                // a layout
                if let Some(layout) = RecordLayout::in_ir(&ir, stream) {
                    pins.insert(format!("{}.{}", instance.path(), stream.name), (instance.path().to_string(), stream.clone(), layout));
                }
            }
        }
        Streams { ir, pins, kept: None, traces }
    }

    /// The pin path `<instance path>.<stream>` of every stream of the design, sorted.
    pub fn paths(&self) -> Vec<&str> {
        let mut paths: Vec<&str> = self.pins.keys().map(String::as_str).collect();
        paths.sort_unstable();
        paths
    }

    /// Keeps only the streams `entries` name (CLI-12 AC 1, 2): an entry is a stream's name,
    /// which keeps that stream of every instance, or one stream's full path
    /// `<instance path>.<stream>`. No entry keeps every stream. An entry that matches no stream
    /// of the design is `E0801`.
    pub fn keep(&mut self, entries: &[String]) -> Result<(), Failure> {
        if entries.is_empty() {
            return Ok(());
        }
        let mut kept = HashSet::new();
        for entry in entries {
            let matched: Vec<&String> = self.pins.iter().filter(|(pin, (_, stream, _))| *pin == entry || stream.name == *entry).map(|(pin, _)| pin).collect();
            if matched.is_empty() {
                let message = format!("`{entry}` names no record stream of the design: the streams are {}", self.paths().join(", "));
                return Err(Failure::Host { code: "E0801", message });
            }
            kept.extend(matched.into_iter().cloned());
        }
        self.kept = Some(kept);
        Ok(())
    }

    /// The record `o` holds; `None` when `o` is no record. A record of a pin no stream has, of a
    /// site that is no `emit!` of that stream, or of bytes that are not its layout's size is a
    /// `Failure`, never a guessed or truncated record.
    pub fn decode(&self, o: &Observation) -> Option<Result<Record, Failure>> {
        if let ObservationKind::Trace { iteration, bytes } = &o.kind {
            return self.trace(o, *iteration, bytes);
        }
        let ObservationKind::Record { site, bytes } = &o.kind else { return None };
        if self.kept.as_ref().is_some_and(|kept| !kept.contains(&o.path)) {
            return None;
        }
        Some(self.record(o, *site, bytes).map(|record| Record { analysis: Some(o.analysis), ..record }))
    }

    /// The record of a `TRACE` write `o` (Newton iteration `iteration`): the site's stream and
    /// fields, read through the watch's layout (a presence byte, then the stream's record). `None`
    /// unless the run names the stream (`streams`): a trace is a debugging surface the host asks for
    /// (Q5); a trace of a site the design lacks, or of the wrong size, is a `Failure`.
    fn trace(&self, o: &Observation, iteration: u32, bytes: &[u8]) -> Option<Result<Record, Failure>> {
        let (instance, site, stream_name) = self.traces.get(&o.path)?;
        let stream_pin = format!("{instance}.{stream_name}");
        // a trace is delivered only for a stream the run names
        if !self.kept.as_ref().is_some_and(|kept| kept.contains(&stream_pin)) {
            return None;
        }
        Some(self.trace_record(o, iteration, instance, *site, &stream_pin, bytes))
    }

    fn trace_record(&self, o: &Observation, iteration: u32, instance: &str, site: u32, stream_pin: &str, bytes: &[u8]) -> Result<Record, Failure> {
        let refused = |why: String| Failure::Backend(format!("a trace on `{}` at {}: {why}", o.path, o.time));
        let (_, stream, layout) = self.pins.get(stream_pin).ok_or_else(|| refused("no stream of the design has that path".into()))?;
        let start = layout.trace_start() as usize;
        if bytes.len() != layout.trace_size() as usize || bytes.first().is_none_or(|present| *present == 0) {
            return Err(refused(format!("{} bytes where the watch's layout holds {} and a set presence byte", bytes.len(), layout.trace_size())));
        }
        let record = &bytes[start..];
        let body = self.ir.instance_at(instance).ok_or_else(|| refused("no instance of the design has that path".into()))?.body();
        let value = |field: u32| -> Result<(String, Field), Failure> {
            let f = stream.fields.get(field as usize).ok_or_else(|| refused(format!("no value field {field}")))?;
            let (offset, carrier) = &layout.fields[field as usize];
            let slice = &record[*offset as usize..(offset + carrier.size()) as usize];
            let cell = match CellShape::of(&self.ir, &f.ty) {
                Some(shape) if !matches!(shape, CellShape::Scalar(_)) => shape.decode(slice)?,
                _ => Cell::decode(&self.ir.carried(&f.ty), slice)?,
            };
            Ok((f.name.clone(), Field::of(cell)?))
        };
        let mut fields = (0..stream.fields.len() as u32).map(value).collect::<Result<Vec<_>, Failure>>()?;
        fields.extend(body.trace_texts(site).into_iter().map(|(name, text)| (name.to_string(), Field::Text(text.to_string()))));
        Ok(Record { time: o.time, instance: instance.to_string(), stream: stream.name.clone(), fields, iteration: Some(iteration), analysis: Some(o.analysis) })
    }

    /// The instant of `o` when it is a `sim` record whose `finish` is true, whether or not the run
    /// keeps the `sim` stream: a model's `finish()` ends the run whatever streams the host asked for.
    pub fn finish_of(&self, o: &Observation) -> Option<f64> {
        let ObservationKind::Record { site, bytes } = &o.kind else { return None };
        let (_, stream, _) = self.pins.get(&o.path)?;
        if stream.name != "sim" {
            return None;
        }
        let record = self.record(o, *site, bytes).ok()?;
        record.holds("finish").then_some(record.time)
    }

    fn record(&self, o: &Observation, site: Option<(u32, u32)>, bytes: &[u8]) -> Result<Record, Failure> {
        let refused = |why: String| Failure::Backend(format!("a record on `{}` at {}: {why}", o.path, o.time));
        let (instance, stream, layout) = self.pins.get(&o.path).ok_or_else(|| refused("no stream of the design has that path".into()))?;
        if bytes.len() != layout.size as usize {
            return Err(refused(format!("{} bytes where the stream's layout holds {}", bytes.len(), layout.size)));
        }
        let value = |field: u32| -> Result<(String, Field), Failure> {
            let f = stream.fields.get(field as usize).ok_or_else(|| refused(format!("no value field {field}")))?;
            let (offset, carrier) = &layout.fields[field as usize];
            let slice = &bytes[*offset as usize..(offset + carrier.size()) as usize];
            let cell = match CellShape::of(&self.ir, &f.ty) {
                Some(shape) if !matches!(shape, CellShape::Scalar(_)) => shape.decode(slice)?,
                _ => Cell::decode(&self.ir.carried(&f.ty), slice)?,
            };
            Ok((f.name.clone(), Field::of(cell)?))
        };
        let fields = match site {
            Some((reaction, statement)) => {
                let (inst, _) = self.ir.record_at(&o.path, reaction, statement).ok_or_else(|| {
                    refused(format!("statement {statement} of reaction {reaction} is no `emit!` of `{}`", stream.name))
                })?;
                let id = inst.body().reaction_statements(reaction)[statement as usize];
                let Stmt::Emit { fields, .. } = self.ir.stmt(id) else { return Err(refused("the site is no `emit!`".into())) };
                fields
                    .iter()
                    .map(|f| match f {
                        EmitField::Value { field, .. } => value(*field),
                        EmitField::Text { name, text } => Ok((name.clone(), Field::Text(text.clone()))),
                    })
                    .collect::<Result<Vec<_>, Failure>>()?
            }
            None => (0..stream.fields.len() as u32).map(value).collect::<Result<Vec<_>, Failure>>()?,
        };
        Ok(Record { time: o.time, instance: instance.clone(), stream: stream.name.clone(), fields, iteration: None, analysis: None })
    }
}

impl Record {
    /// Whether this is a passing check: a record of the reserved `checks` stream whose `pass`
    /// is true. A failing check and a `target` (which has no `pass`) are not.
    pub fn is_passing_check(&self) -> bool {
        self.stream == "checks" && self.fields.iter().any(|(name, field)| name == "pass" && *field == Field::Bool(true))
    }
}

impl Field {
    fn of(cell: Cell) -> Result<Field, Failure> {
        Ok(match cell {
            Cell::Real(v) => Field::Real(v),
            Cell::Bool(v) => Field::Bool(v),
            Cell::Int(v) => Field::Int(v),
            Cell::Float(v) => Field::Float(v),
            Cell::Wide(v) => Field::Wide(v),
            Cell::Text(v) => Field::Variant(v),
            Cell::Composite(v) => Field::Composite(v),
            Cell::Absent => return Err(Failure::Backend("a record field decoded from its bytes is a value, never the absent cell".into())),
        })
    }
}

/// The solver's sink of a session's run: each record decoded and handed to the host's sink as
/// it is delivered, each change of a net to the recorder of the run's table; the first record
/// that cannot be decoded is kept for the run to report.
pub(crate) struct Delivery {
    pub(crate) streams: Streams,
    pub(crate) sink: Rc<RefCell<dyn RecordSink>>,
    pub(crate) failure: Rc<RefCell<Option<Failure>>>,
    /// The recorder of a transient's rows, which holds the nets as they change.
    pub(crate) recorder: Option<Rc<RefCell<Recorder>>>,
    /// The instant of the last `sim` record whose `finish` is known true: where the run ended
    /// by its own model's decision (msl-fixes FIX-02).
    pub(crate) finished: Rc<RefCell<Option<f64>>>,
}

impl Sink for Delivery {
    fn observe(&mut self, o: &Observation) {
        // a change with bytes is a net taking a value; one with none is an event firing
        if let (Some(recorder), ObservationKind::Change { bytes }) = (&self.recorder, &o.kind) {
            if !bytes.is_empty() {
                recorder.borrow_mut().change(&o.path, o.time, bytes);
            }
        }
        if let Some(at) = self.streams.finish_of(o) {
            *self.finished.borrow_mut() = Some(at);
        }
        match self.streams.decode(o) {
            Some(Ok(record)) => self.sink.borrow_mut().record(&record),
            Some(Err(failure)) => {
                self.failure.borrow_mut().get_or_insert(failure);
            }
            None => {}
        }
    }
}
