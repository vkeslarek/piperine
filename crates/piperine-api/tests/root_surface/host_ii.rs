//! Scope: the host II surface reached through the root package's `piperine::` re-export (HII-24):
//! a session opened with a transfer-function run, a top input driven at a time, and a param set at
//! a time. The api crate's own tests hold each rule; this target proves the root hands the same
//! types (`Session`, `Overrides`, `ParamValue`, `Table`, `Cell`) to an external Rust host.
#![cfg(unix)]

use std::cell::RefCell;
use std::rc::Rc;

use piperine::{Cell, Overrides, ParamValue, Project, Record, RecordSink, Session, Table};
use piperine_transpile::Settings;

/// Keeps every record, in the order it is handed over.
#[derive(Default)]
struct Kept(Vec<Record>);

impl RecordSink for Kept {
    fn record(&mut self, record: &Record) {
        self.0.push(record.clone());
    }
}

/// A 1 V source across a 1 kOhm resistor, and a counter of the rising edges of the top's input `clk`.
const DESIGN: &str = "pub module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\npub module Vsource(p: Electrical, n: Electrical) { dc: f64 = 1.0; }\nanalog for Vsource { v(p, n) +> dc; }\nmodule Counter(in clk: Bit, in level: Bit<u8>, out count: Bit<u8> = 0) {}\ndigital for Counter {\n    on clk.rise() {\n        count <- count + 1;\n        emit!(\"edge\", t = now(), count = count, level = level);\n        emit!(\"tick\", n = count);\n    }\n}\nmodule Top(in clk: Bit, in level: Bit<u8>, out count: Bit<u8>) {\n    signal a: Electrical;\n    signal armed: Bit = false;\n    let v1 = Vsource(a, gnd) { dc: 1.0 };\n    let r1 = Resistor(a, gnd) { r: 1k };\n    let ctr = Counter(clk, level, count);\n}\n";

/// Opens `analysis` over [`DESIGN`] with the given overrides, on the compiled backend.
fn session(analysis: &str, overrides: Overrides) -> Session {
    let dir = tempfile::tempdir().expect("tmp");
    let file = dir.path().join("bench.msl");
    std::fs::write(&file, DESIGN).expect("write");
    let project = Project::file(&file).unwrap_or_else(|failure| panic!("{failure}"));
    let overrides = Overrides { backend: Some("rust".into()), top: Some("Top".into()), analysis: Some(analysis.into()), ..overrides };
    let spec = project.request(None, &overrides).unwrap_or_else(|failure| panic!("{failure}"));
    let cache = dir.path().join("cache");
    Session::open(&project, &spec, &Settings { rustc: None, cache }).unwrap_or_else(|failure| panic!("{failure}"))
}

/// Runs `session` and returns its table and the records it wrote.
fn run(session: &mut Session) -> (Table, Vec<Record>) {
    let kept = Rc::new(RefCell::new(Kept::default()));
    let table = session.run(kept.clone()).unwrap_or_else(|failure| panic!("{failure}"));
    let records = kept.borrow().0.clone();
    (table, records)
}

#[test]
fn the_root_runs_tf_a_drive_and_a_timed_set() {
    // HII-24 AC 1. `tf` from `v1` to node `a`: the output is the source's own node, so the gain
    // is 1 (a 1 V step in is a 1 V step out)
    let mut tf = session("tf", Overrides { input: Some("v1".into()), output: Some("a".into()), ..Overrides::default() });
    let (table, _) = run(&mut tf);
    let row = table.rows.iter().find(|row| row[0] == Cell::Text("gain".into())).unwrap_or_else(|| panic!("a gain row in {table:?}"));
    match &row[1] {
        Cell::Real(gain) => assert!((gain - 1.0).abs() < 1e-9, "gain {gain}"),
        other => panic!("a real gain, found {other:?}"),
    }

    // a drive: `clk` low at 0 and high at 1 us gives one rising edge, stamped 1 us, which a
    // timed `set` of `v1.dc` to 2 V at 0.5 us does not disturb; `a` reads 2 V at the stop
    let mut transient = session("transient", Overrides { stop: Some(2e-6), max_step: Some(1e-7), print_step: Some(1e-6), print: vec!["a".into()], ..Overrides::default() });
    transient.drive("clk", ParamValue::Bool(false), 0.0).unwrap_or_else(|failure| panic!("{failure}"));
    transient.drive("clk", ParamValue::Bool(true), 1e-6).unwrap_or_else(|failure| panic!("{failure}"));
    transient.set_at("v1.dc", 2.0, 5e-7).unwrap_or_else(|failure| panic!("{failure}"));
    let (table, records) = run(&mut transient);
    let edges: Vec<&Record> = records.iter().filter(|record| record.stream == "edge").collect();
    assert_eq!(edges.len(), 1, "{records:?}");
    assert!((edges[0].time - 1e-6).abs() < 1e-12, "{:?}", edges[0]);
    match table.rows.last().expect("rows")[1] {
        Cell::Real(a) => assert!((a - 2.0).abs() < 1e-6, "a after the set: {a}"),
        ref other => panic!("a real, found {other:?}"),
    }
}
