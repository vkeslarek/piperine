//! Scope: what a Rust host does to a session from outside (HII-12 to HII-14, X8, Q2 (a)): drive
//! a top's discrete `in` ports, set a param at a time, and narrow the record streams before the
//! run. The designs are MSL written by the test; an expected value is read from the source and
//! from `docs/spec/part_viii_host.md` §23.1, cited at each assertion.
#![cfg(unix)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;

use piperine_api::{Cell, Failure, Field, Overrides, ParamPath, ParamValue, Project, Record, RecordSink, Session, Table};
use piperine_transpile::Settings;

/// Keeps every record, in the order it is handed over.
#[derive(Default)]
struct Kept(Vec<Record>);

impl RecordSink for Kept {
    fn record(&mut self, record: &Record) {
        self.0.push(record.clone());
    }
}

fn settings() -> Settings {
    static CACHE: OnceLock<tempfile::TempDir> = OnceLock::new();
    let dir = CACHE.get_or_init(|| tempfile::tempdir().expect("tmp"));
    Settings { rustc: None, cache: dir.path().to_path_buf() }
}

/// A source, a resistor, a module whose structure a param selects, and a counter that reads two
/// of the top's inputs; `Top` holds the analog part and the counter.
const DESIGN: &str = "pub module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\npub module Vsource(p: Electrical, n: Electrical) { dc: f64 = 1.0; }\nanalog for Vsource { v(p, n) +> dc; }\nmodule Counter(in clk: Bit, in level: Bit<u8>, out count: Bit<u8> = 0) {}\ndigital for Counter {\n    on clk.rise() {\n        count <- count + 1;\n        emit!(\"edge\", t = now(), count = count, level = level);\n    }\n}\nmodule Top(in clk: Bit, in level: Bit<u8>, out count: Bit<u8>) {\n    signal a: Electrical;\n    signal armed: Bit = false;\n    let v1 = Vsource(a, gnd) { dc: 1.0 };\n    let r1 = Resistor(a, gnd) { r: 1k };\n    let ctr = Counter(clk, level, count);\n}\n";

fn session(analysis: &str, extra: Overrides) -> Session {
    let dir = tempfile::tempdir().expect("tmp");
    let file = dir.path().join("bench.msl");
    std::fs::write(&file, DESIGN).expect("write");
    let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
    let overrides = Overrides { backend: Some("rust".into()), top: Some("Top".into()), analysis: Some(analysis.into()), ..extra };
    let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
    Session::open(&project, &spec, &settings()).unwrap_or_else(|f| panic!("{f}"))
}

fn transient(stop: f64, print: &[&str]) -> Session {
    session("transient", Overrides { stop: Some(stop), max_step: Some(stop / 20.0), print: print.iter().map(|s| s.to_string()).collect(), ..Overrides::default() })
}

fn run(session: &mut Session) -> Result<(Table, Vec<Record>), Failure> {
    let kept = Rc::new(RefCell::new(Kept::default()));
    let table = session.run(kept.clone())?;
    let records = kept.borrow().0.clone();
    Ok((table, records))
}

fn code(failure: Failure) -> &'static str {
    match failure {
        Failure::Host { code, .. } => code,
        other => panic!("a host failure, found {other}"),
    }
}

fn edges(records: &[Record]) -> Vec<&Record> {
    records.iter().filter(|r| r.stream == "edge").collect()
}

fn field<'a>(record: &'a Record, name: &str) -> &'a Field {
    &record.fields.iter().find(|(n, _)| n == name).unwrap_or_else(|| panic!("no field {name} in {record:?}")).1
}

#[test]
fn a_drive_at_zero_is_the_initial_value() {
    // HII-12 AC 1 (§23.1 rule 4): the value driven before the run is the port's initial value, so
    // an operating point reads it; HII-12 AC 5 (rule 9): a port never driven stays unknown
    let mut driven = session("op", Overrides { print: vec!["clk".into(), "level".into()], ..Overrides::default() });
    driven.drive("clk", ParamValue::Bool(true), 0.0).expect("drive clk");
    driven.drive("Top.level", ParamValue::Int(5), 0.0).expect("drive level, written in full");
    let (table, _) = run(&mut driven).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(table.rows[0][1], Cell::Bool(true));
    assert_eq!(table.rows[1][1], Cell::Int(5));
    let mut undriven = session("op", Overrides { print: vec!["clk".into(), "level".into()], ..Overrides::default() });
    let (table, _) = run(&mut undriven).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!((&table.rows[0][1], &table.rows[1][1]), (&Cell::Unknown, &Cell::Unknown));
}

#[test]
fn a_drive_later_fires_a_reaction_at_its_time() {
    // HII-12 AC 2: the clock is low from the start and driven high at 1 us; the counter's rising
    // reaction fires at that instant, so its `edge` record is stamped 1 us and carries the level
    // driven at 0
    let mut s = transient(2e-6, &[]);
    s.drive("clk", ParamValue::Bool(false), 0.0).expect("clk low");
    s.drive("level", ParamValue::Int(7), 0.0).expect("level");
    s.drive("clk", ParamValue::Bool(true), 1e-6).expect("clk high at 1 us");
    let (_, records) = run(&mut s).unwrap_or_else(|f| panic!("{f}"));
    let edges = edges(&records);
    assert_eq!(edges.len(), 1, "{records:?}");
    assert!((edges[0].time - 1e-6).abs() < 1e-12, "{:?}", edges[0]);
    assert_eq!(field(edges[0], "level"), &Field::Int(7));
    // the record reads `count` before its non-blocking assignment lands (the Book's Blinky prints
    // `count=0` at its first edge)
    assert_eq!(field(edges[0], "count"), &Field::Int(0));
}

#[test]
fn driving_a_carrier_that_is_not_a_top_input_is_e0804() {
    // HII-12 AC 3: an `out` port of the top, an analog node and a signal of the top are carriers the
    // host holds no drive of; none changes the session, and a path that names nothing is E0801
    let mut s = transient(1e-6, &[]);
    for path in ["count", "Top.a", "armed"] {
        let failure = s.drive(path, ParamValue::Bool(true), 0.0).expect_err(path);
        assert_eq!(code(failure), "E0804", "{path}");
    }
    assert_eq!(code(s.drive("nothing", ParamValue::Bool(true), 0.0).expect_err("no such carrier")), "E0801");
    let (_, records) = run(&mut s).unwrap_or_else(|f| panic!("{f}"));
    assert!(edges(&records).is_empty(), "the refused drives changed nothing: {records:?}");
}

#[test]
fn a_drive_of_the_wrong_type_is_e0802() {
    // HII-12 AC 4: `clk` is a bit and `level` a u8: a real, an integer for the bit and 300 for
    // the u8 are not their types; the refusal names the port
    let mut s = transient(1e-6, &[]);
    for (port, value) in [("clk", ParamValue::Real(1.0)), ("clk", ParamValue::Int(1)), ("level", ParamValue::Int(300)), ("level", ParamValue::Bool(true)), ("level", ParamValue::Bytes(vec![1]))] {
        match s.drive(port, value, 0.0).expect_err(port) {
            Failure::Host { code: "E0802", message } => assert!(message.contains(port), "{message}"),
            other => panic!("E0802, found {other}"),
        }
    }
}

#[test]
fn an_undriven_top_input_is_unknown() {
    // HII-12 AC 5 (§23.1 rule 9): a transient whose host drives `level` and never `clk` has no
    // edge on `clk`, and `clk` is unknown in every row
    let mut s = transient(1e-6, &["clk", "level"]);
    s.drive("level", ParamValue::Int(3), 0.0).expect("level");
    let (table, records) = run(&mut s).unwrap_or_else(|f| panic!("{f}"));
    assert!(edges(&records).is_empty(), "{records:?}");
    assert!(table.rows.iter().all(|row| row[1] == Cell::Unknown && row[2] == Cell::Int(3)), "{table:?}");
}

#[test]
fn a_negative_at_is_e0806() {
    // HII-13 AC 4: a drive and a set at a negative time are refused, naming the action
    let mut s = transient(1e-6, &[]);
    match s.drive("clk", ParamValue::Bool(true), -1e-9).expect_err("past") {
        Failure::Host { code: "E0806", message } => assert!(message.contains("`clk`"), "{message}"),
        other => panic!("E0806, found {other}"),
    }
    match s.set_at("v1.dc", 2.0, -1e-9).expect_err("past") {
        Failure::Host { code: "E0806", message } => assert!(message.contains("`v1.dc`"), "{message}"),
        other => panic!("E0806, found {other}"),
    }
}

/// The values of column `at` of a grid table, one per row.
fn column(table: &Table, at: usize) -> Vec<f64> {
    table.rows.iter().map(|row| match &row[at] {
        Cell::Real(v) => *v,
        other => panic!("a real, found {other:?}"),
    }).collect()
}

fn grid(print: &[&str]) -> Session {
    session("transient", Overrides { stop: Some(1e-6), max_step: Some(1e-7), print_step: Some(5e-7), print: print.iter().map(|s| s.to_string()).collect(), ..Overrides::default() })
}

#[test]
fn a_set_at_zero_applies_before_the_analysis() {
    // HII-13 AC 1 (§23.1 rule 4): a `set` at 0 binds the param before the run, so the source holds
    // 2 V from the first row
    let mut s = grid(&["a"]);
    s.set_at("v1.dc", 2.0, 0.0).expect("set");
    let (table, _) = run(&mut s).unwrap_or_else(|f| panic!("{f}"));
    for v in column(&table, 1) {
        assert!((v - 2.0).abs() < 1e-9, "{table:?}");
    }
}

#[test]
fn a_timed_set_applies_after_the_step_to_its_time() {
    // HII-13 AC 2 (§23.1 rule 4): `dc` moves from 1 V to 2 V at 0.5 us; the node `a` is read by
    // that behavior and keeps 1 V at 0.5 us (the step to it is accepted before the set applies),
    // and holds 2 V at 1 us
    let mut s = grid(&["a"]);
    s.set_at("v1.dc", 2.0, 5e-7).expect("set");
    let (table, _) = run(&mut s).unwrap_or_else(|f| panic!("{f}"));
    let a = column(&table, 1);
    assert_eq!(a.len(), 3, "{table:?}");
    assert!((a[0] - 1.0).abs() < 1e-9 && (a[1] - 1.0).abs() < 1e-9, "old value up to the set: {a:?}");
    assert!((a[2] - 2.0).abs() < 1e-6, "new value after it: {a:?}");
}

#[test]
fn a_timed_set_of_a_structural_param_is_e0803() {
    // HII-13 AC 3 (§8.1.2, §23.1 rule 7): the elaborator marks no param structural yet (the
    // language's generation is outside the M1 subset), so the design's IR is marked here: its `r1.r`
    // is structural, and a set of it at 0.5 us is refused naming it, while a set of a param that is
    // not structural is not
    let s = grid(&["a"]);
    let marked = serde_json::to_string(s.ir()).expect("serialize").replace("\"structural\":false", "\"structural\":true");
    let ir: piperine_ir::Ir = serde_json::from_str(&marked).expect("deserialize");
    let r = ParamPath::resolve(&ir, ir.top().path(), "r1.r").expect("resolves");
    match r.refuse_timed(&ir).expect_err("structural") {
        Failure::Host { code: "E0803", message } => assert!(message.contains("Top.r1.r"), "{message}"),
        other => panic!("E0803, found {other}"),
    }
    let plain = ParamPath::resolve(s.ir(), s.ir().top().path(), "r1.r").expect("resolves");
    plain.refuse_timed(s.ir()).expect("a param the elaborator leaves unmarked is not refused");
}

#[test]
fn a_timed_set_failing_where_is_e0802() {
    // HII-13 AC 5 (§23.1 rule 6): `r > 0.0`: -1 fails it at 0.5 us and is refused naming the
    // param, with no analysis run; a value that passes is accepted
    let mut s = grid(&["a"]);
    match s.set_at("r1.r", -1.0, 5e-7).expect_err("where") {
        Failure::Host { code: "E0802", message } => assert!(message.contains("Top.r1.r"), "{message}"),
        other => panic!("E0802, found {other}"),
    }
    s.set_at("r1.r", 2000.0, 5e-7).expect("a value that passes the where");
    assert!(s.circuit().is_some(), "no analysis ran");
    assert_eq!(code(s.set_at("r1.nothing", 1.0, 5e-7).expect_err("no such param")), "E0801");
}
