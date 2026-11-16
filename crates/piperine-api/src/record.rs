//! `Record`: an `emit!` record as the host receives it; `Streams`, which decodes the solver's
//! record observations into records; and `RecordSink`, where a session hands each one over as
//! the solver delivers it (CLI-06; contract D1, D2, D7).
//!
//! A record names its site: `ObservationKind::Record { site: Some((reaction, statement)), bytes }`
//! on the pin path `<instance path>.<stream>`. `Ir::record_at` gives the instance and the site;
//! the site's `emit!` statement gives the fields in source order; each value field is read from
//! the bytes through the stream's `RecordLayout` (a carrier of the field's type at its offset,
//! its known byte beside it), each text field is the site's literal, which never crosses the
//! ABI. A record is never paired with a commit by order. A record with no site (a relaxed field
//! that moved, abi.md §9 D1) has every value field of its stream and no text field.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use piperine_ir::{EmitField, Ir, Stmt, Stream};
use piperine_plan::RecordLayout;
use piperine_solver::protocol::{Observation, ObservationKind, Sink};

use crate::failure::Failure;
use crate::table::Cell;

/// One `emit!` record: when, which instance's site, which stream, and its fields in source
/// order.
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub time: f64,
    pub instance: String,
    pub stream: String,
    pub fields: Vec<(String, Field)>,
}

/// The value of one field of a record.
#[derive(Debug, Clone, PartialEq)]
pub enum Field {
    Real(f64),
    Bool(bool),
    Int(i128),
    /// A text literal of the site.
    Text(String),
    /// A value whose known byte is clear.
    Unknown,
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
}

impl Streams {
    /// Every stream of every instance of `ir`.
    pub fn of(ir: Arc<Ir>) -> Streams {
        let mut pins = HashMap::new();
        for instance in ir.instances() {
            for stream in instance.body().streams() {
                // `check()` refuses a stream field no carrier holds, so every checked stream has
                // a layout
                if let Some(layout) = RecordLayout::of(stream) {
                    pins.insert(format!("{}.{}", instance.path(), stream.name), (instance.path().to_string(), stream.clone(), layout));
                }
            }
        }
        Streams { ir, pins }
    }

    /// The record `o` holds; `None` when `o` is no record. A record of a pin no stream has, of a
    /// site that is no `emit!` of that stream, or of bytes that are not its layout's size is a
    /// `Failure`, never a guessed or truncated record.
    pub fn decode(&self, o: &Observation) -> Option<Result<Record, Failure>> {
        let ObservationKind::Record { site, bytes } = &o.kind else { return None };
        Some(self.record(o, *site, bytes))
    }

    fn record(&self, o: &Observation, site: Option<(u32, u32)>, bytes: &[u8]) -> Result<Record, Failure> {
        let refused = |why: String| Failure::Backend(format!("a record on `{}` at {}: {why}", o.path, o.time));
        let (instance, stream, layout) = self.pins.get(&o.path).ok_or_else(|| refused("no stream of the design has that path".into()))?;
        if bytes.len() != layout.size as usize {
            return Err(refused(format!("{} bytes where the stream's layout holds {}", bytes.len(), layout.size)));
        }
        let value = |field: u32| -> Result<(String, Field), Failure> {
            let f = stream.fields.get(field as usize).ok_or_else(|| refused(format!("no value field {field}")))?;
            let (offset, carrier) = layout.fields[field as usize];
            let slice = &bytes[offset as usize..(offset + carrier.size) as usize];
            Ok((f.name.clone(), Field::of(Cell::decode(&f.ty, slice)?)))
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
        Ok(Record { time: o.time, instance: instance.clone(), stream: stream.name.clone(), fields })
    }
}

impl Field {
    fn of(cell: Cell) -> Field {
        match cell {
            Cell::Real(v) => Field::Real(v),
            Cell::Bool(v) => Field::Bool(v),
            Cell::Int(v) => Field::Int(v),
            Cell::Text(v) => Field::Text(v),
            Cell::Unknown => Field::Unknown,
        }
    }
}

/// The solver's sink of a session's run: each record decoded and handed to the host's sink as
/// it is delivered; the first record that cannot be decoded is kept for the run to report.
pub(crate) struct Delivery {
    pub(crate) streams: Streams,
    pub(crate) sink: Rc<RefCell<dyn RecordSink>>,
    pub(crate) failure: Rc<RefCell<Option<Failure>>>,
}

impl Sink for Delivery {
    fn observe(&mut self, o: &Observation) {
        match self.streams.decode(o) {
            Some(Ok(record)) => self.sink.borrow_mut().record(&record),
            Some(Err(failure)) => {
                self.failure.borrow_mut().get_or_insert(failure);
            }
            None => {}
        }
    }
}
