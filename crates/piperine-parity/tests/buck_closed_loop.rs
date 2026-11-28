//! The closed loop of the buck converter from MSL source (feature `msl-digital`, T57; checks
//! C151 to C154, DIG-16): `LoopBench` of `tests/buck/closed_loop.msl` places the whole `Buck` of
//! `examples/buck`, writes the setpoint code 0 (0.8 V) and raises `en`, at Vin 12 V into the
//! 6.6 ohm load, and runs 600 us: 128 samples of soft-start ramp, 64 of power-good delay and the
//! rest in regulation (`examples/buck/SPEC.md` §7.2, §8.6). One run serves the four tests. A sink
//! of this file keeps only what they read: the changes of the supervisor's and the ramp's nets,
//! and the records of the DPWM's `Never` monitor. Expected values come from SPEC §2 (the
//! regulation bound), §7 and §8, never from a run.
#![cfg(unix)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;

use piperine_solver::prelude::{NodeIdentifier, Solver, TransientAnalysisOptions};
use piperine_solver::protocol::{AnalysisKind, Observation, ObservationKind, Sink};

use piperine_ir_fixtures::digital;
use piperine_parity::mixed;
use piperine_ir_fixtures::mixed::TopDesign;

use piperine_parity::buck_fixture::{BuckProject, ByteInteger, NetChanges};
use piperine_parity::mixed::BuiltDesign;

/// The run: 600 us (DIG-16).
const STOP: f64 = 600e-6;
/// The analog step cap, as the other buck benches use.
const STEP: f64 = 1e-8;
/// One switching period, 2 us (SPEC §2: 500 kHz).
const PERIOD: f64 = 2e-6;

/// The nets the tests read, by path.
const NETS: [&str; 6] = [
    "LoopBench.mode",
    "LoopBench.fault",
    "LoopBench.pgood",
    "LoopBench.buck.ctl.reference",
    "LoopBench.buck.ctl.setpoint",
    "LoopBench.en",
];
/// The `Never` monitor of the DPWM: a child instance, its records on `<instance>.checks`.
const NEVER: &str = "LoopBench.buck.ctl.pwm.no_shoot_through";

/// What the run gave, as plain data.
struct Outcome {
    /// Every change of each net of `NETS`, in order: instant in ticks and bytes.
    nets: Vec<(String, Vec<(i64, Vec<u8>)>)>,
    /// The `checks` records of the monitor: instant in ticks and bytes, with the texts of
    /// their site.
    checks: Vec<(i64, Vec<u8>, Vec<(String, String)>)>,
    /// `v(vout)` at every accepted time.
    vout: Vec<(f64, f64)>,
    /// The wall-clock time of the run, in seconds.
    seconds: f64,
}

impl Outcome {
    fn series(&self, path: &str) -> &[(i64, Vec<u8>)] {
        &self.nets.iter().find(|(p, _)| p == path).unwrap_or_else(|| panic!("no net {path}")).1
    }
}

/// The kept observations.
#[derive(Default)]
struct Kept {
    nets: Vec<(String, Vec<(i64, Vec<u8>)>)>,
    checks: Vec<(i64, Vec<u8>, Vec<(String, String)>)>,
}

struct SeenSink {
    ir: std::sync::Arc<piperine_ir::Ir>,
    kept: Rc<RefCell<Kept>>,
}

/// The closed-loop bench of the buck, run once for every test.
struct ClosedLoopRun;

impl ClosedLoopRun {
    /// The one run of this test binary.
    fn outcome() -> &'static Outcome {
        static RUN: OnceLock<Outcome> = OnceLock::new();
        RUN.get_or_init(|| {
            let built = BuiltDesign::of(&BuckProject::with_buck("closed_loop.msl"), "LoopBench");
            let vout = NodeIdentifier::Anonymous(built.ir.node_at("LoopBench.buck.vout").expect("the output node").id().index());
            let kept = Rc::new(RefCell::new(Kept { nets: NETS.iter().map(|p| (p.to_string(), Vec::new())).collect(), ..Kept::default() }));
            let circuit = built.compiled.circuit().unwrap_or_else(|e| panic!("{e}"));
            let mut options = TransientAnalysisOptions::new(STOP, STEP);
            options.dt_max = STEP;
            // only the last 20 periods are read as analog values: earlier steps are not kept (each
            // kept step holds every unknown; the whole run would hold hundreds of thousands)
            options.record_from = STOP - 20.0 * PERIOD;
            let sink = SeenSink { ir: built.ir.clone(), kept: kept.clone() };
            let mut solver = Solver::new(circuit).with_tran_opts(options).with_sink(Box::new(sink)).build();
            let start = std::time::Instant::now();
            let result = solver.tran().unwrap_or_else(|e| panic!("{e}")).solve().unwrap_or_else(|e| panic!("the closed loop does not run: {e}"));
            let seconds = start.elapsed().as_secs_f64();
            let vout = result.iter().map(|s| (s.time(), s.get_node(&vout).expect("v(vout)"))).collect();
            let kept = kept.take();
            eprintln!("closed loop: {STOP:e} s simulated in {seconds:.1} s of wall clock");
            Outcome { nets: kept.nets, checks: kept.checks, vout, seconds }
        })
    }
}

/// Seconds in ticks and the last value of a path of a closed-loop run.
struct ClosedLoopReading;

impl ClosedLoopReading {
    fn ticks(seconds: f64) -> i64 {
        (seconds * 1e15).round() as i64
    }

    /// The value of a net of `bytes` value bytes at the end of the run.
    fn last(o: &Outcome, path: &str, bytes: usize) -> i32 {
        ByteInteger::decode(NetChanges::value_at(o.series(path), ClosedLoopReading::ticks(STOP)), bytes, false)
    }
}

impl Sink for SeenSink {
    fn observe(&mut self, o: &Observation) {
        let mut kept = self.kept.borrow_mut();
        match &o.kind {
            ObservationKind::Change { bytes } => {
                if let Some((_, s)) = kept.nets.iter_mut().find(|(p, _)| *p == o.path) {
                    s.push((ClosedLoopReading::ticks(o.time), bytes.clone()));
                }
            }
            ObservationKind::Record { site: Some((reaction, statement)), bytes }
                if o.path == format!("{NEVER}.checks") && o.analysis == AnalysisKind::Transient =>
            {
                // the record names its own site (abi.md C5): decoded on its own, no pairing
                let (_, site) = self.ir.record_at(&o.path, *reaction, *statement).expect("a record decodes to its emit");
                let texts = site.texts.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
                kept.checks.push((ClosedLoopReading::ticks(o.time), bytes.clone(), texts));
            }
            _ => {}
        }
    }
}

#[test]
fn the_buck_starts_and_reaches_regulate() {
    let o = ClosedLoopRun::outcome();
    // SPEC §7.1: the setpoint register takes the code 0 on the host's write
    assert_eq!(ClosedLoopReading::last(o, "LoopBench.buck.ctl.setpoint", 1), 0, "the setpoint is code 0");
    // SPEC §8: `Off` until enabled, `SoftStart` at the next sample, `Regulate` when the ramp
    // reaches the target; no `Hiccup` (tags 0, 1, 2, 3)
    let modes: Vec<i32> = o.series("LoopBench.mode").iter().map(|(_, b)| ByteInteger::decode(b, 1, false)).collect();
    let mut distinct = modes.clone();
    distinct.dedup();
    assert_eq!(distinct, vec![0, 1, 2], "mode goes Off, SoftStart, Regulate: {modes:?}");
    // SPEC §7.1 and §7.2: the reference ramps one code per sample to 128 + 4 * 0 = 128 and stays
    let reference = o.series("LoopBench.buck.ctl.reference");
    assert_eq!(ClosedLoopReading::last(o, "LoopBench.buck.ctl.reference", 2), 128, "the reference rests at 128");
    let peak = reference.iter().map(|(_, b)| ByteInteger::decode(b, 2, false)).max();
    assert_eq!(peak, Some(128), "it never passes the target");
}

#[test]
fn power_good_is_high_and_no_fault_is_latched() {
    let o = ClosedLoopRun::outcome();
    // DIG-16 AC 2, SPEC §8.5 and §8.6: at 600 us power-good is high and the fault code is `Clear`
    assert_eq!(ClosedLoopReading::last(o, "LoopBench.pgood", 1), 1, "pgood is true at 600 us");
    assert_eq!(ClosedLoopReading::last(o, "LoopBench.fault", 1), 0, "fault is Clear at 600 us");
    assert!(o.series("LoopBench.fault").iter().all(|(_, b)| ByteInteger::decode(b, 1, false) == 0), "no fault was ever latched");
}

#[test]
fn the_output_settles_within_10_mv_of_0v8() {
    let o = ClosedLoopRun::outcome();
    // DIG-16 AC 3: the mean of v(vout) over the last 20 switching periods, each accepted point
    // weighted by the step that ends at it
    let from = STOP - 20.0 * PERIOD;
    let (mut area, mut span) = (0.0, 0.0);
    for w in o.vout.windows(2) {
        let ((t0, _), (t1, v1)) = (w[0], w[1]);
        if t0 >= from {
            area += v1 * (t1 - t0);
            span += t1 - t0;
        }
    }
    let mean = area / span;
    // SPEC §2: DC regulation ±1 % or ±10 mV, whichever is larger: at 0.8 V, 1 % is 8 mV, so the
    // bound is 10 mV
    let bound = (0.01 * 0.8_f64).max(10e-3);
    eprintln!("closed loop: mean v(vout) over the last 20 periods = {mean:.6} V ({:.0} s wall clock)", o.seconds);
    assert!((mean - 0.8).abs() <= bound, "mean v(vout) = {mean} V, outside 0.8 V ± {bound} V");
}

#[test]
fn no_shoot_through_during_the_start() {
    let o = ClosedLoopRun::outcome();
    // SPEC §5.3 and §10 S10: the monitor checks at each rising clock edge (200 MHz) that both
    // commands are never on together: one record per edge, `pass` true
    // the 200 MHz clock rises at 2.5 ns + 5 ns (k - 1): 600 us holds edges 1 to 120 000, the
    // last at 599.9975 us. Exactly one record at each, none elsewhere
    assert_eq!(o.checks.len(), 120_000, "one record per rising edge of 600 us of 200 MHz");
    let made: Vec<i64> = o.checks.iter().map(|(t, _, _)| *t).collect();
    let edges: Vec<i64> = (1..=120_000).map(TopDesign::rising_edge).collect();
    assert!(made == edges, "the records are made at the rising edges: first difference at {:?}", made.iter().zip(&edges).position(|(a, b)| a != b));
    let never = vec![("kind".to_string(), "require".to_string()), ("name".to_string(), "never".to_string())];
    assert!(o.checks.iter().all(|(_, _, t)| *t == never), "every record is the `never` check");
    let failed: Vec<_> = o.checks.iter().filter(|(_, b, _)| b[0] != 1).take(5).collect();
    assert!(failed.is_empty(), "a check failed: {failed:?}");
}
