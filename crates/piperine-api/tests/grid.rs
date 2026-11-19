//! The rows of a transient, made while the run goes (feature `long-run`, LR-03, LR-05 to LR-07):
//! a session's transient keeps no step and records the signals it prints, with the print grid
//! applied as the steps arrive. Each table is compared with what the run it replaces gave: the
//! table of a `Keep::Every` run (`Table::transient`), and that table on the grid
//! (`Table::on_grid`), by bits. The RC is the twin `tests/e2e/twins/tran_rc.msl`; the counter is
//! written by the test. Rust-backend sessions build into a cache directory of this target.
#![cfg(unix)]

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::OnceLock;

use piperine_api::{Cell, Failure, Overrides, Project, Record, RecordSink, Session, Table};
use piperine_solver::prelude::{Context, TransientAnalysisOptions};
use piperine_transpile::Settings;

/// A sink that keeps nothing: these tests read tables.
struct Ignore;

impl RecordSink for Ignore {
    fn record(&mut self, _: &Record) {}
}

fn twin(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/e2e/twins").join(name)
}

fn settings() -> Settings {
    static CACHE: OnceLock<tempfile::TempDir> = OnceLock::new();
    let dir = CACHE.get_or_init(|| tempfile::tempdir().expect("tmp"));
    Settings { rustc: None, cache: dir.path().to_path_buf() }
}

fn tran(stop: f64, print: &[&str]) -> Overrides {
    Overrides { analysis: Some("transient".into()), stop: Some(stop), print: print.iter().map(|s| s.to_string()).collect(), ..Overrides::default() }
}

fn session(path: &Path, backend: &str, overrides: Overrides) -> Session {
    let project = Project::file(path).unwrap_or_else(|f| panic!("{f}"));
    let spec = project.request(None, &Overrides { backend: Some(backend.into()), ..overrides }).unwrap_or_else(|f| panic!("{f}"));
    Session::open(&project, &spec, &settings()).unwrap_or_else(|f| panic!("{f}"))
}

/// The rows of the run of `overrides` on `backend`.
fn run(path: &Path, backend: &str, overrides: Overrides) -> Table {
    session(path, backend, overrides).run(Rc::new(RefCell::new(Ignore))).unwrap_or_else(|f| panic!("{backend}: {f}"))
}

/// The table of the same run that keeps every accepted step: the solver's options as the session
/// sets them, with `Keep::Every` (the default) and the window `from` if any.
fn kept_every(path: &Path, backend: &str, overrides: Overrides, stop: f64, from: Option<f64>) -> Table {
    let mut session = session(path, backend, overrides);
    let probes = session.probes().to_vec();
    let mut options = TransientAnalysisOptions::new(stop, stop / 1000.0);
    if let Some(from) = from {
        options = options.with_record_from(from);
    }
    let circuit = session.circuit().expect("a circuit before any run");
    let result = circuit.transient(options, Context::default()).and_then(|mut t| t.solve()).unwrap_or_else(|e| panic!("{backend}: {e}"));
    Table::transient(&probes, &result).unwrap_or_else(|f| panic!("{f}"))
}

/// Two tables equal cell for cell, a real by its bits.
fn assert_bit_equal(got: &Table, want: &Table, what: &str) {
    assert_eq!(got.columns, want.columns, "{what}");
    assert_eq!(got.rows.len(), want.rows.len(), "{what}: row count");
    for (n, (row, want)) in got.rows.iter().zip(&want.rows).enumerate() {
        assert_eq!(row.len(), want.len(), "{what}: row {n}");
        for (cell, want) in row.iter().zip(want) {
            match (cell, want) {
                (Cell::Real(a), Cell::Real(b)) => assert_eq!(a.to_bits(), b.to_bits(), "{what}: row {n}: {a} against {b}"),
                (a, b) => assert_eq!(a, b, "{what}: row {n}"),
            }
        }
    }
}

const BACKENDS: [&str; 2] = ["interp", "rust"];

#[test]
fn a_gridless_transient_equals_the_table_of_a_run_that_kept_every_step() {
    // LR-03 AC 1, 2 and LR-07 AC 1: no print grid, so one row per accepted step (and per step
    // from `from`), whose cells are the ones a run that kept every step reads
    for backend in BACKENDS {
        let plain = tran(10e-6, &["vin", "out", "i(v1)"]);
        let want = kept_every(&twin("tran_rc.msl"), backend, plain.clone(), 10e-6, None);
        assert!(want.rows.len() > 10, "{backend}: the run took {} steps", want.rows.len());
        assert_bit_equal(&run(&twin("tran_rc.msl"), backend, plain), &want, &format!("{backend}, whole run"));
        let from = Overrides { from: Some(3e-6), ..tran(10e-6, &["vin", "out", "i(v1)"]) };
        let want = kept_every(&twin("tran_rc.msl"), backend, from.clone(), 10e-6, Some(3e-6));
        assert!(want.rows.len() > 5, "{backend}: {} rows from 3 us", want.rows.len());
        assert_bit_equal(&run(&twin("tran_rc.msl"), backend, from), &want, &format!("{backend}, from 3 us"));
    }
}

#[test]
fn the_gridded_tran_rc_is_bit_equal_to_on_grid_on_both_backends() {
    // LR-06 AC 1: the rows of a run with `print_step` are the rows `Table::on_grid` makes of the
    // table of every step, by bits, for a grid that hits the steps and one that does not
    for backend in BACKENDS {
        let all = kept_every(&twin("tran_rc.msl"), backend, tran(10e-6, &["vin", "out", "i(v1)"]), 10e-6, None);
        for step in [1e-6, 0.37e-6] {
            let want = all.on_grid(step, 0.0, 10e-6).unwrap_or_else(|f| panic!("{f}"));
            let overrides = Overrides { print_step: Some(step), ..tran(10e-6, &["vin", "out", "i(v1)"]) };
            assert_bit_equal(&run(&twin("tran_rc.msl"), backend, overrides), &want, &format!("{backend}, print_step {step:e}"));
        }
    }
}

#[test]
fn a_gridded_run_with_from_starts_at_the_first_multiple_not_before_it() {
    // LR-05 AC 1: with `from` 2.5 us on a 1 us grid the first row is 3 us, interpolated from the
    // steps around it, and the rows are those `on_grid` makes of the whole run from `from`
    for backend in BACKENDS {
        let all = kept_every(&twin("tran_rc.msl"), backend, tran(10e-6, &["out"]), 10e-6, None);
        let want = all.on_grid(1e-6, 2.5e-6, 10e-6).unwrap_or_else(|f| panic!("{f}"));
        let overrides = Overrides { print_step: Some(1e-6), from: Some(2.5e-6), ..tran(10e-6, &["out"]) };
        let got = run(&twin("tran_rc.msl"), backend, overrides);
        assert_eq!(got.rows[0][0], Cell::Real(3.0 * 1e-6), "{backend}: the first row");
        assert_eq!(got.rows.len(), 8, "{backend}: 3 to 10 us");
        assert_bit_equal(&got, &want, backend);
    }
}

/// An RC under a source and a 1 MHz clock that a counter counts: its rising edges are at
/// 0.5, 1.5, 2.5 us (the oscillator toggles every half period).
const COUNTER: &str = "pub module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\npub module Vsource(p: Electrical, n: Electrical) { dc: f64 = 1.0; }\nanalog for Vsource { v(p, n) +> dc; }\n\npub module Osc(out clk: Bit = false) { f: f64 = 1M where f > 0.0; }\ndigital for Osc { on clk.changed() { clk <- !clk after 0.5 / f; } }\npub module Counter(in clk: Bit, out q: Bit<u8> = 0) {}\ndigital for Counter { on clk.rise() { q <- q + 1; } }\n\nmodule Top() {\n    signal a: Electrical;\n    signal clk: Bit;\n    signal q: Bit<u8>;\n    let v1 = Vsource(a, gnd) { dc: 1.0 };\n    let r1 = Resistor(a, gnd) { r: 1k };\n    let osc = Osc { f: 1M }(clk);\n    let cnt = Counter(clk, q);\n}\n";

#[test]
fn a_counter_that_changes_between_analog_steps_is_exact_on_the_grid() {
    // LR-05 AC 4, LR-06 AC 3: the counter counts the rising edges at (n + 0.5) periods; a row at
    // `t` shows the edges at or before it, however far apart the analog steps are
    let dir = tempfile::tempdir().expect("tmp");
    let file = dir.path().join("counter.msl");
    std::fs::write(&file, COUNTER).expect("write");
    let period = 1e-6;
    let edges = |t: f64| (0..).map(|n| (n as f64 + 0.5) * period).take_while(|edge| *edge <= t).count() as i128;
    // a 0.505 us grid never lands on an edge before 3 us; its second row (0.505 us) lies just
    // after the first edge, between two analog steps (the solver takes steps of about 0.03 us
    // there and none at the edge), where a column sampled at the steps would still be 0
    let overrides = Overrides { print_step: Some(0.505e-6), max_step: Some(1e-6), top: Some("Top".into()), ..tran(3e-6, &["clk", "q"]) };
    let table = run(&file, "rust", overrides);
    assert_eq!(table.rows.len(), 6, "the multiples of 0.505 us up to 3 us");
    for row in &table.rows {
        let Cell::Real(t) = row[0] else { panic!("a time") };
        assert_eq!(row[2], Cell::Int(edges(t)), "q at {t:e} s");
    }
    assert_eq!(table.rows[1][2], Cell::Int(1), "the edge at 0.5 us is before the row at 0.505 us");
    assert_eq!(table.rows[5][2], Cell::Int(3), "three edges by 2.525 us");
}

#[test]
fn a_run_table_and_flags_give_equal_tables() {
    // LR-07 AC 2: `examples/rc`'s `step` (a `[run]` table with `print_step`) and the same run
    // given as flags on its bench are one table
    let rc = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/rc");
    let project = Project::at(&rc).unwrap_or_else(|f| panic!("{f}"));
    let spec = project.request(Some("step"), &Overrides::default()).unwrap_or_else(|f| panic!("{f}"));
    let from_table = Session::open(&project, &spec, &settings()).unwrap_or_else(|f| panic!("{f}")).run(Rc::new(RefCell::new(Ignore))).unwrap_or_else(|f| panic!("{f}"));
    let bench = rc.join("benches/step.msl");
    let flags = Overrides { analysis: Some("transient".into()), stop: Some(5e-6), print_step: Some(1e-6), ..Overrides::default() };
    let from_flags = run(&bench, "rust", flags);
    assert_eq!(from_table.rows.len(), 6, "0 to 5 us on a 1 us grid");
    assert_bit_equal(&from_flags, &from_table, "flags against the table");
}

#[test]
fn a_printed_signal_with_no_value_fails_before_the_first_step() {
    // LR-03 AC 3: a column is resolved against the bound circuit before the solver is built, so
    // a signal the circuit has no value for is a failure naming it with no step taken. `print`
    // only resolves names the design has, and every such name has a value, so the case is made
    // with a probe of a node the circuit lacks, resolved the way the run resolves each of its
    // own (`Probe::column` on the session's circuit); the run's probes all resolve.
    let mut session = session(&twin("tran_rc.msl"), "interp", tran(10e-6, &["vin", "out", "i(v1)"]));
    let probes = session.probes().to_vec();
    let circuit = session.circuit().expect("a circuit before any run");
    for probe in &probes {
        assert!(probe.column(circuit).is_ok(), "{}", probe.label());
    }
    let ghost = piperine_api::Probe::Node { path: "TranRc.ghost".into(), id: piperine_solver::prelude::NodeIdentifier::Anonymous(9999) };
    match ghost.column(circuit) {
        Err(Failure::Backend(message)) => assert!(message.contains("TranRc.ghost"), "{message}"),
        other => panic!("a failure naming `TranRc.ghost`, found {other:?}"),
    }
}
