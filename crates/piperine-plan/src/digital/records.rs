//! Records: the layout of a stream and its record and trace pins.

use piperine_abi::raw::{DEFAULT, Field, Layout, List, Str, leaf, pin_flag, role, watch};
use piperine_abi::shape::{PinSpec, ShapeBuf, WatchSpec};
use piperine_ir::{Body, Ir, Stream, ValueType};

use crate::error::PlanError;
use crate::pin::PinLayout;
use crate::digital::{CarrierLayout, Digital};

/// The layout of a stream's `RECORD` pin: each value field laid out as a pin of its type
/// (`PinLayout`: a carrier's scalar layout, or a composite's), in the stream's order, at the
/// next offset its alignment allows; the size rounded up to the largest alignment.
#[derive(Clone, Debug)]
pub struct RecordLayout {
    pub size: u32,
    pub align: u32,
    /// Per value field of the stream: its offset and the layout it is laid out as.
    pub fields: Vec<(u32, PinLayout)>,
    /// Every leaf of the layout as an ABI field.
    leaves: Vec<RecordLeaf>,
}

/// One ABI field of a record layout: a scalar leaf of a value field, named by the field.
#[derive(Clone, Debug)]
struct RecordLeaf {
    name: String,
    leaf: u32,
    offset: u32,
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
        Some(RecordLayout { size: end.next_multiple_of(align), align, fields, leaves })
    }

    /// The ABI fields of the value field `name` laid out as `layout` at `offset`: the leaf of a
    /// scalar is named by the field, a composite's leaves by the field and their own path.
    fn leaves_of(name: &str, offset: u32, layout: &PinLayout) -> Option<Vec<RecordLeaf>> {
        match layout {
            PinLayout::Scalar(scalar) => {
                let v = scalar.fields.get(0)?;
                Some(vec![RecordLeaf { name: name.to_string(), leaf: v.leaf, offset }])
            }
            PinLayout::Composite(composite) => Some(
                composite
                    .leaves()
                    .iter()
                    .map(|l| RecordLeaf {
                        name: if l.name.is_empty() { name.to_string() } else if l.name.starts_with('[') { format!("{name}{}", l.name) } else { format!("{name}.{}", l.name) },
                        leaf: l.leaf,
                        offset: offset + l.offset,
                    })
                    .collect(),
            ),
        }
    }

    /// Where the record starts in a `TRACE` watch: after the `BOOL` presence byte, at the
    /// record's own alignment (abi.md A5).
    pub fn trace_start(&self) -> u32 {
        1u32.next_multiple_of(self.align)
    }

    /// The size of a `TRACE` watch holding the record: the presence byte, then the record.
    pub fn trace_size(&self) -> u32 {
        (self.trace_start() + self.size).next_multiple_of(self.align)
    }

    /// The ABI fields of the layout: each value field's leaves, named after the stream's fields.
    fn abi_fields(&self) -> Vec<Field<'_>> {
        self.leaves
            .iter()
            .map(|l| Field { name: Str::from_str(&l.name), leaf: l.leaf, offset: l.offset, count: 1 })
            .collect()
    }
}

impl Digital {
    /// Adds one `PRIVATE` `RECORD` pin per stream of `body`, named by the stream; returns their
    /// indices. The body passes `check()`, so every field holds `bool`, `f64` or an integer.
    pub(crate) fn record_pins(s: &mut ShapeBuf, body: Body<'_>) -> Result<Vec<u32>, PlanError> {
        let at = Self::at(body.origin());
        body.streams()
            .iter()
            .map(|stream| {
                let Some(record) = RecordLayout::in_ir(body.ir(), stream) else {
                    return Err(PlanError::new(body.name(), format!("stream {} holds a field no pin holds", stream.name)));
                };
                let fields = record.abi_fields();
                let layout = Layout { size: record.size, align: record.align, fields: List::from_slice(&fields) };
                Ok(s.pin(PinSpec { origin: &at, flags: pin_flag::RECORD, ..PinSpec::discrete(&stream.name, role::PRIVATE, &layout) }))
            })
            .collect()
    }

    /// Adds one `TRACE` watch per trace site of the `analog` block, `trace:<k>`, after the `CAP`
    /// watches: its layout is a `BOOL` presence byte (0 for "not written in this evaluation", abi.md
    /// A5) followed by the record layout of the site's stream, whose fields the site leaves out
    /// stay zero. Returns each site's watch.
    pub(crate) fn trace_watches(s: &mut ShapeBuf, body: Body<'_>) -> Result<Vec<u32>, PlanError> {
        body.traces()
            .iter()
            .enumerate()
            .map(|(k, site)| {
                let Some(stream) = body.streams().get(site.stream as usize) else {
                    return Err(PlanError::new(body.name(), format!("trace site {k} names stream {}, out of range", site.stream)));
                };
                let Some(record) = RecordLayout::in_ir(body.ir(), stream) else {
                    return Err(PlanError::new(body.name(), format!("stream {} holds a field no pin holds", stream.name)));
                };
                let start = record.trace_start();
                let mut fields = vec![Field { name: Str::from_str("present"), leaf: leaf::BOOL, offset: 0, count: 1 }];
                fields.extend(record.abi_fields().into_iter().map(|f| Field { offset: f.offset + start, ..f }));
                let layout = Layout { size: record.trace_size(), align: record.align, fields: List::from_slice(&fields) };
                Ok(s.watch(WatchSpec {
                    name: &format!("trace:{k}"),
                    origin: &Self::at(&site.origin),
                    kind: watch::TRACE,
                    layout: &layout,
                    event: DEFAULT,
                    ttol: DEFAULT,
                    etol: DEFAULT,
                }))
            })
            .collect()
    }
}
