//! Events from MSL source on the Rust backend (feature `unknown-semantics-events`): one test per
//! run-time criterion of `on settle` and of the settle commit, each model written in
//! `tests/events/`, elaborated by the front end, compiled on the transpiled backend and run on the
//! solver. Every expected value is written from the rule it cites (§13 rules 1, 14, 17; §15 step 4,
//! T4 and S1 to S7; §16 O4, rule 5) and, for an event read as a `bool`, §13 rule 2 and rule 9,
//! never read back from the model.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use piperine_api::{Field, Record, Streams};
use piperine_parity::project::Project;
use piperine_solver::prelude::{NodeIdentifier, Solver};
use piperine_solver::protocol::{AnalysisKind, Observation, ObservationKind, Sink};

use piperine_parity::mixed;
#[allow(dead_code)]
use crate::support;

use mixed::BuiltDesign;
use support::build;

/// One microsecond and one nanosecond in ticks (one femtosecond each).
const US: i64 = 1_000_000_000;
const NS: i64 = 1_000_000;

/// Keeps every observation of a run, in the order the solver delivered them. The field is the shared
/// buffer the sink appends to and the run reads back.
struct Tape(std::rc::Rc<std::cell::RefCell<Vec<Observation>>>);

impl Sink for Tape {
    fn observe(&mut self, o: &Observation) {
        self.0.borrow_mut().push(o.clone());
    }
}

/// What one run observed: every observation, in order, the accepted steps of a transient, and the
/// decoders of its records.
struct Run {
    /// Every observation, in the order the solver delivered them.
    observations: Vec<Observation>,
    /// The accepted steps of the transient, in ticks (empty for operating points).
    steps: Vec<i64>,
    /// The decoders of the model's record streams.
    streams: Streams,
}

impl Run {
    /// The records of `stream` made in `analysis`: instant in ticks and the record.
    fn records(&self, analysis: AnalysisKind, stream: &str) -> Vec<(i64, Record)> {
        self.observations
            .iter()
            .filter(|o| o.analysis == analysis)
            .filter_map(|o| self.streams.decode(o).map(|r| (Run::tick(o), r.unwrap_or_else(|f| panic!("{f}")))))
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
                ObservationKind::Change { bytes } => Some((Run::tick(o), bytes.clone())),
                _ => None,
            })
            .collect()
    }

    /// The position in the run's observations of the first record of `stream` at `instant`.
    fn record_position(&self, stream: &str, instant: i64) -> usize {
        self.observations
            .iter()
            .position(|o| Run::tick(o) == instant && self.streams.decode(o).is_some_and(|r| r.is_ok_and(|r| r.stream == stream)))
            .unwrap_or_else(|| panic!("no `{stream}` record at {instant}"))
    }

    /// The instant of an observation in ticks.
    fn tick(o: &Observation) -> i64 {
        (o.time * 1e15).round() as i64
    }

    /// The bytes of a `Bit<f64>`: its eight value bytes.
    fn known_real(value: f64) -> Vec<u8> {
        value.to_le_bytes().to_vec()
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

    /// The bool field `name` of a record.
    fn flag(record: &Record, name: &str) -> bool {
        match record.fields.iter().find(|(field, _)| field == name) {
            Some((_, Field::Bool(value))) => *value,
            other => panic!("no bool field `{name}`: {other:?}"),
        }
    }

    /// The instants and the two bool fields `first` and `second` of the records of `stream`.
    fn flags(&self, stream: &str, first: &str, second: &str) -> Vec<(i64, bool, bool)> {
        self.records(AnalysisKind::Transient, stream).into_iter().map(|(t, r)| (t, Run::flag(&r, first), Run::flag(&r, second))).collect()
    }
}

/// A model of `tests/events/` compiled once, and the runs made of it.
struct Scenario {
    /// The model compiled on the transpiled backend.
    built: BuiltDesign,
    /// The decoders of the model's record streams, handed to each run.
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
        let change = run.observations.iter().position(|o| o.path == net && Run::tick(o) == at && matches!(o.kind, ObservationKind::Change { .. }));
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
        run.observations.iter().filter(|o| !matches!(o.kind, ObservationKind::Record { .. }) && !o.path.contains("tap") && o.path != "$settle").map(|o| (o.path.replace("Plain", "Chain"), Run::tick(o))).collect()
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
    assert!(Run::same_instants(&settled, &steps), "once per time point, the jump's included: {settled:?} against {steps:?}");
}

#[test]
fn a_probing_settle_reaction_reads_the_time_point_solution() {
    // §15 step 4 and the reads table: a reaction on `settle` reads the solution at `t`, so the
    // voltage it writes at each time point is the one the analog side accepted there
    let scenario = Scenario::of("settle.msl", "ProbeBench");
    let (seen, result) = scenario.built.run_with(5e-6, 1e-6);
    let out = NodeIdentifier::Anonymous(scenario.built.ir.node_at("ProbeBench.b").expect("node b").id().index());
    let run = Run { observations: seen.all(), steps: Vec::new(), streams: scenario.streams.clone() };
    let seen: Vec<(i64, f64)> = run.records(AnalysisKind::Transient, "seen").into_iter().map(|(t, r)| (t, Run::real(&r, "v"))).collect();
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
        vec![(0, Run::known_real(1000.0)), (first, Run::known_real(2000.0))],
        "the initial value at the operating point, then the commit at the first time point"
    );
}

#[test]
fn an_event_after_the_landing_reads_the_settle_commit() {
    // §15 S3: the operating point's settle phase committed 2 kOhm; the first time point (well before
    // the first clock edge at 0.37 us) lands it, so every edge reads the landed 2 kOhm. S4 itself (an
    // event time before the next time point reads the old value) cannot arise from source: the first
    // step is a tenth of the distance to the first pending edge, so no edge precedes the first time
    // point. `protocol_transient.rs` `an_event_time_before_the_next_time_point_reads_the_old_value`
    // is its proof
    let run = Scenario::of("commits.msl", "BetweenBench").transient(3e-6, 1e-6);
    let first = run.time_points()[0];
    let reads: Vec<(i64, f64)> = run.records(AnalysisKind::Transient, "read").into_iter().map(|(t, r)| (t, Run::real(&r, "r"))).collect();
    assert!(reads.len() >= 3, "edges at 0.37, 1.11 and 1.85 us at least: {reads:?}");
    for (t, r) in reads {
        assert!(t > first, "the edge at {t} follows the first time point {first}");
        assert_eq!(r, 2000.0, "the edge at {t} reads the landed settle commit");
    }
}

#[test]
fn a_delayed_settle_commit_is_ordinary() {
    // §15 S7: `r <- 2k after 250n` from the operating point's settle phase is an ordinary delayed
    // commit: the carrier changes at 250 ns, not at the next time point (1 us)
    let run = Scenario::of("commits.msl", "SlowBench").transient(3e-6, 1e-6);
    assert_eq!(run.changes("SlowBench.r"), vec![(0, Run::known_real(1000.0)), (250 * NS, Run::known_real(2000.0))]);
}

#[test]
fn a_settle_commit_at_the_stop_time_is_dropped_from_source() {
    // §15 S6: `Late` commits at the settle phase of the time points after 2.5 us. The one made at the
    // stop time (3 us) has no time point to land at and is dropped: the carrier never changes. Run
    // to 4 us the commit of 3 us lands at 4 us, which shows the commit is made at all
    let scenario = Scenario::of("commits.msl", "LateBench");
    let short = scenario.transient(3e-6, 1e-6);
    assert_eq!(short.instants(AnalysisKind::Transient, "committed"), vec![3 * US], "the reaction commits at the stop time");
    assert_eq!(short.changes("LateBench.r"), vec![(0, Run::known_real(1000.0))], "and the commit is dropped");
    let long = scenario.transient(4e-6, 1e-6);
    assert_eq!(long.changes("LateBench.r"), vec![(0, Run::known_real(1000.0)), (4 * US, Run::known_real(5000.0))], "run on, it lands at the next time point");
}

#[test]
fn an_event_read_as_a_bool_says_whether_it_fired() {
    // §13 rule 2 (UE-01 AC 1): the reaction runs at every rising edge; `a.rise()` and `b.rise()`
    // read inside it are true exactly for the edge that fired in that microstep: `a` alone at
    // 10 ns, `b` alone at 20 ns. The falls at 30 ns run nothing
    let run = Scenario::of("reads.msl", "DetectBench").transient(60e-9, 10e-9);
    let edges = run.flags("edges", "a", "b");
    assert_eq!(edges[..2], [(10 * NS, true, false), (20 * NS, false, true)], "{edges:?}");
    assert!(edges.iter().all(|(t, ..)| *t != 30 * NS), "no rising edge at 30 ns: {edges:?}");
}

#[test]
fn two_events_in_one_microstep_run_the_reaction_once_both_true() {
    // §12.1.15, §13 rule 18 (UE-01 AC 2): `a` and `b` rise in one microstep at 40 ns; the
    // reaction on `a.rise() || b.rise()` runs once there with both read as true
    let run = Scenario::of("reads.msl", "DetectBench").transient(60e-9, 10e-9);
    let at_forty: Vec<(i64, bool, bool)> = run.flags("edges", "a", "b").into_iter().filter(|(t, ..)| *t == 40 * NS).collect();
    assert_eq!(at_forty, [(40 * NS, true, true)], "one run, both edges true");
}

#[test]
fn an_event_not_in_the_head_reads_whether_it_fired() {
    // §13 rule 2 (edge case): the reaction is on `a.rise()` alone and reads `b.rise()`: false at
    // 10 ns where only `a` rises, true at 40 ns where `b` rises with it
    let run = Scenario::of("reads.msl", "ElsewhereBench").transient(60e-9, 10e-9);
    let seen: Vec<(i64, bool)> = run.records(AnalysisKind::Transient, "elsewhere").into_iter().map(|(t, r)| (t, Run::flag(&r, "b"))).collect();
    assert_eq!(seen, [(10 * NS, false), (40 * NS, true)], "{seen:?}");
}

#[test]
fn a_committed_event_read_commits_whether_it_fired() {
    // §13 rule 2, design D2 (UE-02 AC 5): `q <- a.rise()` on either edge commits whether `a` rose:
    // true at 10 ns, false at 20 ns (only `b` rose), true at 40 ns (both rose)
    let run = Scenario::of("reads.msl", "CommitBench").transient(60e-9, 10e-9);
    let changes: Vec<(i64, bool)> = run.changes("CommitBench.q").into_iter().filter(|(t, _)| *t > 0).map(|(t, bytes)| (t, bytes[0] == 1)).collect();
    assert_eq!(changes, [(10 * NS, true), (20 * NS, false), (40 * NS, true)], "{changes:?}");
}

#[test]
fn a_settle_reaction_reads_an_event_as_true_only_when_settle_fires_it() {
    // §15 Resolution, §13 rule 2 (UE-01 AC 4): in the settle phase only `settle` has fired, so a
    // reaction on `settle || clk.rise()` reads `settle` as true and the edge as false at every time
    // point, and the reverse at a rising edge (no edge here is a time point)
    let run = Scenario::of("reads.msl", "SettleBench").transient(3e-6, 1e-6);
    let steps = run.time_points();
    let reads = run.flags("either", "settle", "edge");
    for (t, settle, edge) in &reads {
        assert_eq!(*settle, steps.contains(t), "`settle` reads true at the time points only: {t}");
        assert_eq!(*edge, !*settle, "exactly one of the two fired at {t}");
    }
    assert!(reads.iter().any(|(_, settle, _)| *settle) && reads.iter().any(|(_, _, edge)| *edge), "both kinds of instant occur: {reads:?}");
}

#[test]
fn a_guard_reading_events_fires_only_when_d_changed_with_en_high_and_still() {
    // §13 rule 9 (UE-02 AC 1): `on d.changed() && en && !en.changed()` fires in a microstep where
    // `d` changed, `en` is true and `en` did not change: 20 ns (`en` high, still) and 60 ns only.
    // Not at 40 ns (`en` low), 50 ns (`en` rises with `d`) nor 70 ns (`en` falls with `d`)
    let run = Scenario::of("reads.msl", "HoldBench").transient(90e-9, 10e-9);
    assert_eq!(run.instants(AnalysisKind::Transient, "held"), [20 * NS, 60 * NS]);
}

#[test]
fn a_guard_reads_what_the_reaction_would_read() {
    // §13 rules 1 and 9 (UE-02 AC 2): the guard of `x.changed() && y.rise()` is read in the
    // microstep of the head. `x` and `y` rise in one microstep at 10 ns: it fires. At 20 ns and
    // 30 ns `y` follows `x` one microstep later: `y.rise()` reads false where `x` changed and
    // `x.changed()` is false where `y` rises, so it never fires
    let run = Scenario::of("reads.msl", "JointBench").transient(50e-9, 10e-9);
    assert_eq!(run.instants(AnalysisKind::Transient, "joint"), [10 * NS]);
}

impl Run {
    /// The levels `path` takes after the operating point: instant and `true` or `false`.
    fn levels(&self, path: &str) -> Vec<(i64, bool)> {
        self.changes(path).into_iter().filter(|(t, _)| *t > 0).map(|(t, bytes)| (t, bytes[0] == 1)).collect()
    }
}

#[test]
fn pfd3_rising_apart_steps_up_then_down() {
    // §13 rule 2 (UE-01 AC 3, Independent Test): `fref` alone at 10 ns steps the state up (`up` rises), `fvco` alone at
    // 20 ns steps it back to 0; `fvco` alone at 100 ns, the state being 0 after the pair at 80 ns, steps it to -1 (`dn`)
    let run = Scenario::of("pfd3.msl", "PfdBench").transient(120e-9, 10e-9);
    let up = run.levels("PfdBench.up");
    assert_eq!(up[..2], [(10 * NS, true), (20 * NS, false)], "{up:?}");
    let dn = run.levels("PfdBench.dn");
    assert_eq!(dn, [(100 * NS, true)], "{dn:?}");
    assert!(up.iter().filter(|(t, _)| *t < 60 * NS).all(|(t, _)| *t == 10 * NS || *t == 20 * NS), "nothing moves at 40 ns: {up:?}");
}

#[test]
fn pfd3_rising_together_leaves_state_unchanged() {
    // §13 rule 2 (UE-01 AC 3): both clocks rise in one microstep. From state 0 (40 ns) the reaction steps up first and
    // down second, so `up` and `dn` never change there; from state 1 (80 ns, after `fref` alone at 60 ns) it steps
    // `s < 1` false, then down: the state goes to 0, `up` falls and `dn` stays low
    let run = Scenario::of("pfd3.msl", "PfdBench").transient(120e-9, 10e-9);
    let up = run.levels("PfdBench.up");
    assert!(up.iter().all(|(t, _)| *t != 40 * NS), "up did not move at 40 ns: {up:?}");
    assert!(run.levels("PfdBench.dn").iter().all(|(t, _)| *t != 40 * NS), "dn did not move at 40 ns");
    assert_eq!(up, [(10 * NS, true), (20 * NS, false), (60 * NS, true), (80 * NS, false)], "{up:?}");
}

/// The instants, in ns, at which the stimulus of `handshake.msl` makes a handshake: `req` rises with `ack` high.
const HANDSHAKES: [i64; 4] = [40, 60, 110, 130];

#[test]
fn handshake_fires_when_req_rises_with_ack_high() {
    // §13 rule 7 (UE-04 AC 2): `fn handshake(req, ack) -> Event { req.rise() && ack }` heads a reaction that runs in the
    // microsteps where `req` rises with `ack` high: 40, 60 (both rise in one microstep), 110 and 130 ns, and not at
    // 10 ns (`ack` low), 20 ns (`ack` rises, `req` does not) nor 80 ns (`ack` low)
    let run = Scenario::of("handshake.msl", "HandshakeBench").transient(150e-9, 10e-9);
    let wanted: Vec<i64> = HANDSHAKES.iter().map(|ns| ns * NS).collect();
    assert_eq!(run.instants(AnalysisKind::Transient, "head"), wanted);
}

#[test]
fn an_event_function_in_every_event_position() {
    // §13 rule 7 (UE-04 AC 1): the call is the same event in a head, a module-body `let`, a guard after another event and
    // an operand of `||`: the guard holds at 110 ns only (`en` rises with a handshake; at 80 ns it rises with `ack` low),
    // `either` is the handshakes plus `ack` falling at 50 and 70 ns
    let run = Scenario::of("handshake.msl", "HandshakeBench").transient(150e-9, 10e-9);
    let wanted: Vec<i64> = HANDSHAKES.iter().map(|ns| ns * NS).collect();
    assert_eq!(run.instants(AnalysisKind::Transient, "named"), wanted, "a module-body let of the call");
    assert_eq!(run.instants(AnalysisKind::Transient, "modular"), wanted, "the result of a module");
    assert_eq!(run.instants(AnalysisKind::Transient, "guarded"), [110 * NS], "a guard after `en.rise()`");
    let mut either: Vec<i64> = wanted.iter().copied().chain([50 * NS, 70 * NS]).collect();
    either.sort_unstable();
    assert_eq!(run.instants(AnalysisKind::Transient, "either"), either, "an operand of `||`");
}

#[test]
fn an_event_function_read_as_a_bool_says_whether_it_fired() {
    // §13 rule 2 (UE-04 AC 3): read in a body run on either edge, the call is true exactly where it fired: at the edges of
    // 10, 20, 80 and 100 ns it did not, at 40, 60, 110 and 130 ns it did
    let run = Scenario::of("handshake.msl", "HandshakeBench").transient(150e-9, 10e-9);
    let read: Vec<(i64, bool)> = run.records(AnalysisKind::Transient, "read").into_iter().map(|(t, r)| (t, Run::flag(&r, "shaken"))).collect();
    let wanted: Vec<(i64, bool)> = [10, 20, 40, 60, 80, 100, 110, 130].iter().map(|ns| (ns * NS, HANDSHAKES.contains(ns))).collect();
    assert_eq!(read, wanted);
}

#[test]
fn an_event_function_calling_another_fires_under_the_outer_guard() {
    // §13 rule 7 (UE-04 AC 1): `gated` is `handshake(req, ack) && en`: of the handshakes only those with `en` high, 110 and
    // 130 ns, fire it
    let run = Scenario::of("handshake.msl", "HandshakeBench").transient(150e-9, 10e-9);
    assert_eq!(run.instants(AnalysisKind::Transient, "gated"), [110 * NS, 130 * NS]);
}

#[test]
fn an_event_function_binds_named_and_left_out_arguments() {
    // §3.2 with §13 rule 7 (UE-04 AC 1): `edge(clk, b: Some(true))` leaves `a` out (true), so it fires on both rises of
    // `clk`, 10 and 30 ns; `edge(clk, Some(false), ..)` binds `a` to false and never fires; `edge(clk, a: Some(true), b:
    // Some(true))` fires like the first; `edge(clk)` has `b` false and never fires
    let run = Scenario::of("arguments.msl", "ArgumentsBench").transient(50e-9, 10e-9);
    assert_eq!(run.instants(AnalysisKind::Transient, "named"), [10 * NS, 30 * NS], "`b` named, `a` left out");
    assert_eq!(run.instants(AnalysisKind::Transient, "both"), [10 * NS, 30 * NS], "both named");
    assert!(run.instants(AnalysisKind::Transient, "positional").is_empty(), "`a` positional false");
    assert!(run.instants(AnalysisKind::Transient, "neither").is_empty(), "`b` left out is false");
}

#[test]
fn an_event_function_chooses_between_events_by_a_value_read_at_the_instant() {
    // §13 rules 1 and 7 (UE-04): `pick(clk, s)` is `clk.rise()` while `s` holds and `clk.fall()` while it does not. `s` is
    // low until 25 ns, so the rise at 10 ns is not the event (the fall is) and the fall at 20 ns fires; the rise at 30 ns
    // fires, `s` being high. A `match` over the flag and an `if` over a counter the body builds with a `for` choose alike.
    let run = Scenario::of("choice.msl", "ChoiceBench").transient(50e-9, 10e-9);
    assert_eq!(run.instants(AnalysisKind::Transient, "pick"), [20 * NS, 30 * NS], "an `if` over a flag");
    assert_eq!(run.instants(AnalysisKind::Transient, "chosen"), [20 * NS, 30 * NS], "a `match` over a flag");
    assert_eq!(run.instants(AnalysisKind::Transient, "counted"), [20 * NS, 30 * NS], "an `if` over a `let` reassigned by a `for`");
}
