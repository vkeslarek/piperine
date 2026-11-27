//! The supervisor of the buck package runs from MSL source (feature `msl-digital`, T55; checks
//! C147 and C148, DIG-15 AC 4 and 5). `Script` of `tests/buck/supervisor.msl` walks 2400
//! samples: enable, soft-start to the target, power-good in and out of its window, a lone
//! over-voltage sample, seven and then eight cut periods, the hiccup count and its retry, two
//! over-voltage samples, a brown-out during the second hiccup, and a disable from `Regulate`.
//! `Supervisor` of `examples/buck` runs under it on the 200 MHz oscillator. The expected `mode`,
//! `fault`, `run` and `pgood` come from a plain Rust state machine written from
//! `examples/buck/SPEC.md` §8 (the state table and §8.1 to §8.6), not from `supervisor.msl`, and
//! are compared with the values the solver committed at every sample.
#![cfg(unix)]

use piperine_parity::project::Project;
use piperine_solver::protocol::{AnalysisKind, ObservationKind};

use piperine_ir_fixtures::digital;
#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
use piperine_ir_fixtures::mixed::TopDesign;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;

use mixed::Seen;
use support::{build, decode, series, value_at, with_buck};

/// The samples: one per rising edge of the oscillator.
const SAMPLES: i64 = 2400;

fn project() -> Project {
    with_buck("supervisor.msl")
}

/// The inputs of one sample, as `Script` of `tests/buck/supervisor.msl` computes them from its
/// counter; the test asserts the two agree before it judges the supervisor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Inputs {
    en: bool,
    vin_ok: bool,
    cut: bool,
    meas: i32,
    reference: i32,
    at_target: bool,
}

fn script(s: i64) -> Inputs {
    let meas = if (120..123).contains(&s) || (130..134).contains(&s) {
        100
    } else if s == 220 || s == 1298 || s == 1299 {
        170
    } else {
        128
    };
    Inputs {
        en: !(s < 5 || (2300..2305).contains(&s)),
        vin_ok: !(1500..1505).contains(&s),
        cut: (230..237).contains(&s) || (250..258).contains(&s),
        meas,
        reference: 128,
        at_target: s >= 20 && !(1500..1520).contains(&s),
    }
}

/// SPEC §8: the modes, by the tag a `Bit<Mode>` carries (declaration order).
const OFF: i32 = 0;
const SOFT_START: i32 = 1;
const REGULATE: i32 = 2;
const HICCUP: i32 = 3;
/// SPEC §8.5: the fault codes.
const CLEAR: i32 = 0;
const OVER_CURRENT: i32 = 1;
const OVER_VOLTAGE: i32 = 2;

/// What the supervisor shows after one sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Shown {
    mode: i32,
    fault: i32,
    run: bool,
    pgood: bool,
}

/// The state machine of SPEC §8, one step per sample.
struct Reference {
    mode: i32,
    fault: i32,
    pgood: bool,
    /// Consecutive cut periods (§8.2) and samples over the OVP threshold (§8.4).
    cut_run: i32,
    over_run: i32,
    /// Samples still to count in `Hiccup` before the retry (§8.5).
    hiccup_left: i32,
    /// Consecutive samples in, and out of, the power-good window (§8.6).
    inside_run: i32,
    outside_run: i32,
}

impl Reference {
    fn new() -> Reference {
        // SPEC §8.5, §8.6: the converter powers up `Off`, no fault, power-good low
        Reference { mode: OFF, fault: CLEAR, pgood: false, cut_run: 0, over_run: 0, hiccup_left: 0, inside_run: 0, outside_run: 0 }
    }

    fn step(&mut self, i: Inputs) -> Shown {
        // §8.1: UVLO and enable together are "the enabled state"
        let enabled = i.en && i.vin_ok;
        // §8.2: 8 consecutive cut periods are a hard overcurrent
        self.cut_run = if i.cut { (self.cut_run + 1).min(8) } else { 0 };
        // §8.4: 2 consecutive samples above ref + ref / 8 + 16
        let threshold = i.reference + i.reference / 8 + 16;
        self.over_run = if i.meas > threshold { (self.over_run + 1).min(2) } else { 0 };
        let fault_now = if self.cut_run >= 8 {
            Some(OVER_CURRENT)
        } else if self.over_run >= 2 {
            Some(OVER_VOLTAGE)
        } else {
            None
        };
        let was = self.mode;
        if !enabled {
            // the state table: leaving the enabled state goes to `Off` from any mode, at once;
            // §8.1: UVLO is not a fault, the fault code is untouched
            self.mode = OFF;
        } else {
            match was {
                OFF => self.mode = SOFT_START,
                HICCUP => {
                    // §8.5: count 1000 samples, then retry through `SoftStart`
                    if self.hiccup_left == 0 {
                        self.mode = SOFT_START;
                    } else {
                        self.hiccup_left -= 1;
                    }
                }
                _ => {
                    if let Some(f) = fault_now {
                        // §8.2, §8.4: `Hiccup` from `SoftStart` or `Regulate`; §8.5: the fault code
                        // is latched on entry
                        self.mode = HICCUP;
                        self.fault = f;
                        // SPEC §8.5: 1000 samples of `Hiccup` counting the entry's own, so 999 are
                        // left to count
                        self.hiccup_left = 999;
                    } else if was == SOFT_START && i.at_target {
                        // the state table: `Regulate` when the ramp reaches the target; §8.5: the
                        // fault code is cleared when a start reaches `Regulate`
                        self.mode = REGULATE;
                        self.fault = CLEAR;
                    }
                }
            }
        }
        // §8.6: the window is ref ± ref / 16; assert after 64 consecutive in-window samples in
        // `Regulate`, deassert after 4 consecutive samples outside it, or at once on leaving
        // `Regulate`
        let regulating = was == REGULATE && enabled && fault_now.is_none();
        let half = i.reference / 16;
        let inside = i.meas >= i.reference - half && i.meas <= i.reference + half;
        self.inside_run = if regulating && inside { (self.inside_run + 1).min(64) } else { 0 };
        self.outside_run = if regulating && !inside { (self.outside_run + 1).min(4) } else { 0 };
        if !regulating {
            self.pgood = false;
        } else if self.inside_run >= 64 {
            self.pgood = true;
        } else if self.outside_run >= 4 {
            self.pgood = false;
        }
        // the state table: the converter switches (`run`) in `SoftStart` and `Regulate`
        Shown { mode: self.mode, fault: self.fault, run: self.mode == SOFT_START || self.mode == REGULATE, pgood: self.pgood }
    }
}

fn model() -> Vec<Shown> {
    let mut r = Reference::new();
    (0..SAMPLES).map(|s| r.step(script(s))).collect()
}

/// The bench compiled and run over 2400 rising edges at 5 ns.
fn run() -> (mixed::Built, Seen) {
    let built = build(&project(), "SupervisorBench");
    let seen = built.run(TopDesign::rising_edge(SAMPLES) as f64 * 1e-15 + 1e-9, 1e-8);
    (built, seen)
}

/// What the solver committed, read at each sample's rising edge.
fn observed(seen: &Seen) -> Vec<Shown> {
    let (mode, fault, run, pgood) = (
        series(seen, "SupervisorBench.mode"),
        series(seen, "SupervisorBench.fault"),
        series(seen, "SupervisorBench.run"),
        series(seen, "SupervisorBench.pgood"),
    );
    (0..SAMPLES)
        .map(|s| {
            let at = TopDesign::rising_edge(s + 1);
            Shown {
                mode: decode(value_at(&mode, at), 1, false),
                fault: decode(value_at(&fault, at), 1, false),
                run: decode(value_at(&run, at), 1, false) != 0,
                pgood: decode(value_at(&pgood, at), 1, false) != 0,
            }
        })
        .collect()
}

/// The script in MSL and its Rust twin give the same inputs at every sample.
fn assert_inputs(seen: &Seen) {
    let read = |name: &str| series(seen, &format!("SupervisorBench.{name}"));
    let (en, vin_ok, cut, meas, reference, at_target) =
        (read("en"), read("vin_ok"), read("cut"), read("meas"), read("reference"), read("at_target"));
    for s in 0..SAMPLES {
        let at = TopDesign::rising_edge(s + 1) - 1;
        let got = Inputs {
            en: decode(value_at(&en, at), 1, false) != 0,
            vin_ok: decode(value_at(&vin_ok, at), 1, false) != 0,
            cut: decode(value_at(&cut, at), 1, false) != 0,
            meas: decode(value_at(&meas, at), 2, false),
            reference: decode(value_at(&reference, at), 2, false),
            at_target: decode(value_at(&at_target, at), 1, false) != 0,
        };
        assert_eq!(got, script(s), "the inputs of sample {s}");
    }
}

#[test]
fn the_supervisor_follows_its_state_table() {
    let expected = model();
    // the script reaches every row of the table, so a supervisor that did nothing could not match
    let modes = |m: i32| expected.iter().filter(|x| x.mode == m).count();
    assert!(modes(OFF) > 0 && modes(SOFT_START) > 0 && modes(REGULATE) > 0 && modes(HICCUP) > 1000);
    assert!(expected.iter().any(|x| x.fault == OVER_CURRENT) && expected.iter().any(|x| x.fault == OVER_VOLTAGE));
    // a brown-out during the second hiccup leaves the fault code latched in `Off` (§8.1)
    assert!(expected.iter().any(|x| x.mode == OFF && x.fault == OVER_VOLTAGE));

    let (built, seen) = run();
    assert_inputs(&seen);
    for (sample, (want, got)) in expected.iter().zip(&observed(&seen)).enumerate() {
        assert_eq!(want, got, "sample {sample}");
    }

    // §8.5 and `std/io.msl`: the hiccup entry writes one `log` record of severity `Warning` (1);
    // the reaction and the statement the record carries give its site (`Ir::record_at`)
    let entries: Vec<i64> = (0..SAMPLES as usize)
        .filter(|&s| expected[s].mode == HICCUP && (s == 0 || expected[s - 1].mode != HICCUP))
        .map(|s| TopDesign::rising_edge(s as i64 + 1))
        .collect();
    assert_eq!(entries.len(), 2, "the script enters `Hiccup` twice");
    let all = seen.all();
    let decoded: Vec<(i64, Vec<u8>, Vec<(String, String)>)> = all
        .iter()
        .filter(|o| o.analysis == AnalysisKind::Transient && o.path == "SupervisorBench.sup.log")
        .filter_map(|o| match &o.kind {
            ObservationKind::Record { site, bytes } => {
                let (reaction, statement) = site.expect("a record of an emit names its site");
                let (instance, site) = built.ir.record_at(&o.path, reaction, statement).expect("a record decodes to its emit");
                assert_eq!(instance.path(), "SupervisorBench.sup");
                let texts = site.texts.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
                Some(((o.time * 1e15).round() as i64, bytes.clone(), texts))
            }
            _ => None,
        })
        .collect();
    let records: Vec<(i64, Vec<u8>)> = decoded.iter().map(|(t, b, _)| (*t, b.clone())).collect();
    assert_eq!(records.iter().map(|(t, _)| *t).collect::<Vec<_>>(), entries, "one record per hiccup entry");
    assert!(records.iter().all(|(_, b)| b[0] == 1), "severity Warning: {records:?}");
    let texts: Vec<Vec<(String, String)>> = decoded.into_iter().map(|(_, _, t)| t).collect();
    let message = vec![("msg".to_string(), "buck: fault, entering hiccup".to_string())];
    assert_eq!(texts, vec![message.clone(), message], "the text of each record");
}

#[test]
fn power_good_asserts_after_64_and_drops_after_4() {
    let expected = model();
    // `Regulate` is first in force at sample 21 (entered at 20, when `at_target` rises): the 64th
    // consecutive in-window sample is 84 (§8.6)
    let rises: Vec<usize> = (1..SAMPLES as usize).filter(|&s| expected[s].pgood && !expected[s - 1].pgood).collect();
    let falls: Vec<usize> = (1..SAMPLES as usize).filter(|&s| !expected[s].pgood && expected[s - 1].pgood).collect();
    assert_eq!(expected[20].mode, REGULATE);
    assert_eq!(rises.first(), Some(&84));
    // three samples outside (120 to 122) do not drop it; the 4th consecutive (133) does; it comes
    // back at the 64th in-window sample after that (134 + 63)
    assert_eq!(falls.first(), Some(&133));
    assert_eq!(rises.get(1), Some(&197));
    // it drops at once on leaving `Regulate`: the overcurrent entry at 257
    assert_eq!(falls.get(1), Some(&257));

    let (_, seen) = run();
    let pgood = series(&seen, "SupervisorBench.pgood");
    let at = |s: usize| decode(value_at(&pgood, TopDesign::rising_edge(s as i64 + 1)), 1, false) != 0;
    for s in [83, 84, 122, 132, 133, 196, 197, 256, 257] {
        assert_eq!(at(s), expected[s].pgood, "pgood at sample {s}");
    }
    for s in 0..SAMPLES as usize {
        assert_eq!(at(s), expected[s].pgood, "pgood at sample {s}");
    }
}

#[test]
fn each_hiccup_lasts_1000_samples() {
    // BS-09 AC 1, SPEC §8.5: `Hiccup` lasts 1000 samples (2 ms at 500 kHz), so the retry's
    // `SoftStart` shows at the 1000th sample after the entry. The count is taken from the
    // supervisor's own observed run against SPEC's number, not against `Reference`, which
    // carried the same off-by-one as the model (a hiccup of 1001 samples) and so could not
    // see it.
    let (_, seen) = run();
    let shown = observed(&seen);
    let entries: Vec<usize> = (1..shown.len()).filter(|&s| shown[s].mode == HICCUP && shown[s - 1].mode != HICCUP).collect();
    assert_eq!(entries.len(), 2, "the script enters `Hiccup` twice");
    // a hiccup ends in `SoftStart`, or is cut short by a brown-out into `Off` (the second one is,
    // at sample 1500); only the first reaches its retry
    let lengths: Vec<usize> = entries
        .iter()
        .filter_map(|&entry| shown[entry..].iter().position(|s| s.mode != HICCUP).filter(|&length| shown[entry + length].mode == SOFT_START))
        .collect();
    assert_eq!(lengths, vec![1000], "samples from each entry into `Hiccup` to the retry's `SoftStart`");
}
