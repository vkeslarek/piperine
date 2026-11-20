//! Events from MSL source on the Rust backend (feature `unknown-semantics-events`): one test per
//! run-time criterion of `on settle` and of the settle commit, each model written in
//! `tests/events/`, elaborated by the front end, compiled on the transpiled backend and run on the
//! solver. Every expected value is written from the rule it cites (§13 rules 1, 14, 17; §15 step 4,
//! T4 and S1 to S7; §16 O4, rule 5), never read back from the model.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use piperine_api::{Field, Record, Streams};
use piperine_parity::project::Project;
use piperine_solver::prelude::{NodeIdentifier, Solver};
use piperine_solver::protocol::{AnalysisKind, Observation, ObservationKind, Sink};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;

use mixed::Built;
use support::build;

/// One microsecond and one nanosecond in ticks (one femtosecond each).
const US: i64 = 1_000_000_000;
const NS: i64 = 1_000_000;

/// The instant of an observation in ticks.
fn ticks(o: &Observation) -> i64 {
    (o.time * 1e15).round() as i64
}

/// Keeps every observation of a run, in the order the solver delivered them.
struct Tape(std::rc::Rc<std::cell::RefCell<Vec<Observation>>>);

impl Sink for Tape {
    fn observe(&mut self, o: &Observation) {
        self.0.borrow_mut().push(o.clone());
    }
}

/// What one run observed: every observation, in order, the accepted steps of a transient, and the
/// decoders of its records.
struct Run {
    observations: Vec<Observation>,
    steps: Vec<i64>,
    streams: Streams,
}

impl Run {
    /// The records of `stream` made in `analysis`: instant in ticks and the record.
    fn records(&self, analysis: AnalysisKind, stream: &str) -> Vec<(i64, Record)> {
        self.observations
            .iter()
            .filter(|o| o.analysis == analysis)
            .filter_map(|o| self.streams.decode(o).map(|r| (ticks(o), r.unwrap_or_else(|f| panic!("{f}")))))
            .filter(|(_, r)| r.stream == stream)
            .collect()
    }

    /// The instants of the records of `stream` made in `analysis`.
    fn instants(&self, analysis: AnalysisKind, stream: &str) -> Vec<i64> {
        self.records(analysis, stream).into_iter().map(|(t, _)| t).collect()
    }

    /// The accepted steps of the transient after its start, in ticks.
    fn time_points(&self) -> Vec<i64> {
        self.steps.iter().copied().filter(|t| *t > 0).collect()
    }

    /// The value changes of the net at `path`, the operating point's first: instant and bytes.
    fn changes(&self, path: &str) -> Vec<(i64, Vec<u8>)> {
        self.observations
            .iter()
            .filter(|o| o.path == path && !self.streams.paths().contains(&path))
            .filter_map(|o| match &o.kind {
                ObservationKind::Change { bytes } => Some((ticks(o), bytes.clone())),
                _ => None,
            })
            .collect()
    }

    /// The position in the run's observations of the first record of `stream` at `instant`.
    fn record_position(&self, stream: &str, instant: i64) -> usize {
        self.observations
            .iter()
            .position(|o| ticks(o) == instant && self.streams.decode(o).is_some_and(|r| r.is_ok_and(|r| r.stream == stream)))
            .unwrap_or_else(|| panic!("no `{stream}` record at {instant}"))
    }
}

/// A model of `tests/events/` compiled once, and the runs made of it.
struct Scenario {
    built: Built,
    streams: Streams,
}

impl Scenario {
    /// `top` of the file `file` of `tests/events/`, with the buck package as the crate `buck`.
    fn of(file: &str, top: &str) -> Scenario {
        let root: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/events").join(file);
        let buck = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/buck/src/lib.msl");
        let built = build(&Project::new(&root).dependency("buck", &buck), top);
        let streams = Streams::of(built.ir.clone());
        Scenario { built, streams }
    }

    /// A transient to `stop` seconds with at most `step` between time points.
    fn transient(&self, stop: f64, step: f64) -> Run {
        let (seen, result) = self.built.run_with(stop, step);
        let steps = result.iter().map(|s| (s.time() * 1e15).round() as i64).collect();
        Run { observations: seen.all(), steps, streams: self.streams.clone() }
    }

    /// `count` operating points in a row on one engine, as the points of a `Dc` sweep: every point
    /// but the last is followed by another (§15 S6).
    fn operating_points(&self, count: usize) -> Run {
        let circuit = self.built.compiled.circuit().unwrap_or_else(|e| panic!("{e}"));
        let tape = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut solver = Solver::new(circuit).with_sink(Box::new(Tape(tape.clone()))).build();
        for point in 0..count {
            let dc = solver.dc().unwrap_or_else(|e| panic!("{e}"));
            dc.followed_by_another_point(point + 1 < count).solve().unwrap_or_else(|e| panic!("{e}"));
        }
        drop(solver);
        let observations = tape.borrow().clone();
        Run { observations, steps: Vec::new(), streams: self.streams.clone() }
    }
}

/// The bytes of a known `Bit<f64>`: the value, its known byte and the padding of the 16-byte carrier.
fn known_real(value: f64) -> Vec<u8> {
    let mut bytes = value.to_le_bytes().to_vec();
    bytes.push(1);
    bytes.resize(16, 0);
    bytes
}

/// Whether two lists of instants in ticks are the same instants: an accepted step is the analog
/// side's own time, a record the tick of its instant, and the two round to ticks a tick apart at
/// most.
fn same_instants(left: &[i64], right: &[i64]) -> bool {
    left.len() == right.len() && left.iter().zip(right).all(|(a, b)| (a - b).abs() <= 1)
}

/// The real field `name` of a record.
fn real(record: &Record, name: &str) -> f64 {
    match record.fields.iter().find(|(field, _)| field == name) {
        Some((_, Field::Real(value))) => *value,
        other => panic!("no real field `{name}`: {other:?}"),
    }
}

#[test]
fn settle_runs_once_per_accepted_time_point() {
    // §15 step 4, §13 rule 17: the settle phase runs once at every time point, so a reaction on
    // `settle` writes one record at each accepted step of a transient (5 steps of at most 1 us)
    let run = Scenario::of("settle.msl", "TapBench").transient(5e-6, 1e-6);
    let steps = run.time_points();
    assert_eq!(steps.len(), 5, "five accepted steps: {steps:?}");
    assert_eq!(run.instants(AnalysisKind::Transient, "settled"), steps, "one record per accepted time point");
}

#[test]
fn settle_does_not_run_at_an_event_time_that_is_not_a_time_point() {
    // §15 T4: a clock edge nothing continuous reads is an instant but no time point, so `settle`
    // does not fire there: the records stay on the accepted steps although the clock changes
    // between them
    let run = Scenario::of("settle.msl", "ClockBench").transient(3e-6, 1e-6);
    let steps = run.time_points();
    let edges: Vec<i64> = run.changes("ClockBench.clk").into_iter().map(|(t, _)| t).filter(|t| *t > 0).collect();
    assert!(edges.iter().any(|t| !steps.contains(t)), "the clock changes between the steps: {edges:?} against {steps:?}");
    assert_eq!(run.instants(AnalysisKind::Transient, "settled"), steps, "settle ran at the time points only");
}

#[test]
fn settle_runs_at_the_operating_point_each_dc_point_and_the_last_instant() {
    // §16 O3 and §15 step 4: the settle phase runs at the operating point and at each point of a
    // `Dc` sweep, whose instants are time points; §16 rule 5: the instant that ends the analysis
    // (`finish()` at 10 ns) completes as a time point, so its settle phase runs and nothing after it
    let scenario = Scenario::of("settle.msl", "TapBench");
    let op = scenario.operating_points(1);
    assert_eq!(op.instants(AnalysisKind::OperatingPoint, "settled"), vec![0], "the operating point settles once");
    let sweep = scenario.operating_points(3);
    assert_eq!(sweep.instants(AnalysisKind::OperatingPoint, "settled"), vec![0, 0, 0], "each of three points settles once");
    let finished = Scenario::of("settle.msl", "FinishBench").transient(100e-9, 10e-9);
    assert_eq!(finished.instants(AnalysisKind::OperatingPoint, "settled"), vec![0]);
    assert_eq!(finished.instants(AnalysisKind::Transient, "settled"), vec![10 * NS], "the last instant settles and the run ends there");
}

#[test]
fn settle_records_follow_every_microstep_of_their_time_point() {
    // §19 rules 7 and 8: a clock edge sets `x` at microstep 1 and `x` sets `y` at microstep 2; `y`
    // drives the analog side, so the instant is a time point of three microsteps. The settle
    // record is stamped with the last of them, so it is delivered after every change of the instant
    let run = Scenario::of("settle.msl", "ChainBench").transient(3e-6, 1e-6);
    let at = run.changes("ChainBench.y").into_iter().find(|(t, bytes)| *t > 0 && bytes[0] == 1).expect("y rises").0;
    let record = run.record_position("settled", at);
    for net in ["ChainBench.x", "ChainBench.y"] {
        let change = run.observations.iter().position(|o| o.path == net && ticks(o) == at && matches!(o.kind, ObservationKind::Change { .. }));
        assert!(change.is_some_and(|c| c < record), "{net} changes before the record of its time point");
    }
}

#[test]
fn settle_combined_with_an_event_fires_by_rule_1() {
    // §13 rule 1: `on settle || clk.rise()` runs when either fires: at every time point and at
    // every rising edge, once each (no edge here is a time point)
    let run = Scenario::of("settle.msl", "EitherBench").transient(3e-6, 1e-6);
    let rises: Vec<i64> = run.changes("EitherBench.clk").into_iter().filter(|(_, bytes)| bytes[0] == 1).map(|(t, _)| t).collect();
    let mut wanted: Vec<i64> = run.time_points().into_iter().chain(rises).collect();
    wanted.sort_unstable();
    assert_eq!(run.instants(AnalysisKind::Transient, "either"), wanted, "the time points and the rising edges, each once");
}

#[test]
fn a_time_point_with_no_settle_reaction_adds_no_microstep_and_no_record() {
    // §15 step 4: a design no reaction of which is on `settle` has no settle phase: no record, and
    // the instants are those of the reactions alone. The same chain with a tap adds records only
    let plain = Scenario::of("settle.msl", "PlainBench").transient(3e-6, 1e-6);
    assert!(plain.observations.iter().all(|o| !matches!(o.kind, ObservationKind::Record { .. })), "no record at all");
    assert!(plain.observations.iter().all(|o| o.path != "$settle"), "SETTLE never fires: the design has no settle phase");
    let tapped = Scenario::of("settle.msl", "ChainBench").transient(3e-6, 1e-6);
    let without_records = |run: &Run| -> Vec<(String, i64)> {
        run.observations.iter().filter(|o| !matches!(o.kind, ObservationKind::Record { .. }) && !o.path.contains("tap") && o.path != "$settle").map(|o| (o.path.replace("Plain", "Chain"), ticks(o))).collect()
    };
    assert_eq!(without_records(&plain), without_records(&tapped), "the tap changes no instant and adds no microstep of the rest");
}

#[test]
fn settle_runs_once_per_time_point_across_a_jump() {
    // §15 step 4: a gate edge at 2.5 us, off the 1 us grid, is a jump of the analog side and a
    // time point of its own; `settle` runs once there, and once at each other accepted step
    let run = Scenario::of("settle.msl", "JumpBench").transient(6e-6, 1e-6);
    let steps = run.time_points();
    assert!(steps.contains(&(5 * US / 2)), "the rising edge at 2.5 us is a time point: {steps:?}");
    let settled = run.instants(AnalysisKind::Transient, "settled");
    assert!(same_instants(&settled, &steps), "once per time point, the jump's included: {settled:?} against {steps:?}");
}

#[test]
fn a_probing_settle_reaction_reads_the_time_point_solution() {
    // §15 step 4 and the reads table: a reaction on `settle` reads the solution at `t`, so the
    // voltage it writes at each time point is the one the analog side accepted there
    let scenario = Scenario::of("settle.msl", "ProbeBench");
    let (seen, result) = scenario.built.run_with(5e-6, 1e-6);
    let out = NodeIdentifier::Anonymous(scenario.built.ir.node_at("ProbeBench.b").expect("node b").id().index());
    let run = Run { observations: seen.all(), steps: Vec::new(), streams: scenario.streams.clone() };
    let seen: Vec<(i64, f64)> = run.records(AnalysisKind::Transient, "seen").into_iter().map(|(t, r)| (t, real(&r, "v"))).collect();
    assert_eq!(seen.len(), 5);
    for (t, v) in seen {
        let step = result.iter().find(|s| (s.time() * 1e15).round() as i64 == t).expect("a step at the record's instant");
        assert_eq!(v, step.get_node(&out).expect("v(b)"), "the record holds the accepted solution at {t}");
    }
}

#[test]
fn settle_in_a_design_with_nothing_analog_runs_at_the_operating_point_and_the_stop() {
    // §15 step 4, §16: with no unknown the instants between run on their own and are no time
    // point; `settle` runs at the operating point and at the stop time, the last instant
    let run = Scenario::of("settle.msl", "DigitalBench").transient(100e-9, 10e-9);
    assert_eq!(run.instants(AnalysisKind::OperatingPoint, "settled"), vec![0]);
    assert_eq!(run.instants(AnalysisKind::Transient, "settled"), vec![100 * NS]);
}

#[test]
fn a_settle_commit_is_read_at_the_next_time_point() {
    // §15 S1, S2, S6, §16 O4: the operating point's settle phase commits `r <- r0 (1 + v(ctl))` =
    // 2 kOhm from the 1 V control; the commit is due at the first time point after it (1 us), where
    // `changed()` fires, and the carrier does not change at the operating point itself
    let run = Scenario::of("commits.msl", "LagBench").transient(3e-6, 1e-6);
    let first = run.time_points()[0];
    assert_eq!(
        run.changes("LagBench.r"),
        vec![(0, known_real(1000.0)), (first, known_real(2000.0))],
        "the initial value at the operating point, then the commit at the first time point"
    );
}

#[test]
fn an_event_time_between_reads_the_value_before_the_settle_commit() {
    // §15 S4: a rising edge at 0.37 us lies between the operating point and the first time point
    // (1 us): it reads 1 kOhm although the operating point's settle phase committed 2 kOhm; the
    // edges from 1.11 us on read the landed 2 kOhm
    let run = Scenario::of("commits.msl", "BetweenBench").transient(3e-6, 1e-6);
    let first = run.time_points()[0];
    let reads: Vec<(i64, f64)> = run.records(AnalysisKind::Transient, "read").into_iter().map(|(t, r)| (t, real(&r, "r"))).collect();
    assert!(reads.len() >= 3, "edges at 0.37, 1.11 and 1.85 us at least: {reads:?}");
    for (t, r) in reads {
        let wanted = if t < first { 1000.0 } else { 2000.0 };
        assert_eq!(r, wanted, "the edge at {t} reads the value of its own time");
    }
}

#[test]
fn a_delayed_settle_commit_is_ordinary() {
    // §15 S7: `r <- 2k after 250n` from the operating point's settle phase is an ordinary delayed
    // commit: the carrier changes at 250 ns, not at the next time point (1 us)
    let run = Scenario::of("commits.msl", "SlowBench").transient(3e-6, 1e-6);
    assert_eq!(run.changes("SlowBench.r"), vec![(0, known_real(1000.0)), (250 * NS, known_real(2000.0))]);
}

#[test]
fn a_settle_commit_at_the_stop_time_is_dropped_from_source() {
    // §15 S6: `Late` commits at the settle phase of the time points after 2.5 us. The one made at the
    // stop time (3 us) has no time point to land at and is dropped: the carrier never changes. Run
    // to 4 us the commit of 3 us lands at 4 us, which shows the commit is made at all
    let scenario = Scenario::of("commits.msl", "LateBench");
    let short = scenario.transient(3e-6, 1e-6);
    assert_eq!(short.instants(AnalysisKind::Transient, "committed"), vec![3 * US], "the reaction commits at the stop time");
    assert_eq!(short.changes("LateBench.r"), vec![(0, known_real(1000.0))], "and the commit is dropped");
    let long = scenario.transient(4e-6, 1e-6);
    assert_eq!(long.changes("LateBench.r"), vec![(0, known_real(1000.0)), (4 * US, known_real(5000.0))], "run on, it lands at the next time point");
}
