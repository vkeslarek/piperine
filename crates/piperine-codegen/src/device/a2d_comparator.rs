//! A2D bridge comparator (T63, LRMW2-60): the first real consumer of
//! [`interpolate_a2d_crossing`] (`piperine_solver::abi`, built by T41/
//! LRMC-C7-14 but never called by any Element until this task). Samples
//! one analog terminal via `EvalCtx::analog[terminal_index]` and, on a
//! threshold crossing between two consecutive samples, schedules the
//! digital transition at the secant-interpolated crossing instant
//! instead of snapping it to the later sample's own solver time —
//! §8.4.6's absdelta-style A2D event text: "interpolating the times
//! between the last time step and next time step." Quantizes the
//! interpolated time onto an optional digital tick grid (T26/LRMW2-26,
//! [`DigitalKernel::quantize_time`](crate::kernel::digital::DigitalKernel::quantize_time)'s
//! own formula) *after* interpolating, never before — quantizing first
//! would throw away the sub-tick precision interpolation exists to
//! recover, and this device exists specifically to prove that ordering
//! (T63 Done-when #2).
//!
//! **SPEC_DEVIATION (disclosed, not silent):** the emitted
//! [`DigitalEvent`](piperine_solver::abi::DigitalEvent)'s `time` is the
//! true interpolated instant, which necessarily falls at or before the
//! solver time `comb_phase` itself runs at (the crossing is somewhere
//! inside the just-completed `[prev, now]` bracket, never after `now`).
//! Making the wider mixed-signal transient loop actually *retry* with a
//! smaller analog step so it lands exactly on a *predicted future*
//! crossing (the way `cross`/`above` events already do for pure-analog
//! blocks — see `device::analog::events`) is a separate, larger
//! solver-loop integration this task's stated `Where` (`digital/
//! interface.rs`, `device/`) does not cover. This device closes the
//! narrower, specific gap `interpolate_a2d_crossing`'s own prior
//! disclosure named — "no comparator-style digital device... calls it"
//! — with a real, tested `Element`: it calls the mechanism with real
//! sample data, and its emitted event genuinely carries the
//! interpolated (not snapped) time, proven directly against the
//! `DigitalEvent`'s own `time` field (`crates/piperine-codegen/tests/
//! a2d_comparator_interpolation.rs`), the same way `ExternalInverter`
//! (`piperine-solver::digital::interface`'s own test module) proves a
//! non-JIT participant needs nothing else.

use piperine_solver::abi::{
    interpolate_a2d_crossing, AnalogDevice, CrossDir, DigitalDevice, DigitalNet, DigitalPorts, Element,
    ElementCapabilities, EvalCtx, EventSink, Introspect, LogicValue,
};

/// A single-terminal analog-to-digital comparator: `output` is `One`
/// when the sampled analog terminal is at or above `threshold`, `Zero`
/// otherwise. The transition's *scheduled* time is the secant-
/// interpolated crossing instant when the current sample brackets one
/// with the previous sample — the plain "same time as this call"
/// behavior (`delay = 0.0`) applies only on the very first sample (no
/// previous bracket exists yet) or when no crossing occurred.
pub struct A2dComparator {
    name: String,
    output: DigitalNet,
    terminal_index: usize,
    threshold: f64,
    /// The digital engine's minimum time granularity (T26/LRMW2-26) — the
    /// interpolated crossing instant is quantized onto this grid, `None`
    /// meaning no quantization (matches `DigitalKernel::timescale_grid`'s
    /// own `` `timescale ``-absent default).
    tick: Option<f64>,
    /// `(time, value - threshold)` of the previous sample — `None` before
    /// the first `comb_phase` call.
    prev: Option<(f64, f64)>,
}

impl A2dComparator {
    pub fn new(name: impl Into<String>, output: DigitalNet, terminal_index: usize, threshold: f64, tick: Option<f64>) -> Self {
        Self { name: name.into(), output, terminal_index, threshold, tick, prev: None }
    }

    /// T26/LRMW2-26: rounds `t` to the nearest multiple of `tick` — the
    /// same formula `DigitalKernel::quantize_time` applies, kept in sync
    /// deliberately (both round `(t / grid).round() * grid`) rather than
    /// sharing a fn, since this device has no `DigitalKernel` of its own
    /// to borrow the method from.
    fn quantize(&self, t: f64) -> f64 {
        match self.tick {
            Some(grid) if grid > 0.0 => (t / grid).round() * grid,
            _ => t,
        }
    }
}

impl AnalogDevice for A2dComparator {}
impl Introspect for A2dComparator {}

impl DigitalDevice for A2dComparator {
    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts { inputs: &[], outputs: std::slice::from_ref(&self.output) }
    }

    fn init(&mut self, sink: &mut dyn EventSink) {
        sink.emit(self.output, LogicValue::X, 0.0);
    }

    fn comb_phase(&mut self, ctx: &EvalCtx<'_>, sink: &mut dyn EventSink) {
        let raw = ctx.analog.get(self.terminal_index).copied().unwrap_or(0.0);
        let v = raw - self.threshold;
        let now = if v >= 0.0 { LogicValue::One } else { LogicValue::Zero };

        if let Some((t0, v0)) = self.prev {
            let dir = if v0 < 0.0 && v >= 0.0 {
                Some(CrossDir::Rising)
            } else if v0 >= 0.0 && v < 0.0 {
                Some(CrossDir::Falling)
            } else {
                None
            };
            if let Some(dir) = dir {
                if let Some(pred) = interpolate_a2d_crossing(dir, t0, v0, ctx.time, v) {
                    let event_time = self.quantize(pred.t_cross);
                    sink.emit(self.output, now, event_time - ctx.time);
                }
            }
        }

        self.prev = Some((ctx.time, v));
    }
}

impl Element for A2dComparator {
    fn name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::DIGITAL | ElementCapabilities::SAMPLES_ANALOG
    }
}
