//! `emit!` in `analog` run from MSL source on the Rust backend through the host library
//! (analog-operators AO-09, Part VI §19 rules 3 and 7): each bench is written in
//! `tests/operators/traces.msl`, opened as a session that names the stream `newton`, and its
//! trace records are read as the host receives them.
#![cfg(unix)]

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::OnceLock;

use piperine_api::{Cell, Field, Overrides, Project, Record, RecordSink, Session, Table};
use piperine_transpile::Settings;

/// Keeps every record, in the order it is handed over.
#[derive(Default)]
struct Kept(Vec<Record>);

impl RecordSink for Kept {
    fn record(&mut self, record: &Record) {
        self.0.push(record.clone());
    }
}

/// The benches of `tests/operators/traces.msl` and their runs.
struct Traced;

impl Traced {
    /// The compile cache the runs share.
    fn settings() -> Settings {
        static CACHE: OnceLock<tempfile::TempDir> = OnceLock::new();
        let dir = CACHE.get_or_init(|| tempfile::tempdir().expect("a temp dir"));
        Settings { rustc: None, cache: dir.path().to_path_buf() }
    }

    /// The file of the benches.
    fn path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/operators/traces.msl")
    }

    /// One run of `top` with `overrides`: the table and the records it delivered.
    fn run(top: &str, overrides: Overrides) -> (Table, Vec<Record>) {
        let (table, records, _) = Self::run_counting_rejections(top, overrides);
        (table, records)
    }

    /// As [`run`](Self::run), with the number of step attempts the transient rejected (0 for another analysis).
    fn run_counting_rejections(top: &str, overrides: Overrides) -> (Table, Vec<Record>, usize) {
        let project = Project::file(&Self::path()).unwrap_or_else(|f| panic!("{f}"));
        let overrides = Overrides { top: Some(top.into()), ..overrides };
        let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
        let mut session = Session::open(&project, &spec, &Self::settings()).unwrap_or_else(|f| panic!("{f}"));
        let kept = Rc::new(RefCell::new(Kept::default()));
        let table = session.run(kept.clone()).unwrap_or_else(|f| panic!("{f}"));
        let records = kept.borrow().0.clone();
        let rejected = session.statistics().map_or(0, |statistics| statistics.analog().steps_rejected);
        (table, records, rejected)
    }

    /// The operating point of `a` that names the stream `newton`.
    fn op() -> Overrides {
        Overrides { analysis: Some("op".into()), print: vec!["a".into()], streams: vec!["newton".into()], ..Overrides::default() }
    }

    /// The real field `field` of `record`.
    fn real(record: &Record, field: &str) -> f64 {
        match record.fields.iter().find(|(name, _)| name == field) {
            Some((_, Field::Real(v))) => *v,
            other => panic!("no real `{field}` in {record:?}: {other:?}"),
        }
    }

    /// The value of the row `signal` of an operating point's table.
    fn value(table: &Table, signal: &str) -> f64 {
        let row = table.rows.iter().find(|r| r[0] == Cell::Text(signal.into())).unwrap_or_else(|| panic!("no row `{signal}` in {table:?}"));
        match &row[1] {
            Cell::Real(v) => *v,
            other => panic!("a real, found {other:?}"),
        }
    }
}

#[test]
fn a_trace_writes_one_record_per_evaluation_with_path_analysis_instant_and_iteration() {
    // AO-09 AC 1: a diode behind 1 kohm from 5 V takes several Newton iterations from 0 V; each evaluation
    // writes one record with the instance path, the analysis, the instant (0 in an operating point) and the
    // iteration, counted from 0 without a gap; `v` climbs from the first iterate to the operating point.
    let (table, records) = Traced::run("DiodeBench", Traced::op());
    assert!(records.len() >= 3, "several evaluations: {records:?}");
    assert!(records.iter().all(|r| r.instance == "DiodeBench.d" && r.stream == "newton"), "{records:?}");
    assert!(records.iter().all(|r| r.analysis == Some(piperine_solver::protocol::AnalysisKind::OperatingPoint) && r.time == 0.0), "{records:?}");
    let iterations: Vec<u32> = records.iter().filter_map(|r| r.iteration).collect();
    assert_eq!(iterations, (0..records.len() as u32).collect::<Vec<_>>(), "one record per evaluation, numbered from 0");
    let last = Traced::real(records.last().expect("a record"), "v");
    let solved = Traced::value(&table, "DiodeBench.a");
    assert!((last - solved).abs() <= 1e-6, "the last evaluation is at the operating point: {last} vs {solved}");
    assert!(Traced::real(&records[0], "v") < last, "Newton starts below the operating point");
}

#[test]
fn a_trace_written_through_a_fn_is_the_trace_of_its_call() {
    // AO-09 AC 1 (§19 rule 10, GAPS L72): `note(v(a))` with `fn note(level) { emit!("newton", v = level); }` writes the
    // records `emit!("newton", v = v(a))` writes: the same instants and values as the diode bench's.
    let (_, direct) = Traced::run("DiodeBench", Traced::op());
    let (_, through) = Traced::run("FnTracedBench", Traced::op());
    let values = |records: &[Record]| records.iter().map(|r| (r.iteration, Traced::real(r, "v"))).collect::<Vec<_>>();
    assert!(!through.is_empty());
    assert_eq!(values(&through), values(&direct));
    assert!(through.iter().all(|r| r.instance == "FnTracedBench.d" && r.stream == "newton"), "{through:?}");
}

#[test]
fn traces_are_recorded_only_for_a_named_stream() {
    // AO-09 AC 6 (Q5): with no stream named the host receives no trace; naming another stream receives none either.
    let (_, none) = Traced::run("DiodeBench", Overrides { streams: vec![], ..Traced::op() });
    assert!(none.is_empty(), "{none:?}");
}

#[test]
fn a_trace_changes_no_solution() {
    // AO-09 AC 7: the same circuit with the trace and without gives the same operating point.
    let (traced, _) = Traced::run("DiodeBench", Traced::op());
    let (quiet, _) = Traced::run("QuietBench", Overrides { streams: vec![], ..Traced::op() });
    assert_eq!(Traced::value(&traced, "DiodeBench.a"), Traced::value(&quiet, "QuietBench.a"));
}

#[test]
fn a_trace_under_an_if_writes_only_when_executed() {
    // AO-09 AC 4: the guarded trace writes only the evaluations at which `v(a)` is above 0.5 V: every
    // record's `v` is above it, and fewer records exist than the unguarded bench's.
    let (_, guarded) = Traced::run("GuardedBench", Traced::op());
    let (_, all) = Traced::run("DiodeBench", Traced::op());
    assert!(!guarded.is_empty() && guarded.len() < all.len(), "{} of {}", guarded.len(), all.len());
    assert!(guarded.iter().all(|r| Traced::real(r, "v") > 0.5), "{guarded:?}");
}

#[test]
fn an_ac_or_noise_sweep_writes_no_trace_beyond_its_operating_point() {
    // AO-09 AC 3 and AC 1: the operating point an AC or noise run linearizes about is an equilibrium solve
    // and writes the records of the operating-point run, one per evaluation; the sweep adds none, however
    // many frequencies it takes (3 here).
    let (_, op) = Traced::run("DiodeBench", Traced::op());
    assert!(!op.is_empty());
    let ac = Overrides { analysis: Some("ac".into()), from: Some(1e3), to: Some(1e5), per_decade: Some(1), print: vec!["a".into()], streams: vec!["newton".into()], ..Overrides::default() };
    let (swept, ac_records) = Traced::run("DiodeBench", ac);
    assert!(swept.rows.len() >= 3, "{} frequencies", swept.rows.len());
    assert_eq!(ac_records, op, "the AC run delivers its operating point's records and nothing else");
    let noise = Overrides { analysis: Some("noise".into()), output: Some("a".into()), from: Some(1e3), to: Some(1e5), per_decade: Some(1), streams: vec!["newton".into()], ..Overrides::default() };
    let (_, noise_records) = Traced::run("DiodeBench", noise);
    assert_eq!(noise_records, op, "the noise run delivers its operating point's records and nothing else");
}

#[test]
fn traces_of_a_rejected_step_stay() {
    // AO-09 AC 2 (§19 rule 7): the transient across a steep step rejects steps, and the records the rejected
    // attempts wrote stay: exactly as many instants hold records that no accepted row has as the solver
    // counts rejected attempts.
    let overrides = Overrides { analysis: Some("transient".into()), stop: Some(3e-6), max_step: Some(1e-6), print: vec!["o".into()], streams: vec!["newton".into()], ..Overrides::default() };
    let (table, records, rejected) = Traced::run_counting_rejections("StepBench", overrides);
    assert!(rejected > 0, "the bench must reject steps: {rejected}");
    let accepted: std::collections::BTreeSet<u64> = table.rows.iter().filter_map(|row| match row[0] {
        Cell::Real(t) => Some(t.to_bits()),
        _ => None,
    }).collect();
    let instants: std::collections::BTreeSet<u64> = records.iter().map(|r| r.time.to_bits()).collect();
    let rejected_instants = instants.difference(&accepted).count();
    assert_eq!(rejected_instants, rejected, "each rejected attempt delivered its records at an instant no accepted row has");
    assert!(records.iter().all(|r| r.iteration.is_some() && r.analysis.is_some()));
}
