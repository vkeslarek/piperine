//! The digital side of the solver ABI: the value types every discrete
//! participant shares with the scheduler ([`crate::digital::DigitalState`]).
//!
//! Digital evaluation itself is expressed through [`crate::core::element::Element`]
//! — the single simulated-thing contract — via its `boundary`/`init`/`seq_phase`/
//! `comb_phase` methods. This module owns the surrounding wire types those
//! methods speak in: [`DigitalPorts`] (boundary wiring), [`EvalCtx`] (the
//! read-only snapshot), and [`EventSink`] (the write-only event façade). A
//! JIT-compiled logic cone, the A2D/D2A bridge, and an external co-simulator are
//! all just Elements; the scheduler never learns which kind sits behind the
//! contract.
//!
//! ## Contract stability
//!
//! [`DigitalEvent`] is the wire ABI (a value-change on a net at a time). These
//! types evolve **additively only** — a new `#[non_exhaustive]` field, never a
//! signature break — so a model compiled or written against version N keeps
//! working. Treat changes here like changes to a published FFI header.

use crate::digital::{DigitalEvent, DigitalNet, LogicValue};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

pub use piperine_ir::events::CrossDir;

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
    /// Per-analog-terminal voltages for A2D-sampling elements (those that
    /// declare `ElementCapabilities::SAMPLES_ANALOG`); empty otherwise.
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
            region: crate::digital::regions::Region::Active,
        }));
        *self.seq += 1;
    }
}

/// §8.4.6 (LRMC-C7-14, T41): interpolates the crossing time at an
/// analog-to-digital boundary — given the analog sample bracketing a
/// threshold crossing (`t0`/`v0` before, `t1`/`v1` after, `v` measured
/// relative to the threshold so the crossing is `v == 0`), returns the
/// secant-interpolated instant instead of snapping the digital event to
/// `t1` (the sample time). Reuses `piperine_ir::events::predict_crossing`
/// — the same detector `absdelta`/`cross`/`above` events already use in
/// the analog domain (`piperine-codegen::device::analog::events`), so the
/// A2D boundary gets the identical published error bound rather than a
/// second, independent approximation.
///
/// **Scope note (SPEC_DEVIATION, disclosed):** this is the reusable
/// interpolation *mechanism* the task's stated `Where`
/// (`digital/interface.rs` only) covers. No A2D-sampling `Element` in
/// this codebase calls it yet — `EvalCtx::analog` is plumbed through the
/// scheduler ABI (`scheduler.rs`) but grep confirms zero consumers in
/// `piperine-codegen::device::digital`/`kernel::digital` today; there is
/// no comparator-style digital device whose `comb_phase` reads
/// `ctx.analog` to snap against in the first place, so there is no
/// existing "snap to sample" call site to replace. Wiring a real
/// comparator device to call this (JIT emission in `kernel/digital/
/// compile.rs`, tracking each element's own previous analog
/// sample/time) is future work once such a device exists.
///
/// Returns `None` when the bracket doesn't actually cross zero (same
/// contract as `predict_crossing`) — the caller keeps its snap-to-sample
/// fallback (`t1`) in that case, unaffected: "non-interpolated behavior
/// unchanged when absdelta is not in play" (this task's own Done-when).
pub fn interpolate_a2d_crossing(dir: CrossDir, t0: f64, v0: f64, t1: f64, v1: f64) -> Option<piperine_ir::events::Prediction> {
    piperine_ir::events::predict_crossing(dir, t0, v0, t1, v1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::element::{AnalogDevice, DigitalDevice, Element, ElementCapabilities, Introspect};

    /// A minimal external-style model: an inverter written directly against the
    /// stable interface, proving a non-JIT participant needs nothing else.
    struct ExternalInverter {
        input: DigitalNet,
        output: DigitalNet,
        delay: f64,
    }

    impl AnalogDevice for ExternalInverter {}

    impl DigitalDevice for ExternalInverter {
        fn boundary(&self) -> DigitalPorts<'_> {
            DigitalPorts { inputs: std::slice::from_ref(&self.input), outputs: std::slice::from_ref(&self.output) }
        }
        fn init(&mut self, sink: &mut dyn EventSink) {
            sink.emit(self.output, LogicValue::X, 0.0);
        }
        fn comb_phase(&mut self, ctx: &EvalCtx<'_>, sink: &mut dyn EventSink) {
            let out = match ctx.nets[self.input.0] {
                LogicValue::Zero => LogicValue::One,
                LogicValue::One => LogicValue::Zero,
                _ => LogicValue::X,
            };
            sink.emit(self.output, out, self.delay);
        }
    }

    impl Introspect for ExternalInverter {}

    impl Element for ExternalInverter {
        fn name(&self) -> &str { "external_inverter" }
        fn capabilities(&self) -> ElementCapabilities { ElementCapabilities::DIGITAL }
    }

    /// LRMC-C7-14/T41: the A2D interpolated crossing time matches the
    /// LRM's own secant-interpolation expectation within the published
    /// `error_bound` — a linear bracket interpolates exactly (the same
    /// property `piperine_ir::events::predict_crossing`'s own test
    /// already proves; re-asserted here against the digital-facing
    /// re-export so a future signature drift is caught at this layer
    /// too, not only upstream).
    #[test]
    fn a2d_crossing_interpolates_within_the_published_error_bound() {
        // v(t) = t - 0.5 relative to the threshold, bracketed by [0, 1] —
        // the true crossing is t = 0.5.
        let p = interpolate_a2d_crossing(CrossDir::Rising, 0.0, -0.5, 1.0, 0.5).expect("brackets a rising crossing");
        assert!((p.t_cross - 0.5).abs() <= p.error_bound, "t_cross={}, error_bound={}", p.t_cross, p.error_bound);
        assert!((p.t_cross - 0.5).abs() < 1e-12, "linear bracket interpolates exactly");
    }

    /// "Non-interpolated behavior unchanged when absdelta is not in
    /// play" (this task's own Done-when): a non-bracketing sample pair
    /// (no sign change, or the wrong direction) yields `None` — the
    /// caller's existing snap-to-sample fallback is untouched.
    #[test]
    fn non_crossing_samples_yield_no_interpolation() {
        assert!(interpolate_a2d_crossing(CrossDir::Rising, 0.0, 0.5, 1.0, 1.0).is_none(), "no sign change");
        assert!(interpolate_a2d_crossing(CrossDir::Falling, 0.0, -0.5, 1.0, 0.5).is_none(), "wrong direction");
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
