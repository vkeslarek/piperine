//! Scope: a transient of a design with no analog unknown runs to its stop (HII-17, L56). The
//! design is the Book's `Blinky`: an oscillator at 1 MHz (a half period of 0.5 us) and a counter
//! of its rising edges. A transient that ends at a `finish` is a run ended ok (X4: HII-17 AC 3 restated). A run
//! that genuinely ends short has no source: since `msl-fixes` the only thing that ends a transient
//! early is a `finish` or a `fatal`, both of which set the run's end; the short-end rule itself
//! (CLI-05 AC 11) is held by `Recorder::reached` and its unit test.
#![cfg(unix)]

use std::cell::RefCell;
use std::rc::Rc;

use piperine_api::{Cell, Failure, Overrides, Project, Record, Session, Table};
use piperine_parity::compile_cache::CompileCache;
use piperine_parity::sinks::Kept;

/// The `Blinky` design run through a session that keeps its records.
struct StreamRun;

impl StreamRun {
    /// The run of `source` that keeps only the record streams `streams` name (all of them when empty).
    fn run_streams(source: &str, stop: f64, print_step: Option<f64>, streams: Vec<String>) -> Result<(Table, Vec<Record>), Failure> {
        let dir = tempfile::tempdir().expect("tmp");
        let file = dir.path().join("blinky.msl");
        std::fs::write(&file, source).expect("write");
        let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
        let overrides = Overrides { backend: Some("rust".into()), analysis: Some("transient".into()), top: Some("Blinky".into()), stop: Some(stop), print_step, print: vec!["clk".into(), "count".into()], streams, ..Overrides::default() };
        let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
        let mut session = Session::open(&project, &spec, &CompileCache::shared().settings()).unwrap_or_else(|f| panic!("{f}"));
        let kept = Rc::new(RefCell::new(Kept(Vec::new())));
        let table = session.run(kept.clone())?;
        let records = kept.borrow().0.clone();
        Ok((table, records))
    }
}

const BLINKY: &str = "module Osc(out clk: Bit = false) {\n    f: f64 = 1M where f > 0.0;\n}\ndigital for Osc {\n    on clk.changed() { clk <- !clk after 0.5 / f; }\n}\n\nmodule Counter(in clk: Bit, out count: Bit<u8> = 0) {}\ndigital for Counter {\n    on clk.rise() {\n        count <- count + 1;\n        emit!(\"edge\", t = now(), count = count);\n    }\n}\n\nmodule Blinky() {\n    signal clk: Bit;\n    pub signal count: Bit<u8>;\n    let osc = Osc(clk) { f: 1M };\n    let ctr = Counter(clk, count);\n}\n";

/// `BLINKY` and a module that calls `finish` at 2 us, a run the model ends itself.
const FINISHING: &str = "module Stopper() {}\ndigital for Stopper { on at(2u) { finish(); } }\n";

/// Associated test helpers of `digital_only`, grouped on one fixture type.
struct DigitalOnlyFixture;

impl DigitalOnlyFixture {
    fn run(stop: f64, print_step: Option<f64>) -> Result<(Table, Vec<Record>), Failure> {
        DigitalOnlyFixture::run_source(BLINKY, stop, print_step)
    }

    fn run_source(source: &str, stop: f64, print_step: Option<f64>) -> Result<(Table, Vec<Record>), Failure> {
        StreamRun::run_streams(source, stop, print_step, Vec::new())
    }
}

#[test]
fn a_digital_only_transient_reaches_its_stop() {
    // HII-17 AC 1 and 4: three rising edges by 3 us (at 0.5, 1.5 and 2.5 us, the Book's listing),
    // a table whose last row is the stop
    let (table, records) = DigitalOnlyFixture::run(3e-6, None).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(records.iter().filter(|r| r.stream == "edge").count(), 3, "{records:?}");
    let last = table.rows.last().expect("a table with rows");
    assert_eq!(last[0], Cell::Real(3e-6));
    assert_eq!(last[2], Cell::Int(3), "the counter after three edges: {last:?}");
}

#[test]
fn a_digital_only_grid_holds_net_values() {
    // HII-17 AC 2 (LR-05 AC 4): each row holds the value after the last instant at or before it.
    // The clock rises at 0.5, 1.5, 2.5 us and falls at 1, 2, 3 us, so on the 1 us grid it is low;
    // the counter reads 0, 1, 2, 3
    let (table, _) = DigitalOnlyFixture::run(3e-6, Some(1e-6)).unwrap_or_else(|f| panic!("{f}"));
    let times: Vec<Cell> = table.rows.iter().map(|r| r[0].clone()).collect();
    assert_eq!(times, [0.0, 1e-6, 2e-6, 3e-6].map(Cell::Real));
    let counts: Vec<Cell> = table.rows.iter().map(|r| r[2].clone()).collect();
    assert_eq!(counts, [0, 1, 2, 3].map(Cell::Int));
    assert!(table.rows.iter().all(|r| r[1] == Cell::Bool(false)), "{:?}", table.rows);
}

#[test]
fn a_digital_only_run_ended_by_finish_is_ok() {
    // HII-17 AC 3 restated with X4 (`finish` ends the run, ok): Blinky plus a `Stopper` that calls
    // `finish` at 2 us, in a run to 5 us, is not "ended before its stop": it gives its table up to
    // 2 us (the counter has seen the edge at 0.5 us and 1.5 us) and no failure
    let source = BLINKY.replace("    let ctr = Counter(clk, count);\n", "    let ctr = Counter(clk, count);\n    let stop = Stopper();\n") + FINISHING;
    let (table, records) = DigitalOnlyFixture::run_source(&source, 5e-6, None).unwrap_or_else(|f| panic!("{f}"));
    let last = table.rows.last().expect("a table with rows");
    let Cell::Real(ended) = last[0] else { panic!("a time: {last:?}") };
    assert!((ended - 2e-6).abs() <= 1e-12, "the table ends at the finish instant: {last:?}");
    assert_eq!(last[2], Cell::Int(2), "the counter after the edges at 0.5 and 1.5 us: {last:?}");
    assert_eq!(records.iter().filter(|r| r.stream == "edge").count(), 2, "{records:?}");
}

#[test]
fn a_finish_ends_the_run_ok_when_the_sim_stream_is_not_kept() {
    // the Book's `finish()` ends the run whatever streams the host asked for: with only the `edge`
    // stream kept, the `sim` record that carries the finish is not delivered, and the run still
    // ends at 2 us ok and not "ended before its stop"
    let source = BLINKY.replace("    let ctr = Counter(clk, count);\n", "    let ctr = Counter(clk, count);\n    let stop = Stopper();\n") + FINISHING;
    let (table, records) = StreamRun::run_streams(&source, 5e-6, None, vec!["edge".into()]).unwrap_or_else(|f| panic!("{f}"));
    let last = table.rows.last().expect("a table with rows");
    let Cell::Real(ended) = last[0] else { panic!("a time: {last:?}") };
    assert!((ended - 2e-6).abs() <= 1e-12, "the table ends at the finish instant: {last:?}");
    assert!(records.iter().all(|r| r.stream == "edge"), "only the kept stream is delivered: {records:?}");
}
