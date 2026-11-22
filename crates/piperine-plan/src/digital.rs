//! The digital half of a body's plan (spec RS-12, design "Phase B: from a body to a shape"):
//! what the discrete forms of the IR become in the ABI. Everything here is appended after the
//! M1 entries of `plan.rs`, so an M1 body's pins, functions and their indices do not move.
//!
//! Carriers (RS-12 AC 1): one `DISCRETE` pin per carrier, in declaration order, after every M1
//! pin; `PORT` for an `in` or `out` port and for a `signal` a child's carrier port is bound to
//! (`Body::child_carriers`: part II §5, the parent's signal is the net its children meet on, so
//! the netlist binds it to that net like a port; msl-digital T65), `PRIVATE` for any other
//! `signal`. Its layout is one field `v`
//! with a known byte (abi.md P3): `BIT` for `bool`, `REAL` for `f64`, and for `UInt<N>`/`Int<N>`
//! the narrowest of `U8` to `U64` or `I8` to `I64` that holds `N` bits, the known byte right
//! after the value and the size rounded up to the value's alignment, as `BIT`, `BYTE` and
//! `REAL` have it. An initial value gives `HAS_INIT` with its canonical bytes: the value
//! little-endian, the known byte 1, every padding byte 0. An integer param takes the same
//! layout and default bytes as a carrier of its type.
//!
//! Events (RS-12 AC 3): one `PRIVATE` `EVENT` pin `event:<k>` per event of the body, after the
//! carrier pins. A `Changed` event is written by the discrete function `event:<k>`, which reads
//! the `VALUE` and `CHANGED` of its carrier and the pins its guard reads.
//!
//! Drives (RS-12 AC 2, AC 5): a drive without `after` is the discrete function
//! `drive:<carrier>` reading the pins its value reads and writing the carrier's pin. A drive
//! with `after` writes, instead, the `PRIVATE` pin `drive:<carrier>.now` (the carrier's layout);
//! a `changed` event on it, the event pin and function `drive:<carrier>.changed`, fires the
//! reaction `drive:<carrier>`, which reads the undelayed value and what the delay reads and
//! commits the value to the carrier after the delay. Those two pins follow the event pins, a
//! drive's pair in drive order.
//!
//! Reactions (RS-12 AC 4, AC 8): one ABI reaction `reaction:<k>` per reaction of the body, on
//! the event pins of its events, reading the `VALUE` of every pin its body reads (carriers,
//! params, and the continuous pins it probes), with one statement per commit in source order
//! (a depth-first walk of its statements, `then` before `else`). A statement commits the whole
//! of its carrier's pin. It is `LATER` with `min_after` the delay only when the delay is a real
//! literal greater than 0 (abi.md §6 C1: a bound the generator can state); a delay that reads
//! anything, or none, gives no flag. The reaction of a delayed drive follows the same rule.
//!
//! Crossings (RS-12 AC 6, abi.md §11.5): a `cross` event is a `CROSS` watch `cross:<k>`
//! (one `F64`, default tolerances) firing the event's pin, its value written by the analog
//! function `cross:<k>` with a partial against each continuous pin of its dependency set. A
//! guarded `cross` fires a private event pin `cross:<k>` instead (after the event pins), and the
//! event function `event:<k>` reads its `CHANGED` and the guard's pins and fires the event pin.
//!
//! An analog read of a carrier (RS-12 AC 7) is in the dependency set of the term that reads it:
//! `terms` reads the carrier's `VALUE`, with no partial (it is no differentiable read).
//!
//! Timers (msl-digital A3; abi.md R1: "a timer is a delayed commit to an event pin"): an `at(t)`
//! event `k` is fired by its own **arming reaction** `at:<k>`, which listens to the `GLOBAL`
//! `START` pin (one per body, `start`) and to one private event pin `at:<k>.<carrier>` per
//! carrier `t` reads, each fired by the discrete function of the same name when that carrier
//! changed. The reaction reads what `t` reads and the ambient `TIME`, and has one `REPLACE`
//! statement on the event pin (C3: a commit through it removes every pending one, so a re-armed
//! timer drops its earlier firing). A guarded `at` fires a private event pin `at:<k>` instead,
//! and its event function `event:<k>` reads that pin's `CHANGED` and the guard's pins, as a
//! guarded `cross` does. These pins follow the delayed pins: the timer pins in event order, the
//! `START` pin, then the re-arming pins in event and read order.
//!
//! `settle` (unknown-semantics-events, §13 rule 17): a body with a `settle` event has one
//! `GLOBAL` `SETTLE` pin (`settle`, after every other pin), and the event function `event:<k>` of
//! each `settle` event reads its `CHANGED` byte (`Source::Settle`) and the guard's pins, as a
//! `changed` event reads its carrier's.
//!
//! Records (msl-digital A4; abi.md P7, C5): each stream of a body is one `PRIVATE` `DISCRETE`
//! pin named by the stream and flagged `RECORD` (so `QUIET`: no reader, never a microstep). Its
//! layout holds one field per value field of the stream, in the stream's order, each laid out
//! as a carrier of its type (the value, then its known byte) at the next offset its alignment
//! allows, then the field `maybe` of §19 rule 1 (a `bool`, laid out the same way), the size
//! rounded up to the largest alignment (`RecordLayout`): a site that leaves a
//! field out writes it all zero, its known byte 0 (P3, C5). Each emit site is one statement of
//! its reaction, in the order of the commits (`Body::reaction_statements`), committing the
//! whole layout. The record pins follow the re-arming pins, in stream order.
//!
//! Function order after the M1 functions: `cross:*` in event order, `drive:*` in drive order,
//! then `event:*` in event order, then each `drive:<carrier>.changed`, then each
//! `at:<k>.<carrier>`. Reactions: the body's in source order, then one per delayed drive in
//! drive order, then one arming reaction per `at` event in event order.

use piperine_abi::layout::{BIT, BYTE, REAL, leaf_size};
use piperine_abi::layout::F64;
use piperine_abi::raw::{
    DEFAULT, Field, GROUND, Layout, List, Statement, Str, field_flag, global, leaf, pin_flag, role, statement_flag, watch,
};
use piperine_abi::shape::{PinSpec, ReactionSpec, ShapeBuf, WatchSpec};
use piperine_ir::{Body, CarrierRole, Constant, EventSource, Expr, ExprId, Ir, Origin, Stmt, Stream, Target as Query, ValueType};

use crate::pin::PinLayout;
use crate::plan::{BodyPlan, DelayedPins, FunctionPlan, Mode, Pins, ReadSet, Source, Target};

/// What a reaction of the shape runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReactionMode {
    /// Reaction `k` of the body.
    Reaction(u32),
    /// The commit of drive `k`'s undelayed value after its delay (RS-12 AC 5).
    Delayed(u32),
    /// The arming of `at` event `k`: a commit to its pin due at its time (msl-digital A3).
    Arm(u32),
}

/// What one reaction of the shape does, parallel to the shape's reaction.
#[derive(Clone, Debug, PartialEq)]
pub struct ReactionPlan {
    pub mode: ReactionMode,
    /// The event pins it listens to, parallel to the reaction's `on`.
    pub on: Vec<u32>,
    /// Parallel to the reaction's reads, each a `VALUE` of a pin.
    pub reads: Vec<Source>,
    /// Parallel to the reaction's statements.
    pub statements: Vec<StatementPlan>,
}

/// One statement of a reaction: the pin it commits to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StatementPlan {
    /// `Target::Carrier` for a commit; `Target::EventPin` or `Target::Timer` for the statement
    /// of an arming reaction.
    pub target: Target,
    /// `Some`: the statement is `LATER` with this `min_after`, in seconds.
    pub min_after: Option<f64>,
    /// The statement is `REPLACE` (abi.md C3): a commit through it removes every pending one.
    pub replace: bool,
}

impl StatementPlan {
    /// The statement of a commit to `carrier` delayed by `after`.
    pub fn commit(ir: &Ir, carrier: u32, after: Option<ExprId>) -> StatementPlan {
        StatementPlan { target: Target::Carrier(carrier), min_after: after.and_then(|a| Self::min_after(ir, a)), replace: false }
    }

    /// The statement of an emit site of stream `s`: the whole of its `RECORD` pin, due at the
    /// next microstep.
    pub fn record(s: u32) -> StatementPlan {
        StatementPlan { target: Target::Record(s), min_after: None, replace: false }
    }

    /// The statements of reaction `k` of `body`, one per commit and per emit site, in ABI order.
    pub fn of_reaction(body: Body<'_>, k: u32) -> Vec<StatementPlan> {
        let ir = body.ir();
        body.reaction_statements(k)
            .into_iter()
            .filter_map(|id| match ir.stmt(id) {
                Stmt::Commit { carrier, after, .. } => Some(StatementPlan::commit(ir, *carrier, *after)),
                Stmt::Emit { stream, .. } => Some(StatementPlan::record(*stream)),
                Stmt::Let { .. } | Stmt::Expr(_) | Stmt::Assign { .. } | Stmt::Contribute { .. } | Stmt::BoundStep { .. } => None,
            })
            .collect()
    }

    /// The statement of the arming reaction of `at` event `k` of `body`: `REPLACE` on the pin the
    /// timer fires.
    pub fn arm(body: Body<'_>, k: u32) -> StatementPlan {
        let guarded = body.events().get(k as usize).is_some_and(|e| e.guard.is_some());
        StatementPlan { target: if guarded { Target::Timer(k) } else { Target::EventPin(k) }, min_after: None, replace: true }
    }

    /// The `min_after` of a commit delayed by `after`: the delay itself when it is a real
    /// literal greater than 0, else none (abi.md §6 C1).
    pub fn min_after(ir: &Ir, after: ExprId) -> Option<f64> {
        match ir.expr(after) {
            Expr::Real(r) if r.get() > 0.0 => Some(r.get()),
            _ => None,
        }
    }
}

/// The layout of a discrete value of a given IR type, and its bytes.
pub struct CarrierLayout;

static U16_FIELDS: [Field<'static>; 1] = [CarrierLayout::field(leaf::U16, 2)];
static U32_FIELDS: [Field<'static>; 1] = [CarrierLayout::field(leaf::U32, 4)];
static U64_FIELDS: [Field<'static>; 1] = [CarrierLayout::field(leaf::U64, 8)];
static I8_FIELDS: [Field<'static>; 1] = [CarrierLayout::field(leaf::I8, 1)];
static I16_FIELDS: [Field<'static>; 1] = [CarrierLayout::field(leaf::I16, 2)];
static I32_FIELDS: [Field<'static>; 1] = [CarrierLayout::field(leaf::I32, 4)];
static I64_FIELDS: [Field<'static>; 1] = [CarrierLayout::field(leaf::I64, 8)];

/// `{ v: U16 @0, known @2 }`, size 4, align 2.
static U16: Layout<'static> = Layout { size: 4, align: 2, fields: List::from_slice(&U16_FIELDS) };
/// `{ v: U32 @0, known @4 }`, size 8, align 4.
static U32: Layout<'static> = Layout { size: 8, align: 4, fields: List::from_slice(&U32_FIELDS) };
/// `{ v: U64 @0, known @8 }`, size 16, align 8.
static U64: Layout<'static> = Layout { size: 16, align: 8, fields: List::from_slice(&U64_FIELDS) };
/// `{ v: I8 @0, known @1 }`, size 2, align 1.
static I8: Layout<'static> = Layout { size: 2, align: 1, fields: List::from_slice(&I8_FIELDS) };
/// `{ v: I16 @0, known @2 }`, size 4, align 2.
static I16: Layout<'static> = Layout { size: 4, align: 2, fields: List::from_slice(&I16_FIELDS) };
/// `{ v: I32 @0, known @4 }`, size 8, align 4.
static I32: Layout<'static> = Layout { size: 8, align: 4, fields: List::from_slice(&I32_FIELDS) };
/// `{ v: I64 @0, known @8 }`, size 16, align 8.
static I64: Layout<'static> = Layout { size: 16, align: 8, fields: List::from_slice(&I64_FIELDS) };

impl CarrierLayout {
    /// The one field `v` of a layout: `leaf` at offset 0, its known byte at `known`.
    const fn field(leaf: u32, known: u32) -> Field<'static> {
        Field { name: Str::from_static("v"), leaf, offset: 0, count: 1, flags: field_flag::HAS_KNOWN, known }
    }

    /// The width in bytes of the value of an `N`-bit integer: the narrowest of 1, 2, 4 and 8.
    fn width(n: u32) -> u32 {
        match n {
            0..=8 => 1,
            9..=16 => 2,
            17..=32 => 4,
            _ => 8,
        }
    }

    /// The layout of a carrier (or a discrete param) holding `ty`; `None` for a type no carrier
    /// holds (`check()` refuses it on a carrier).
    pub fn of(ty: &ValueType) -> Option<&'static Layout<'static>> {
        match *ty {
            ValueType::Bool => Some(&BIT),
            ValueType::F64 => Some(&REAL),
            ValueType::UInt(n) => Some(match Self::width(n) {
                1 => &BYTE,
                2 => &U16,
                4 => &U32,
                _ => &U64,
            }),
            ValueType::Int(n) => Some(match Self::width(n) {
                1 => &I8,
                2 => &I16,
                4 => &I32,
                _ => &I64,
            }),
            _ => None,
        }
    }

    /// The layout of a carrier holding `ty` in the design `ir`: [`of`](Self::of) the scalar the pin holds
    /// for it (`Ir::carried`: an enum without fields is its tag).
    pub fn in_ir(ir: &Ir, ty: &ValueType) -> Option<&'static Layout<'static>> {
        Self::of(&ir.carried(ty))
    }

    /// [`bytes`](Self::bytes) of a value of `ty` in the design `ir`: an enum constant is its
    /// discriminant held as the tag.
    pub fn bytes_in(ir: &Ir, ty: &ValueType, value: Constant) -> Option<Vec<u8>> {
        match ir.discriminant(ty, value) {
            Some(tag) => Self::bytes(&ir.carried(ty), Constant::Int(tag)),
            None => Self::bytes(ty, value),
        }
    }

    /// The canonical bytes of `value` held as `ty` (abi.md P3): the value little-endian at
    /// offset 0, the known byte 1, padding 0. `None` when `value` is not a value of `ty`.
    pub fn bytes(ty: &ValueType, value: Constant) -> Option<Vec<u8>> {
        let layout = Self::of(ty)?;
        let field = layout.fields.get(0)?;
        let value: Vec<u8> = match (ty, value) {
            (ValueType::Bool, Constant::Bool(b)) => vec![u8::from(b)],
            (ValueType::F64, Constant::Real(r)) => r.get().to_le_bytes().to_vec(),
            (ValueType::UInt(_) | ValueType::Int(_), Constant::Int(v)) => {
                let (lo, hi) = ty.integer_range()?;
                if v < lo || v > hi {
                    return None;
                }
                v.to_le_bytes()[..leaf_size(field.leaf)?].to_vec()
            }
            _ => return None,
        };
        let mut bytes = vec![0u8; layout.size as usize];
        bytes[..value.len()].copy_from_slice(&value);
        bytes[field.known as usize] = 1;
        Some(bytes)
    }
}

/// The layout of a stream's `RECORD` pin: each value field laid out as a pin of its type
/// (`PinLayout`: a carrier's scalar layout, or a composite's), in the stream's order, at the
/// next offset its alignment allows, then the `maybe` field (§19 rule 1: whether the record was
/// reached under unknown control), a `bool` laid out as a carrier; the size rounded up to the
/// largest alignment.
#[derive(Clone, Debug)]
pub struct RecordLayout {
    pub size: u32,
    pub align: u32,
    /// Per value field of the stream: its offset and the layout it is laid out as. The `maybe`
    /// field is not among them: it is no field of the stream (`maybe_offset`).
    pub fields: Vec<(u32, PinLayout)>,
    /// The offset of the `maybe` field.
    maybe: u32,
    /// Every leaf of the layout as an ABI field, the `maybe` field last.
    leaves: Vec<RecordLeaf>,
}

/// One ABI field of a record layout: a scalar leaf of a value field, named by the field.
#[derive(Clone, Debug)]
struct RecordLeaf {
    name: String,
    leaf: u32,
    offset: u32,
    known: u32,
}

impl RecordLayout {
    /// The layout of `stream`'s pin; `None` when a field holds a type no carrier holds
    /// (`check()` refuses it). A composite field needs the type arena: use [`in_ir`](Self::in_ir).
    pub fn of(stream: &Stream) -> Option<RecordLayout> {
        Self::laid_out(stream, |ty| CarrierLayout::of(ty).map(PinLayout::Scalar))
    }

    /// [`of`](Self::of) in the design `ir`: a field holding an enum without fields is laid out as
    /// its tag, a composite field as its leaves.
    pub fn in_ir(ir: &Ir, stream: &Stream) -> Option<RecordLayout> {
        Self::laid_out(stream, |ty| PinLayout::in_ir(ir, ty))
    }

    /// The layout of `stream`'s pin with each field laid out as `pin` gives for its type.
    fn laid_out(stream: &Stream, pin: impl Fn(&ValueType) -> Option<PinLayout>) -> Option<RecordLayout> {
        let (mut end, mut align) = (0u32, 1u32);
        let mut fields = Vec::with_capacity(stream.fields.len());
        let mut leaves = Vec::new();
        for f in &stream.fields {
            let layout = pin(&f.ty)?;
            let offset = end.next_multiple_of(layout.align());
            leaves.extend(Self::leaves_of(&f.name, offset, &layout)?);
            end = offset + layout.size();
            align = align.max(layout.align());
            fields.push((offset, layout));
        }
        let maybe_layout = pin(&ValueType::Bool)?;
        let maybe = end.next_multiple_of(maybe_layout.align());
        leaves.extend(Self::leaves_of("maybe", maybe, &maybe_layout)?);
        end = maybe + maybe_layout.size();
        align = align.max(maybe_layout.align());
        Some(RecordLayout { size: end.next_multiple_of(align), align, fields, maybe, leaves })
    }

    /// The ABI fields of the value field `name` laid out as `layout` at `offset`: the leaf of a
    /// scalar is named by the field, a composite's leaves by the field and their own path.
    fn leaves_of(name: &str, offset: u32, layout: &PinLayout) -> Option<Vec<RecordLeaf>> {
        match layout {
            PinLayout::Scalar(scalar) => {
                let v = scalar.fields.get(0)?;
                Some(vec![RecordLeaf { name: name.to_string(), leaf: v.leaf, offset, known: offset + v.known }])
            }
            PinLayout::Composite(composite) => Some(
                composite
                    .leaves()
                    .iter()
                    .map(|l| RecordLeaf {
                        name: if l.name.is_empty() { name.to_string() } else if l.name.starts_with('[') { format!("{name}{}", l.name) } else { format!("{name}.{}", l.name) },
                        leaf: l.leaf,
                        offset: offset + l.offset,
                        known: offset + composite.known_offset(),
                    })
                    .collect(),
            ),
        }
    }

    /// The offset of the `maybe` field: a `bool` carrier (its value byte, then its known byte)
    /// after the stream's value fields.
    pub fn maybe_offset(&self) -> u32 {
        self.maybe
    }

    /// The ABI fields of the layout: each value field's leaves, named after the stream's fields,
    /// then `maybe` (§19 rule 1).
    fn abi_fields(&self) -> Vec<Field<'_>> {
        self.leaves
            .iter()
            .map(|l| Field { name: Str::from_str(&l.name), leaf: l.leaf, offset: l.offset, count: 1, flags: field_flag::HAS_KNOWN, known: l.known })
            .collect()
    }
}

/// The discrete entries of a body's shape.
pub(crate) struct Digital;

impl Digital {
    /// Adds one pin per carrier of `body`, in declaration order; returns the index of the first
    /// (the index the next pin would get when the body has none). The body passes `check()`, so
    /// every carrier holds `bool`, `f64` or an integer type.
    pub(crate) fn carrier_pins(s: &mut ShapeBuf, body: Body<'_>) -> u32 {
        let first = s.shape().pins.len() as u32;
        for (i, c) in body.carriers().iter().enumerate() {
            let at = format!("{}:{}:{}", c.origin.file, c.origin.line, c.origin.col);
            let Some(layout) = PinLayout::in_ir(body.ir(), &c.ty) else {
                panic!("body {}: carrier {} at {at} holds {}, which no pin holds; check() refuses it", body.name(), c.name, c.ty)
            };
            let init = match c.init {
                Some(value) => PinLayout::bytes_in(body.ir(), &c.ty, value).unwrap_or_else(|| {
                    panic!("body {}: carrier {} at {at}: initial value {value:?} is not a value of {}; check() refuses it", body.name(), c.name, c.ty)
                }),
                None => Vec::new(),
            };
            let pin_role = if Self::is_port(body, i as u32) { role::PORT } else { role::PRIVATE };
            let flags = if init.is_empty() { 0 } else { pin_flag::HAS_INIT };
            let fields = layout.fields();
            let abi = layout.layout(&fields);
            let pin = s.pin(PinSpec { origin: &at, flags, init: &init, ..PinSpec::discrete(&c.name, pin_role, &abi) });
            debug_assert_eq!(pin, first + i as u32);
        }
        first
    }

    /// Whether carrier `c` of `body` is a `PORT` pin: an `in` or `out` port, or a `signal` that a
    /// child's carrier port is bound to.
    pub(crate) fn is_port(body: Body<'_>, c: u32) -> bool {
        match body.carriers().get(c as usize).map(|x| x.role) {
            // A reached carrier is a read-only view of a child's: a `PORT` pin, as an `in` port.
            Some(CarrierRole::In | CarrierRole::Out | CarrierRole::Reached) => true,
            // A `signal` is met on a net when a child's carrier port is bound to it or a parent reads it (`inst.name`).
            Some(CarrierRole::Signal) => (0..body.children().len()).any(|k| body.child_carriers(k).contains(&Some(c))) || body.is_reached(c),
            None => false,
        }
    }

    /// `origin` as a shape writes it.
    fn at(o: &Origin) -> String {
        format!("{}:{}:{}", o.file, o.line, o.col)
    }

    /// The name of carrier `c` of `body`.
    fn carrier_name(body: Body<'_>, c: u32) -> String {
        body.carriers().get(c as usize).map_or_else(|| format!("carrier{c}"), |x| x.name.clone())
    }

    /// Adds one event pin `event:<k>` per event of `body`; returns their indices.
    pub(crate) fn event_pins(s: &mut ShapeBuf, body: Body<'_>) -> Vec<u32> {
        body.events()
            .iter()
            .enumerate()
            .map(|(k, e)| s.pin(PinSpec { origin: &Self::at(&e.origin), ..PinSpec::event(&format!("event:{k}"), role::PRIVATE) }))
            .collect()
    }

    /// Adds the pin each guarded `cross` event's watch fires; `None` for every other event.
    pub(crate) fn crossed_pins(s: &mut ShapeBuf, body: Body<'_>) -> Vec<Option<u32>> {
        body.events()
            .iter()
            .enumerate()
            .map(|(k, e)| match (&e.source, e.guard) {
                (EventSource::Cross { .. }, Some(_)) => {
                    Some(s.pin(PinSpec { origin: &Self::at(&e.origin), ..PinSpec::event(&format!("cross:{k}"), role::PRIVATE) }))
                }
                _ => None,
            })
            .collect()
    }

    /// Adds one `CROSS` watch per `cross` event, firing the event's pin (or, guarded, its
    /// `cross:<k>` pin); returns each event's watch.
    pub(crate) fn watches(
        s: &mut ShapeBuf,
        body: Body<'_>,
        events: &[u32],
        crossed: &[Option<u32>],
        tolerances: &[(Option<u32>, Option<u32>)],
    ) -> Vec<Option<u32>> {
        body.events()
            .iter()
            .enumerate()
            .map(|(k, e)| {
                let EventSource::Cross { .. } = e.source else { return None };
                let event = crossed.get(k).copied().flatten().or_else(|| events.get(k).copied())?;
                Some(s.watch(WatchSpec {
                    name: &format!("cross:{k}"),
                    origin: &Self::at(&e.origin),
                    kind: watch::CROSS,
                    layout: &F64,
                    event,
                    ttol: tolerances.get(k).and_then(|t| t.0).unwrap_or(DEFAULT),
                    etol: tolerances.get(k).and_then(|t| t.1).unwrap_or(DEFAULT),
                }))
            })
            .collect()
    }

    /// Adds the private `F64` pins `ttol:<k>` and `etol:<k>` of each `cross` event that gives a
    /// tolerance, written by a function of the parameter cone.
    pub(crate) fn tolerance_pins(s: &mut ShapeBuf, body: Body<'_>) -> Vec<(Option<u32>, Option<u32>)> {
        body.events()
            .iter()
            .enumerate()
            .map(|(k, e)| {
                let EventSource::Cross { ttol, etol, .. } = e.source else { return (None, None) };
                let mut pin = |given: Option<ExprId>, name: &str| {
                    given.map(|_| s.pin(PinSpec { origin: &Self::at(&e.origin), ..PinSpec::discrete(&format!("{name}:{k}"), role::PRIVATE, &F64) }))
                };
                (pin(ttol, "ttol"), pin(etol, "etol"))
            })
            .collect()
    }

    /// Adds the private `F64` pin `d<k>.abstol` of each derivative site that gives an `abstol`,
    /// written by a function of the parameter cone and named by the site's `DDT` term.
    pub(crate) fn derivative_abstol_pins(s: &mut ShapeBuf, body: Body<'_>) -> Vec<Option<u32>> {
        body.derivatives()
            .iter()
            .enumerate()
            .map(|(k, site)| {
                site.abstol.map(|_| s.pin(PinSpec { origin: &Self::at(&site.origin), ..PinSpec::discrete(&format!("d{k}.abstol"), role::PRIVATE, &F64) }))
            })
            .collect()
    }

    /// Adds one `CAP` watch per `bound_step` site of the `analog` block, `cap:<site>`; returns
    /// `(site, watch)`.
    pub(crate) fn cap_watches(s: &mut ShapeBuf, body: Body<'_>) -> Vec<(u32, u32)> {
        BodyPlan::bound_steps(body)
            .into_iter()
            .map(|site| {
                let watch_index = s.watch(WatchSpec {
                    name: &format!("cap:{site}"),
                    origin: &Self::at(body.origin()),
                    kind: watch::CAP,
                    layout: &F64,
                    event: DEFAULT,
                    ttol: DEFAULT,
                    etol: DEFAULT,
                });
                (site, watch_index)
            })
            .collect()
    }

    /// Adds the two pins of each drive with `after`; `None` for a drive without one.
    pub(crate) fn delayed_pins(s: &mut ShapeBuf, body: Body<'_>) -> Vec<Option<DelayedPins>> {
        body.drives()
            .iter()
            .map(|d| {
                d.after?;
                let name = Self::carrier_name(body, d.carrier);
                let layout = body.carriers().get(d.carrier as usize).and_then(|c| PinLayout::in_ir(body.ir(), &c.ty))?;
                let at = Self::at(&d.origin);
                let fields = layout.fields();
                let abi = layout.layout(&fields);
                let value = s.pin(PinSpec { origin: &at, ..PinSpec::discrete(&format!("drive:{name}.now"), role::PRIVATE, &abi) });
                let event = s.pin(PinSpec { origin: &at, ..PinSpec::event(&format!("drive:{name}.changed"), role::PRIVATE) });
                Some(DelayedPins { value, event })
            })
            .collect()
    }

    /// Adds the private event pin `at:<k>` the timer of each guarded `at` event fires; `None`
    /// for every other event.
    pub(crate) fn timer_pins(s: &mut ShapeBuf, body: Body<'_>) -> Vec<Option<u32>> {
        body.events()
            .iter()
            .enumerate()
            .map(|(k, e)| match (&e.source, e.guard) {
                (EventSource::At(_), Some(_)) => {
                    Some(s.pin(PinSpec { origin: &Self::at(&e.origin), ..PinSpec::event(&format!("at:{k}"), role::PRIVATE) }))
                }
                _ => None,
            })
            .collect()
    }

    /// Adds the `GLOBAL` `START` pin when `body` has an `at` event.
    pub(crate) fn start_pin(s: &mut ShapeBuf, body: Body<'_>) -> Option<u32> {
        body.events().iter().any(|e| matches!(e.source, EventSource::At(_))).then(|| {
            s.pin(PinSpec { origin: &Self::at(body.origin()), global: global::START, ..PinSpec::event("start", role::GLOBAL) })
        })
    }

    /// Adds the `GLOBAL` `SETTLE` pin when `body` has a `settle` event (§13 rule 17): the pin the
    /// engine fires in the settle phase (§15 step 4), read by the event function of each
    /// `settle` event (`Source::Settle`).
    pub(crate) fn settle_pin(s: &mut ShapeBuf, body: Body<'_>) -> Option<u32> {
        body.events().iter().any(|e| matches!(e.source, EventSource::Settle)).then(|| {
            s.pin(PinSpec { origin: &Self::at(body.origin()), global: global::SETTLE, ..PinSpec::event("settle", role::GLOBAL) })
        })
    }

    /// Adds, per `at` event, one event pin `at:<k>.<carrier>` per carrier its time reads.
    pub(crate) fn rearm_pins(s: &mut ShapeBuf, body: Body<'_>) -> Vec<Vec<(u32, u32)>> {
        body.events()
            .iter()
            .enumerate()
            .map(|(k, e)| {
                let EventSource::At(_) = e.source else { return Vec::new() };
                body.carrier_reads(Query::At(k as u32))
                    .into_iter()
                    .map(|c| {
                        let name = format!("at:{k}.{}", Self::carrier_name(body, c));
                        (c, s.pin(PinSpec { origin: &Self::at(&e.origin), ..PinSpec::event(&name, role::PRIVATE) }))
                    })
                    .collect()
            })
            .collect()
    }

    /// Adds one `PRIVATE` `RECORD` pin per stream of `body`, named by the stream; returns their
    /// indices. The body passes `check()`, so every field holds `bool`, `f64` or an integer.
    pub(crate) fn record_pins(s: &mut ShapeBuf, body: Body<'_>) -> Vec<u32> {
        let at = Self::at(body.origin());
        body.streams()
            .iter()
            .map(|stream| {
                let Some(record) = RecordLayout::in_ir(body.ir(), stream) else {
                    panic!("body {}: stream {} holds a field no pin holds; check() refuses it", body.name(), stream.name)
                };
                let fields = record.abi_fields();
                let layout = Layout { size: record.size, align: record.align, fields: List::from_slice(&fields) };
                s.pin(PinSpec { origin: &at, flags: pin_flag::RECORD, ..PinSpec::discrete(&stream.name, role::PRIVATE, &layout) })
            })
            .collect()
    }

    /// Declares the discrete functions of `body`: `drive:*`, `event:*`, `drive:*.changed`,
    /// `at:*.*`.
    pub(crate) fn functions(s: &mut ShapeBuf, pins: &Pins, body: Body<'_>) -> Vec<FunctionPlan> {
        let mut functions = Vec::new();
        let mut add = |s: &mut ShapeBuf, name: String, origin: &Origin, plan: FunctionPlan| {
            BodyPlan::declare(s, pins, &[], body, &name, &Self::at(origin), &plan);
            functions.push(plan);
        };
        for (k, e) in body.events().iter().enumerate() {
            let EventSource::Cross { .. } = e.source else { continue };
            let k = k as u32;
            let set = ReadSet::target(body, Query::Cross(k));
            add(s, format!("cross:{k}"), &e.origin, BodyPlan::function(Mode::Cross(k), &set, vec![(Target::Watch(k), Some(&set))]));
        }
        for (k, e) in body.events().iter().enumerate() {
            let k = k as u32;
            let EventSource::Cross { ttol, etol, .. } = e.source else { continue };
            if ttol.is_some() {
                let set = ReadSet::target(body, Query::Ttol(k));
                add(s, format!("ttol:{k}"), &e.origin, BodyPlan::function(Mode::Ttol(k), &set, vec![(Target::Ttol(k), None)]));
            }
            if etol.is_some() {
                let set = ReadSet::target(body, Query::Etol(k));
                add(s, format!("etol:{k}"), &e.origin, BodyPlan::function(Mode::Etol(k), &set, vec![(Target::Etol(k), None)]));
            }
        }
        for (k, d) in body.drives().iter().enumerate() {
            let k = k as u32;
            let set = ReadSet::target(body, Query::Drive(k));
            let write = if d.after.is_some() { Target::Delayed(k) } else { Target::Carrier(d.carrier) };
            let name = format!("drive:{}", Self::carrier_name(body, d.carrier));
            add(s, name, &d.origin, BodyPlan::function(Mode::Drive(k), &set, vec![(write, None)]));
        }
        for (k, e) in body.events().iter().enumerate() {
            let k = k as u32;
            let mut set = ReadSet::target(body, Query::Guard(k));
            match e.source {
                EventSource::Changed(c) => set.0.extend([Source::Carrier(c), Source::CarrierChanged(c)]),
                // `p.changed()`: the param pin's value and the byte the host's `set` stamps
                EventSource::ParamChanged(p) => set.0.extend([Source::Param(p), Source::ParamChanged(p)]),
                EventSource::Cross { .. } if e.guard.is_some() => {
                    set.0.insert(Source::Crossed(k));
                }
                EventSource::Cross { .. } => continue,
                // a guarded timer fires its own pin, which the event function gates
                EventSource::At(_) if e.guard.is_some() => {
                    set.0.insert(Source::Timer(k));
                }
                // an unguarded one is fired by its arming reaction's statement
                EventSource::At(_) => continue,
                // `settle` (§13 rule 17): the settle pin's `CHANGED` byte, gated by the guard
                EventSource::Settle => {
                    set.0.insert(Source::Settle);
                }
            }
            add(s, format!("event:{k}"), &e.origin, BodyPlan::function(Mode::Event(k), &set, vec![(Target::EventPin(k), None)]));
        }
        for (k, d) in body.drives().iter().enumerate() {
            if d.after.is_none() {
                continue;
            }
            let k = k as u32;
            let set = ReadSet([Source::Delayed(k), Source::DelayedChanged(k)].into_iter().collect());
            let name = format!("drive:{}.changed", Self::carrier_name(body, d.carrier));
            add(s, name, &d.origin, BodyPlan::function(Mode::DelayedEvent(k), &set, vec![(Target::DelayedEventPin(k), None)]));
        }
        for (k, e) in body.events().iter().enumerate() {
            let event = k as u32;
            for &(carrier, _) in pins.rearms.get(k).map_or(&[][..], Vec::as_slice) {
                let set = ReadSet([Source::CarrierChanged(carrier)].into_iter().collect());
                let name = format!("at:{k}.{}", Self::carrier_name(body, carrier));
                let plan = BodyPlan::function(Mode::Rearm { event, carrier }, &set, vec![(Target::Rearm { event, carrier }, None)]);
                add(s, name, &e.origin, plan);
            }
        }
        functions
    }

    /// What reaction `k` of `body` reads: its own dependency set.
    fn reaction_reads(body: Body<'_>, k: u32) -> ReadSet {
        ReadSet::target(body, Query::Reaction(k))
    }

    /// Declares the reactions of `body`: its own, then one per drive with `after`.
    pub(crate) fn reactions(s: &mut ShapeBuf, pins: &Pins, body: Body<'_>) -> Vec<ReactionPlan> {
        let ir = body.ir();
        let mut reactions = Vec::new();
        for (k, r) in body.reactions().iter().enumerate() {
            let plan = ReactionPlan {
                mode: ReactionMode::Reaction(k as u32),
                on: r.on.iter().map(|e| pins.events.get(*e as usize).copied().unwrap_or(u32::MAX)).collect(),
                // abi.md §4.2: a reaction reads no `FREQ`; the `freq()` of a function it calls is 0
                // (§14.4.4), which the printer binds
                reads: Self::reaction_reads(body, k as u32).0.into_iter().filter(|s| *s != Source::Freq).collect(),
                statements: StatementPlan::of_reaction(body, k as u32),
            };
            Self::declare(s, pins, body, &format!("reaction:{k}"), &Self::at(&r.origin), &plan);
            reactions.push(plan);
        }
        for (k, d) in body.drives().iter().enumerate() {
            let k = k as u32;
            let Some(delayed) = pins.delayed(k) else { continue };
            let mut set = ReadSet::target(body, Query::DriveAfter(k));
            set.0.insert(Source::Delayed(k));
            let plan = ReactionPlan {
                mode: ReactionMode::Delayed(k),
                on: vec![delayed.event],
                reads: set.0.into_iter().collect(),
                statements: vec![StatementPlan::commit(ir, d.carrier, d.after)],
            };
            let name = format!("drive:{}", Self::carrier_name(body, d.carrier));
            Self::declare(s, pins, body, &name, &Self::at(&d.origin), &plan);
            reactions.push(plan);
        }
        for (k, e) in body.events().iter().enumerate() {
            let EventSource::At(_) = e.source else { continue };
            let event = k as u32;
            let mut set = ReadSet::target(body, Query::At(event));
            set.0.insert(Source::Time);
            let mut on: Vec<u32> = pins.start.into_iter().collect();
            on.extend(pins.rearms.get(k).map_or(&[][..], Vec::as_slice).iter().map(|(_, pin)| *pin));
            let plan = ReactionPlan {
                mode: ReactionMode::Arm(event),
                on,
                reads: set.0.into_iter().collect(),
                statements: vec![StatementPlan::arm(body, event)],
            };
            Self::declare(s, pins, body, &format!("at:{k}"), &Self::at(&e.origin), &plan);
            reactions.push(plan);
        }
        reactions
    }

    /// Adds `plan` to the shape as reaction `name`: each statement commits the whole of its
    /// carrier's pin, or fires an event pin (`len` 0, abi.md C4).
    fn declare(s: &mut ShapeBuf, pins: &Pins, body: Body<'_>, name: &str, origin: &str, plan: &ReactionPlan) {
        let limits = body.limits().len() as u32;
        let writes = plan
            .statements
            .iter()
            .map(|st| {
                let (pin, len) = match st.target {
                    Target::Carrier(c) => {
                        (pins.carrier(c), body.carriers().get(c as usize).and_then(|x| PinLayout::in_ir(body.ir(), &x.ty)).map_or(0, |l| l.size()))
                    }
                    Target::EventPin(k) => (pins.events.get(k as usize).copied().unwrap_or(GROUND), 0),
                    Target::Timer(k) => (pins.timer(k), 0),
                    Target::Record(r) => {
                        (pins.record(r), body.streams().get(r as usize).and_then(|stream| RecordLayout::in_ir(body.ir(), stream)).map_or(0, |l| l.size))
                    }
                    _ => (GROUND, 0),
                };
                let (mut flags, min_after) = match st.min_after {
                    Some(m) => (statement_flag::LATER, m),
                    None => (0, 0.0),
                };
                if st.replace {
                    flags |= statement_flag::REPLACE;
                }
                Statement { pin, offset: 0, len, flags, min_after }
            })
            .collect();
        s.reaction(ReactionSpec {
            name,
            origin,
            on: plan.on.clone(),
            reads: plan.reads.iter().map(|r| BodyPlan::read(pins, limits, *r)).collect(),
            writes,
            ..ReactionSpec::default()
        });
    }
}
