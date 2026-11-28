//! The oscillator and the DPWM of the buck package run from MSL source (feature `msl-digital`,
//! T48; checks C130, C131, C134, DIG-14 AC 3, 4 and 7): `Osc` alone for 100 ns, then into `Dpwm`
//! with `run` true and the duty word 1325 for eight switching periods (`tests/buck/bench.msl`
//! places `buck::dpwm::{Osc, Dpwm}` from `examples/buck`, read from disk). The expected widths,
//! the dead times and the release count are computed here from `examples/buck/SPEC.md` §5 (a
//! 9-bit counter at 200 MHz wrapping at 400, duty word `8 * coarse + fine` with `fine` dithered
//! over a bit-reversed 3-bit frame counter, dead time 4 counts, the low side released at count
//! 396) and not read back from the model.
#![cfg(unix)]

use std::sync::{Mutex, OnceLock};

use piperine_ir_fixtures::digital;
use piperine_parity::mixed;
use piperine_ir_fixtures::mixed::TopDesign;

use mixed::{BuiltDesign, Seen};
use piperine_parity::buck_fixture::{BuckFixture, HIGH, LOW};

/// One clock count (5 ns) in ticks.
const COUNT: i64 = 5_000_000;
/// The counter wraps at N = 400 (SPEC §5.1); the dead time is 4 counts and the low side is
/// released at count 396 (SPEC §5.3).
const N: i64 = 400;
const DEAD: i64 = 4;
const RELEASE: i64 = N - DEAD;

#[test]
fn the_oscillator_toggles_every_half_period() {
    let seen = BuckFixture::build(&BuckFixture::with_buck("bench.msl"), "OscBench").run(100e-9, 1e-9);
    // 200 MHz: a period of 5 ns, a toggle every 2.5 ns, from false: true at 2.5 ns, false at
    // 5 ns, ... `k` = 1 to 40 inside 100 ns, and no change at any other instant
    let expected: Vec<(i64, Vec<u8>)> =
        (1..=40).map(|k| (k * 2_500_000, if k % 2 == 1 { HIGH.to_vec() } else { LOW.to_vec() })).collect();
    assert_eq!(BuckFixture::transient(&seen, "OscBench.clk"), expected);
}

/// Eight periods and a little: the ninth period's high side would start at the rising edge
/// 3600 (17 997.5 ns).
const STOP: f64 = 17.99e-6;

/// Associated test helpers of `buck_dpwm`, grouped on one fixture type.
struct BuckDpwmFixture;

impl BuckDpwmFixture {
    fn run_once() -> Seen {
        static BUILT: OnceLock<Mutex<BuiltDesign>> = OnceLock::new();
        let built =
            BUILT.get_or_init(|| Mutex::new(BuckFixture::build(&BuckFixture::with_buck("bench.msl"), "DpwmBench"))).lock().expect("the design is not poisoned");
        built.run(STOP, 1e-8)
    }

    /// The bit reversal of a 3-bit frame index.
    fn reversed(k: i64) -> i64 {
        ((k & 1) << 2) | (k & 2) | ((k >> 2) & 1)
    }

    /// The width in counts of the `p`-th full period, `p` from 1: `duty / 8` counts, one more in
    /// `duty % 8` periods of every 8, picked by the bit-reversed frame index (SPEC §5.2). The frame
    /// counter is 0 at reset and counts the wraps; the width of a period is latched at the wrap
    /// that starts it, from the counter value after that wrap.
    fn width(duty: i64, p: i64) -> i64 {
        duty / 8 + i64::from(BuckDpwmFixture::reversed(p % 8) < duty % 8)
    }
}

#[test]
fn the_dpwm_dithers_keeps_dead_time_and_never_overlaps() {
    let seen = BuckDpwmFixture::run_once();
    let duty = 1325;
    // SPEC §5.2: 1325 = 8 * 165 + 5: 166 counts in five periods of eight, 165 in three
    let widths: Vec<i64> = (1..=8).map(|p| BuckDpwmFixture::width(duty, p)).collect();
    assert_eq!(widths.iter().filter(|w| **w == 166).count(), 5);
    assert_eq!(widths.iter().filter(|w| **w == 165).count(), 3);
    assert_eq!(widths.iter().sum::<i64>(), 8 * 165 + 5, "five extra counts over the frame");
    // SPEC §5.1 and §9: the first period (rising edges 1 to 399) has no duty word latched yet and
    // stays off; the counter wraps at rising edge 400 p, where the high side turns on and the
    // period's width is latched. The high side is on for `width` counts and the low side from
    // `width + 4` to 396 (SPEC §5.3). The first period runs on the reset width, 0: no high side,
    // and the low side from `0 + 4` to 396 as for any width.
    let mut hs = Vec::new();
    let mut ls = vec![(TopDesign::rising_edge(DEAD), HIGH.to_vec()), (TopDesign::rising_edge(RELEASE), LOW.to_vec())];
    let mut high_intervals = Vec::new();
    let mut low_intervals = Vec::new();
    for p in 1..=8 {
        let start = N * p;
        let w = widths[p as usize - 1];
        hs.push((TopDesign::rising_edge(start), HIGH.to_vec()));
        hs.push((TopDesign::rising_edge(start + w), LOW.to_vec()));
        ls.push((TopDesign::rising_edge(start + w + DEAD), HIGH.to_vec()));
        ls.push((TopDesign::rising_edge(start + RELEASE), LOW.to_vec()));
        high_intervals.push((TopDesign::rising_edge(start), TopDesign::rising_edge(start + w)));
        low_intervals.push((TopDesign::rising_edge(start + w + DEAD), TopDesign::rising_edge(start + RELEASE)));
    }
    assert_eq!(BuckFixture::transient(&seen, "DpwmBench.hs"), hs, "the high side: on at the wrap, off after `width` counts");
    assert_eq!(BuckFixture::transient(&seen, "DpwmBench.ls"), ls, "the low side: 4 counts after the high side, off at count 396");
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
    assert_eq!(BuckFixture::transient(&seen, "DpwmBench.cut"), vec![]);
}

#[test]
fn two_runs_give_the_same_changes() {
    // the oscillator and the DPWM circuit, run twice on the one compiled design: every
    // observation of the two runs is the same, its instant bit for bit
    let (first, second) = (BuckDpwmFixture::run_once().all(), BuckDpwmFixture::run_once().all());
    assert!(first.len() > 8000, "the run has a few thousand instants: {}", first.len());
    assert_eq!(first, second);
}
