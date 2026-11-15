//! The digital side of the solver ABI: the value types every discrete
//! participant shares with the scheduler (the owner's scheduler state).
//!
//! Digital evaluation itself is expressed through [`DigitalDevice`]
//! — the digital half of the owner's single simulated-thing contract — via its `boundary`/`init`/`seq_phase`/
//! `comb_phase` methods. This module owns the surrounding wire types those
//! methods speak in: [`DigitalPorts`] (boundary wiring), [`EvalCtx`] (the
//! read-only snapshot), and [`EventSink`] (the write-only event façade). A
//! JIT-compiled logic cone, the A2D/D2A bridge, and an external co-simulator are
//! all just digital devices; the scheduler never learns which kind sits behind the
//! contract.
//!
//! ## Contract stability
//!
//! [`DigitalEvent`] is the wire ABI (a value-change on a net at a time). These
//! types evolve **additively only** — a new `#[non_exhaustive]` field, never a
//! signature break — so a model compiled or written against version N keeps
//! working. Treat changes here like changes to a published FFI header.

use crate::events::{DigitalEvent, DigitalNet, LogicValue};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};

/// Which sign changes a crossing detector fires on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossDir {
    Rising,
    Falling,
    Either,
}

/// A predicted crossing instant with its error bound — `t_cross` is inside
/// `[t0, t1]`, the bracket that produced it, and `error_bound` is the width
/// of that bracket (never smaller than what the caller actually resolved).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Prediction {
    /// The interpolated (secant) crossing instant.
    pub t_cross: f64,
    /// How far `t_cross` could still be off — the bracket width. Shrinks
    /// as the bracket narrows; never claims better than it has.
    pub error_bound: f64,
}

/// One secant prediction from a bracketing pair of accepted samples whose
/// sign change matches `dir` (`Rising`: `v0 <= 0` and `v1 > 0`; `Falling`:
/// `v0 >= 0` and `v1 < 0`; `Either`: any sign change). `None` when
/// `(t0, v0)`-`(t1, v1)` does not actually bracket a crossing in that
/// direction.
pub fn predict_crossing(dir: CrossDir, t0: f64, v0: f64, t1: f64, v1: f64) -> Option<Prediction> {
    let brackets = match dir {
        CrossDir::Rising => v0 <= 0.0 && v1 > 0.0,
        CrossDir::Falling => v0 >= 0.0 && v1 < 0.0,
        CrossDir::Either => (v0 <= 0.0 && v1 > 0.0) || (v0 >= 0.0 && v1 < 0.0),
    };
    if !brackets || v1 == v0 {
        return None;
    }
    let t_cross = t0 + (t1 - t0) * (-v0) / (v1 - v0);
    Some(Prediction { t_cross, error_bound: (t1 - t0).abs() })
}

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

    /// Same as [`Self::emit`], but the emitting model supplies an explicit
    /// LRM §7.9 Table 7-2 drive strength for this value-change (LRMW3-09,
    /// A.2.2.2 — e.g. `assign (strong1, weak0) w = r;`) instead of letting
    /// the scheduler fall back to [`super::drivers::default_strength_for`].
    /// Additive: the default implementation drops `strength` and forwards
    /// to [`Self::emit`], so an existing `EventSink` implementer (an
    /// external co-simulator, a hand-written test double) keeps compiling
    /// and behaving exactly as before without overriding it. Only
    /// [`QueueSink`] threads the strength through to the scheduler today.
    fn emit_with_strength(
        &mut self,
        net: DigitalNet,
        value: LogicValue,
        delay: f64,
        strength: crate::drivers::Strength,
    ) {
        let _ = strength;
        self.emit(net, value, delay);
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

impl QueueSink<'_> {
    /// Shared push logic for [`EventSink::emit`]/[`EventSink::emit_with_strength`]
    /// — the only difference between the two is whether `strength` is `Some`.
    fn push(&mut self, net: DigitalNet, value: LogicValue, delay: f64, strength: Option<crate::drivers::Strength>) {
        self.queue.push(Reverse(DigitalEvent {
            time: self.base_time + delay,
            net,
            value,
            source: self.source,
            seq: *self.seq,
            strength,
        }));
        *self.seq += 1;
    }
}

impl EventSink for QueueSink<'_> {
    fn emit(&mut self, net: DigitalNet, value: LogicValue, delay: f64) {
        self.push(net, value, delay, None);
    }

    fn emit_with_strength(
        &mut self,
        net: DigitalNet,
        value: LogicValue,
        delay: f64,
        strength: crate::drivers::Strength,
    ) {
        self.push(net, value, delay, Some(strength));
    }
}

/// §8.4.6 (LRMC-C7-14, T41): interpolates the crossing time at an
/// analog-to-digital boundary — given the analog sample bracketing a
/// threshold crossing (`t0`/`v0` before, `t1`/`v1` after, `v` measured
/// relative to the threshold so the crossing is `v == 0`), returns the
/// secant-interpolated instant instead of snapping the digital event to
/// `t1` (the sample time). Uses [`predict_crossing`], the secant
/// detector with a published error bound.
///
/// **Real consumer (T63, LRMW2-60):**
/// [`piperine_codegen::device::A2dComparator`](../../../piperine_codegen/device/struct.A2dComparator.html)
/// — a hand-written `Element` (no JIT involved, the same non-JIT-
/// participant pattern this module's own `ExternalInverter` test proves
/// needs nothing else) samples one analog terminal via `EvalCtx::analog`,
/// tracks the previous sample, and on a threshold crossing schedules the
/// digital transition at this function's interpolated instant instead
/// of snapping to the later sample's own solver time. See that device's
/// own module doc for the disclosed remaining gap (the wider mixed-
/// signal transient loop does not yet *retry* a step to land exactly on
/// a *predicted future* crossing the way `cross`/`above` do for
/// pure-analog blocks) — this function itself has a real caller now,
/// with real sample data, no longer only unit-tested against literal
/// numbers.
///
/// Returns `None` when the bracket doesn't actually cross zero (same
/// contract as `predict_crossing`) — the caller keeps its snap-to-sample
/// fallback (`t1`) in that case, unaffected: "non-interpolated behavior
/// unchanged when absdelta is not in play" (this task's own Done-when).
pub fn interpolate_a2d_crossing(dir: CrossDir, t0: f64, v0: f64, t1: f64, v1: f64) -> Option<Prediction> {
    predict_crossing(dir, t0, v0, t1, v1)
}

/// Digital participation: two-phase delta cycle + hidden-state round-trip.
///
/// The delta cycle is two-phase to preserve non-blocking (NBA) semantics
/// across register chains (SPEC §9): the scheduler calls `seq_phase` on every
/// woken element first, then `comb_phase` on every woken element, so a
/// register samples the pre-edge net snapshot instead of racing ahead.
///
/// Every method defaults to an element that drives no nets, so a purely
/// analog device inherits the inert digital surface untouched.
pub trait DigitalDevice: Send + Sync {
    /// Boundary wiring: the nets this element reads (its sensitivity list) and
    /// the nets it drives. Defaults to driving/reading nothing.
    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts { inputs: &[], outputs: &[] }
    }

    /// Power-on: apply register initial values and emit initial output events
    /// (typically at `t = 0`). No-op for elements with no digital state.
    fn init(&mut self, _sink: &mut dyn EventSink) {}

    /// Phase 1 (register commit): detect clock edges against the previous
    /// evaluation and commit register writes from the pre-settle net snapshot.
    /// Returns whether any clocked block fired. **Must not** emit output events
    /// — those happen in [`comb_phase`](DigitalDevice::comb_phase).
    fn seq_phase(&mut self, _ctx: &EvalCtx<'_>) -> bool { false }

    /// Phase 2 (combinational): recompute outputs from live `ctx.nets` and the
    /// (possibly just-committed) register banks, emitting change events into
    /// `sink`.
    fn comb_phase(&mut self, _ctx: &EvalCtx<'_>, _sink: &mut dyn EventSink) {}

    /// Fused one-shot evaluation: [`seq_phase`](DigitalDevice::seq_phase) then
    /// [`comb_phase`](DigitalDevice::comb_phase) in a single call. Used by external
    /// co-simulators that don't participate in the scheduler's two-phase cycle.
    fn evaluate(&mut self, ctx: &EvalCtx<'_>, sink: &mut dyn EventSink) {
        self.seq_phase(ctx);
        self.comb_phase(ctx, sink);
    }

    /// Convenience: true if any of the element's input nets is in `changed`.
    fn has_input_on(&self, changed: &HashSet<DigitalNet>) -> bool {
        self.boundary().inputs.iter().any(|n| changed.contains(n))
    }

    /// Hidden digital state (module vars, edge-detection memory) as an
    /// opaque `(int, real)` carrier, snapshotted into each recorded
    /// `TransientStep` and restored verbatim on full-state
    /// re-entry (PSS shots, `TransientSolver::with_initial_state`) — the
    /// shot-state contract requires register state to round-trip with the
    /// digital nets. `None` = stateless (pure combinational) element.
    fn digital_hidden_snapshot(&self) -> Option<(Vec<i64>, Vec<f64>)> {
        None
    }

    /// Restore a state previously produced by [`Self::digital_hidden_snapshot`].
    /// Called on full-state re-entry after `init`, before the first settle.
    fn digital_hidden_restore(&mut self, _state: &(Vec<i64>, Vec<f64>)) {}
}

/// A [`DigitalDevice`] the scheduler can tell apart from a purely analog one.
/// The owner implements it for its composed element type, answering from the
/// capability flags it declares; the scheduler and the topology are generic
/// over it.
pub trait DigitalParticipant: DigitalDevice {
    /// Whether this participant takes part in digital evaluation.
    fn is_digital(&self) -> bool;
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

    /// LRMC-C7-14/T41: the A2D interpolated crossing time matches the
    /// LRM's own secant-interpolation expectation within the published
    /// `error_bound` — a linear bracket interpolates exactly (the same
    /// property `predict_crossing`'s own test
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
    fn predict_crossing_interpolates_a_linear_bracket_exactly() {
        // v(t) = t - 0.5, bracketed by [0, 1]: the secant is exact for a
        // linear function.
        let p = predict_crossing(CrossDir::Rising, 0.0, -0.5, 1.0, 0.5).unwrap();
        assert!((p.t_cross - 0.5).abs() < 1e-15);
        assert_eq!(p.error_bound, 1.0, "bracket width, unrefined");
    }

    #[test]
    fn predict_crossing_rejects_a_non_bracketing_pair() {
        assert!(predict_crossing(CrossDir::Rising, 0.0, 0.5, 1.0, 1.0).is_none(), "no sign change");
        assert!(predict_crossing(CrossDir::Falling, 0.0, -0.5, 1.0, 0.5).is_none(), "wrong direction");
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
