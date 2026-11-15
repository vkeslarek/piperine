//! The compiled oscillator and the compiled DPWM on the solver (feature `ir-rust`, RS-13 AC 6
//! and 11; checks C85, C90, C95): the oscillator alone for 100 ns, then into `Dpwm` with `run`
//! true and the duty word 1325 for eight switching periods. The expected instants are
//! computed here from `examples/buck/SPEC.md` §5 (a 9-bit counter at 200 MHz wrapping at 400,
//! duty word `8 * coarse + fine` with `fine` dithered over a bit-reversed 3-bit frame counter,
//! dead time 4 counts, the low side released at count 396) and not read back from the model.
#![cfg(unix)]

use std::sync::{Mutex, OnceLock};

use piperine_abi::layout::{HIGH, LOW};
use piperine_ir::{Constant, Real, ValueType};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "support/mixed.rs"]
mod mixed;

use digital::{Buck, Ty};
use mixed::{Built, Seen, Wire, constant, rise};

const BIT: ValueType = ValueType::Bool;
/// One clock count (5 ns) in ticks.
const COUNT: i64 = 5_000_000;
/// The counter wraps at N = 400 (SPEC §5.1); the dead time is 4 counts and the low side is
/// released at count 396 (SPEC §5.3).
const N: i64 = 400;
const DEAD: i64 = 4;
const RELEASE: i64 = N - DEAD;

fn oscillator() -> Wire {
    let mut w = Wire::new();
    let osc = Buck::osc(&mut w.b);
    w.place("osc", osc, vec![Constant::Real(Real::new(200e6))], &[("clk", BIT)]);
    w
}

#[test]
fn the_oscillator_toggles_every_half_period() {
    let w = oscillator();
    let seen = Built::new(w.finish()).run(100e-9, 1e-9);
    // 200 MHz: a period of 5 ns, a toggle every 2.5 ns, from false: true at 2.5 ns, false at
    // 5 ns, ... `k` = 1 to 40 inside 100 ns, and no change at any other instant
    let expected: Vec<(i64, Vec<u8>)> =
        (1..=40).map(|k| (k * 2_500_000, if k % 2 == 1 { HIGH.to_vec() } else { LOW.to_vec() })).collect();
    assert_eq!(seen.transient("clk"), expected);
}

/// The oscillator into `Dpwm`, with `run` true, the duty word 1325, and both current-limit
/// inputs known false.
fn dpwm_design() -> Built {
    let mut w = Wire::new();
    let osc = Buck::osc(&mut w.b);
    let dpwm = Buck::dpwm(&mut w.b);
    let run = constant(&mut w.b, "Run", BIT, Constant::Bool(true));
    let duty = constant(&mut w.b, "Duty", Ty::u(12), Constant::Int(1325));
    let off = constant(&mut w.b, "Off", BIT, Constant::Bool(false));
    w.place("osc", osc, vec![Constant::Real(Real::new(200e6))], &[("clk", BIT)]);
    w.place("run", run, vec![], &[("run", BIT)]);
    w.place("duty", duty, vec![], &[("duty", Ty::u(12))]);
    w.place("oc_off", off, vec![], &[("oc", BIT)]);
    w.place("sink_off", off, vec![], &[("sink_oc", BIT)]);
    // clk, run, duty, oc, sink_oc, hs, ls, sample, cut, then the signals count, width, frame,
    // hs_off, hs_cut, ls_cut
    w.place(
        "dpwm",
        dpwm,
        vec![],
        &[
            ("clk", BIT),
            ("run", BIT),
            ("duty", Ty::u(12)),
            ("oc", BIT),
            ("sink_oc", BIT),
            ("hs", BIT),
            ("ls", BIT),
            ("sample", BIT),
            ("cut", BIT),
            ("dpwm.count", Ty::u(9)),
            ("dpwm.width", Ty::u(9)),
            ("dpwm.frame", Ty::u(3)),
            ("dpwm.hs_off", Ty::u(9)),
            ("dpwm.hs_cut", BIT),
            ("dpwm.ls_cut", BIT),
        ],
    );
    Built::new(w.finish())
}

/// Eight periods and a little: the ninth period's high side would start at the rising edge
/// 3600 (17 997.5 ns).
const STOP: f64 = 17.99e-6;

fn run_once() -> Seen {
    static BUILT: OnceLock<Mutex<Built>> = OnceLock::new();
    let built = BUILT.get_or_init(|| Mutex::new(dpwm_design())).lock().expect("the design is not poisoned");
    built.run(STOP, 1e-8)
}

/// The bit reversal of a 3-bit frame index.
fn reversed(k: i64) -> i64 {
    ((k & 1) << 2) | (k & 2) | ((k >> 2) & 1)
}

/// The width in counts of the `p`-th full period, `p` from 1: `duty / 8` counts, one more in
/// `duty % 8` periods of every 8, picked by the bit-reversed frame index (SPEC §5.2). The frame
/// counter is 0 at reset and counts the wraps; the width of a period is latched at the wrap
/// that starts it, from the counter value after that wrap (`dpwm.msl`: `frame + 1`).
fn width(duty: i64, p: i64) -> i64 {
    duty / 8 + i64::from(reversed(p % 8) < duty % 8)
}

#[test]
fn the_dpwm_dithers_keeps_dead_time_and_never_overlaps() {
    let seen = run_once();
    let duty = 1325;
    // SPEC §5.2: 1325 = 8 * 165 + 5: 166 counts in five periods of eight, 165 in three
    let widths: Vec<i64> = (1..=8).map(|p| width(duty, p)).collect();
    assert_eq!(widths.iter().filter(|w| **w == 166).count(), 5);
    assert_eq!(widths.iter().filter(|w| **w == 165).count(), 3);
    assert_eq!(widths.iter().sum::<i64>(), 8 * 165 + 5, "five extra counts over the frame");
    // SPEC §5.1 and §9: the first period (rising edges 1 to 399) has no duty word latched yet and
    // stays off; the counter wraps at rising edge 400 p, where the high side turns on and the
    // period's width is latched. The high side is on for `width` counts and the low side from
    // `width + 4` to 396 (SPEC §5.3)
    let mut hs = Vec::new();
    // the first period runs on the reset width, 0: no high side, and the low side from `0 + 4`
    // to 396 as for any width (SPEC §5.3)
    let mut ls = vec![(rise(DEAD), HIGH.to_vec()), (rise(RELEASE), LOW.to_vec())];
    let mut high_intervals = Vec::new();
    let mut low_intervals = Vec::new();
    for p in 1..=8 {
        let start = N * p;
        let w = widths[p as usize - 1];
        hs.push((rise(start), HIGH.to_vec()));
        hs.push((rise(start + w), LOW.to_vec()));
        ls.push((rise(start + w + DEAD), HIGH.to_vec()));
        ls.push((rise(start + RELEASE), LOW.to_vec()));
        high_intervals.push((rise(start), rise(start + w)));
        low_intervals.push((rise(start + w + DEAD), rise(start + RELEASE)));
    }
    assert_eq!(seen.transient("hs"), hs, "the high side: on at the wrap, off after `width` counts");
    assert_eq!(seen.transient("ls"), ls, "the low side: 4 counts after the high side, off at count 396");
    // SPEC §5.3: both commands are never on together, and each edge sees 4 counts (20 ns) of
    // dead time: after the high side falls, and before the next high side rises
    for (h, l) in high_intervals.iter().zip(&low_intervals) {
        assert!(h.1 <= l.0 && l.1 <= h.0 + N * COUNT, "the intervals do not overlap");
        assert_eq!(l.0 - h.1, DEAD * COUNT, "20 ns from the high side off to the low side on");
    }
    for (l, next) in low_intervals.iter().zip(high_intervals.iter().skip(1)) {
        assert_eq!(next.0 - l.1, DEAD * COUNT, "20 ns from the low side off to the next high side on");
    }
    // no current limit was asked: the `cut` flag never rose
    assert_eq!(seen.transient("cut"), vec![]);
}

#[test]
fn two_runs_give_the_same_changes() {
    // the oscillator and the DPWM circuit, run twice on the one compiled design: every
    // observation of the two runs is the same, its instant bit for bit
    let (first, second) = (run_once(), run_once());
    let (first, second) = (first.all(), second.all());
    assert!(first.len() > 8000, "the run has a few thousand instants: {}", first.len());
    assert_eq!(first, second);
}
