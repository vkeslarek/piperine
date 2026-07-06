//! The stable digital **event interface** — the "OSDI for digital".
//!
//! Every participant in the discrete world talks to the scheduler
//! ([`crate::topology::DigitalState`]) only through [`DigitalEventModel`]:
//!
//! - a JIT-compiled Piperine logic cone (today: one per instance; the
//!   follow-up fuses a whole ranked network into one — see
//!   `piperine-codegen/docs/DIGITAL_JIT.md`),
//! - the analog engine's A2D/D2A bridge,
//! - an **external co-simulator** (an Arduino core, an ESP32 image, a
//!   hand-written peripheral) plugged in-process or over FFI.
//!
//! The scheduler never learns which kind sits behind the trait. That is the
//! whole point: the fused JIT kernel and a running firmware emulator must be
//! interchangeable, or the JIT would be baked into the core and external models
//! made second-class.
//!
//! ## Contract stability
//!
//! [`DigitalEvent`] is the wire ABI (a value-change on a net at a time). This
//! trait and that struct evolve **additively only** — a new default method or a
//! new `#[non_exhaustive]` field, never a signature break — so a model compiled
//! or written against version N keeps working. Treat changes here like changes
//! to a published FFI header.

use crate::digital::{DigitalEvent, DigitalNet, LogicValue};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// A model's boundary wiring: the nets it reads (its sensitivity list) and the
/// nets it drives. Net ids are allocated by the circuit builder and are the
/// scheduler's shared namespace across every model.
#[derive(Debug, Clone, Copy)]
pub struct DigitalPorts<'a> {
    /// Nets the model reads. A change on any of these wakes the model.
    pub inputs: &'a [DigitalNet],
    /// Nets the model drives.
    pub outputs: &'a [DigitalNet],
}

/// Read-only snapshot handed to a model at evaluation time. Carries no mutable
/// access to circuit internals — a model observes and emits, nothing else.
#[derive(Debug, Clone, Copy)]
pub struct EvalCtx<'a> {
    /// Current simulation time (seconds).
    pub time: f64,
    /// Logic state of every digital net, indexed by [`DigitalNet`].
    pub nets: &'a [LogicValue],
    /// Per-analog-terminal voltages for A2D-sampling models
    /// ([`DigitalEventModel::samples_analog`]); empty otherwise.
    pub analog: &'a [f64],
}

/// Write-only façade over the scheduler's event queue. A model emits future
/// net value-changes through this and never names the concrete queue type, so
/// the scheduler is free to batch, reorder, or route a model's events over FFI
/// without any model change.
pub trait EventSink {
    /// Schedule `net` to take `value` at `now + delay`. `delay == 0.0` is a
    /// same-timestep (delta-cycle) update.
    fn emit(&mut self, net: DigitalNet, value: LogicValue, delay: f64);
}

/// The stable digital model contract. See the module docs.
pub trait DigitalEventModel: Send {
    /// Boundary wiring (input/output nets). Stable across a model's lifetime.
    fn boundary(&self) -> DigitalPorts<'_>;

    /// Power-on: apply register initial values and emit initial output events
    /// (typically at `t = 0`).
    fn init(&mut self, sink: &mut dyn EventSink);

    /// React to the net state in `ctx`; emit resulting output events into
    /// `sink`. Called when an input net changed (or explicitly for
    /// analog-sampling models when only a voltage moved).
    fn evaluate(&mut self, ctx: &EvalCtx<'_>, sink: &mut dyn EventSink);

    /// Whether this model's logic samples analog quantities (A2D). Such models
    /// are evaluated on an analog solve even without a digital input event.
    fn samples_analog(&self) -> bool {
        false
    }
}

/// The concrete [`EventSink`] backing today's scheduler: a binary-heap event
/// queue. Constructed per model per evaluation so `source`/`seq` provenance is
/// filled in for the model automatically.
pub struct QueueSink<'q> {
    queue: &'q mut BinaryHeap<Reverse<DigitalEvent>>,
    base_time: f64,
    source: usize,
    seq: &'q mut u64,
}

impl<'q> QueueSink<'q> {
    /// Wrap the scheduler queue for a model identified by `source`, stamping
    /// events at `base_time + delay` with a monotonic `seq` tiebreaker.
    pub fn new(
        queue: &'q mut BinaryHeap<Reverse<DigitalEvent>>,
        base_time: f64,
        source: usize,
        seq: &'q mut u64,
    ) -> Self {
        Self { queue, base_time, source, seq }
    }
}

impl EventSink for QueueSink<'_> {
    fn emit(&mut self, net: DigitalNet, value: LogicValue, delay: f64) {
        self.queue.push(Reverse(DigitalEvent {
            time: self.base_time + delay,
            net,
            value,
            source: self.source,
            seq: *self.seq,
        }));
        *self.seq += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal external-style model: an inverter written directly against the
    /// stable interface, proving a non-JIT participant needs nothing else.
    struct ExternalInverter {
        input: DigitalNet,
        output: DigitalNet,
        delay: f64,
    }

    impl DigitalEventModel for ExternalInverter {
        fn boundary(&self) -> DigitalPorts<'_> {
            DigitalPorts { inputs: std::slice::from_ref(&self.input), outputs: std::slice::from_ref(&self.output) }
        }
        fn init(&mut self, sink: &mut dyn EventSink) {
            sink.emit(self.output, LogicValue::X, 0.0);
        }
        fn evaluate(&mut self, ctx: &EvalCtx<'_>, sink: &mut dyn EventSink) {
            let out = match ctx.nets[self.input.0] {
                LogicValue::Zero => LogicValue::One,
                LogicValue::One => LogicValue::Zero,
                _ => LogicValue::X,
            };
            sink.emit(self.output, out, self.delay);
        }
    }

    #[test]
    fn external_model_emits_through_the_sink() {
        let mut model = ExternalInverter { input: DigitalNet(0), output: DigitalNet(1), delay: 2.0 };
        let nets = [LogicValue::Zero, LogicValue::X];
        let mut queue: BinaryHeap<Reverse<DigitalEvent>> = BinaryHeap::new();
        let mut seq = 0u64;
        {
            let mut sink = QueueSink::new(&mut queue, 5.0, 42, &mut seq);
            model.evaluate(&EvalCtx { time: 5.0, nets: &nets, analog: &[] }, &mut sink);
        }
        let Reverse(ev) = queue.pop().expect("one event");
        assert_eq!(ev.net, DigitalNet(1));
        assert_eq!(ev.value, LogicValue::One);
        assert_eq!(ev.time, 7.0); // 5.0 + delay 2.0
        assert_eq!(ev.source, 42);
        assert_eq!(seq, 1);
    }
}
