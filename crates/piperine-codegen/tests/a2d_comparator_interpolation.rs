//! LRM-COVERS: 8.4.6
//! T63 (LRMW2-60): `A2dComparator` (`device/a2d_comparator.rs`) is the
//! first real `Element` to call `interpolate_a2d_crossing`
//! (`piperine_solver::abi`, built by T41/LRMC-C7-14 but never called by
//! any Element until this task). Proves both stated Done-when items
//! directly against the emitted `DigitalEvent`'s own `time` field: (1)
//! a crossing between two timepoints schedules the transition at the
//! interpolated instant, not at the later sample's own solver time, and
//! (2) the interpolated instant is quantized onto the digital tick grid
//! (T26/LRMW2-26) *after* interpolating, not before — the quantized
//! result differs from both the raw interpolated value and either
//! sample's own time, proving genuine interpolate-then-quantize
//! cooperation rather than a no-op or a snap-to-sample fallback.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use piperine_codegen::device::A2dComparator;
use piperine_solver::abi::{DigitalDevice, DigitalEvent, DigitalNet, EvalCtx, LogicValue, QueueSink};

/// v(t) relative to the 2.5 threshold is linear: `v0 = -0.23` at `t0 =
/// 0.0`, `v1 = 0.77` at `t1 = 1.0` — a rising crossing at the exact
/// (secant-exact, since linear) fraction `0.23` of the bracket, i.e.
/// `t_cross = 0.23`. `raw = threshold + v` is what `ctx.analog[0]`
/// carries (the comparator only ever sees the raw sampled voltage).
const THRESHOLD: f64 = 2.5;
const RAW0: f64 = THRESHOLD - 0.23;
const RAW1: f64 = THRESHOLD + 0.77;

fn sample(comparator: &mut A2dComparator, time: f64, raw: f64, source: usize) -> Vec<DigitalEvent> {
    let analog = [raw];
    let ctx = EvalCtx { time, nets: &[], analog: &analog };
    let mut queue: BinaryHeap<Reverse<DigitalEvent>> = BinaryHeap::new();
    let mut seq = 0u64;
    {
        let mut sink = QueueSink::new(&mut queue, time, source, &mut seq);
        comparator.comb_phase(&ctx, &mut sink);
    }
    queue.into_sorted_vec().into_iter().map(|Reverse(e)| e).collect()
}

/// T63 Done-when #1 (discriminating): the crossing between `t0 = 0.0`
/// and `t1 = 1.0` schedules the digital transition at the interpolated
/// instant `0.23`, not at `t1 = 1.0` (the timepoint `comb_phase` itself
/// runs at when it detects the crossing).
#[test]
fn crossing_schedules_at_the_interpolated_instant_not_the_timepoint() {
    let mut comparator = A2dComparator::new("cmp1", DigitalNet(0), 0, THRESHOLD, None);

    let first = sample(&mut comparator, 0.0, RAW0, 7);
    assert!(first.is_empty(), "no bracket yet on the first sample: {first:?}");

    let second = sample(&mut comparator, 1.0, RAW1, 7);
    assert_eq!(second.len(), 1, "exactly one crossing event: {second:?}");
    let ev = &second[0];
    assert_eq!(ev.value, LogicValue::One, "rising crossing settles the output high");
    assert_eq!(ev.net, DigitalNet(0));
    assert_eq!(ev.source, 7);
    assert!(
        (ev.time - 0.23).abs() < 1e-12,
        "event must land at the interpolated crossing t=0.23, not the timepoint t=1.0: got {}",
        ev.time
    );
    assert_ne!(ev.time, 1.0, "must not snap to the later sample's own solver time");
}

/// T63 Done-when #2 (ordering asserted): with a `tick = 0.05` digital
/// grid, the emitted event time is `quantize(0.23) = 0.25` — the
/// *interpolated* instant rounded onto the grid, not `0.23` itself
/// (unquantized) and not either sample's own time (`0.0`/`1.0`,
/// themselves already grid-aligned and therefore indistinguishable from
/// a "quantize-then-interpolate" wrong order if the raw sample times
/// were used verbatim). Confirms quantization runs on top of the
/// interpolated result, not in place of it.
#[test]
fn interpolated_crossing_is_quantized_after_interpolating_not_before() {
    let mut comparator = A2dComparator::new("cmp2", DigitalNet(0), 0, THRESHOLD, Some(0.05));

    sample(&mut comparator, 0.0, RAW0, 9);
    let events = sample(&mut comparator, 1.0, RAW1, 9);
    assert_eq!(events.len(), 1, "exactly one crossing event: {events:?}");
    let t = events[0].time;

    assert!((t - 0.25).abs() < 1e-12, "quantize(0.23, tick=0.05) must be 0.25: got {t}");
    assert!((t - 0.23).abs() > 1e-9, "the raw interpolated 0.23 must not pass through unquantized: got {t}");
    assert_ne!(t, 0.0, "must not collapse to the earlier sample's own time");
    assert_ne!(t, 1.0, "must not collapse to the later sample's own time (the timepoint)");
}

/// "Non-interpolated behavior unchanged when absdelta is not in play"
/// (the interpolation mechanism's own established contract, `digital/
/// interface.rs`'s doc comment): two samples that never bracket a
/// crossing emit nothing.
#[test]
fn no_crossing_emits_no_event() {
    let mut comparator = A2dComparator::new("cmp3", DigitalNet(0), 0, THRESHOLD, None);
    sample(&mut comparator, 0.0, THRESHOLD + 1.0, 3);
    let events = sample(&mut comparator, 1.0, THRESHOLD + 2.0, 3);
    assert!(events.is_empty(), "no sign change relative to the threshold: {events:?}");
}
