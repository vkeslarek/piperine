//! What `print` can name beyond a node (HII-09): a param prints its value in every row, and an
//! instance with no potential contribution and one flow contribution prints that flow at each
//! solution; several flow contributions are `E0801`. The designs are written here from their parts.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;

use piperine_api::{Cell, Failure, Overrides, Project, Record, RecordSink, Session, Table};
use piperine_transpile::Settings;

struct Ignore;

impl RecordSink for Ignore {
    fn record(&mut self, _: &Record) {}
}

fn settings() -> Settings {
    static CACHE: OnceLock<tempfile::TempDir> = OnceLock::new();
    let dir = CACHE.get_or_init(|| tempfile::tempdir().expect("tmp"));
    Settings { rustc: None, cache: dir.path().to_path_buf() }
}

/// `Top`: 10 V over 1 kOhm then 3 kOhm, built from the parts; `Pair` has two flow contributions
/// and no potential one.
const DESIGN: &str = "pub module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\npub module Vsource(p: Electrical, n: Electrical) { dc: f64 = 1.0; }\nanalog for Vsource { v(p, n) +> dc; }\nmodule Pair(a: Electrical, b: Electrical) { g: f64 = 1m; }\nanalog for Pair { i(a) +> v(a) * g; i(b) +> v(b) * g; }\nmodule Top() {\n    signal vin: Electrical;\n    signal mid: Electrical;\n    let v1 = Vsource(vin, gnd) { dc: 10.0 };\n    let r1 = Resistor(vin, mid) { r: 1k };\n    let r2 = Resistor(mid, gnd) { r: 3k };\n    let pr = Pair(vin, mid);\n}\n";

fn run(overrides: Overrides) -> Result<Table, Failure> {
    let dir = tempfile::tempdir().expect("tmp");
    let file = dir.path().join("bench.msl");
    std::fs::write(&file, DESIGN).expect("write");
    let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
    let spec = project.request(None, &Overrides { backend: Some("rust".into()), top: Some("Top".into()), ..overrides }).unwrap_or_else(|f| panic!("{f}"));
    let mut session = Session::open(&project, &spec, &settings())?;
    session.run(Rc::new(RefCell::new(Ignore)))
}

fn op(print: &[&str]) -> Overrides {
    Overrides { analysis: Some("op".into()), print: print.iter().map(|s| s.to_string()).collect(), ..Overrides::default() }
}

fn real(cell: &Cell) -> f64 {
    match cell {
        Cell::Real(v) => *v,
        other => panic!("a real, found {other:?}"),
    }
}

#[test]
fn a_param_prints_its_value_in_every_row() {
    // HII-09 AC 1: `<instance>.<param>` is the value the run used, the elaborated one or a set one,
    // in the operating point and in every row of a transient
    let table = run(op(&["r1.r", "Top.r2.r"])).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(table.rows[0], [Cell::Text("Top.r1.r".into()), Cell::Real(1000.0)]);
    assert_eq!(table.rows[1], [Cell::Text("Top.r2.r".into()), Cell::Real(3000.0)]);
    let set = Overrides { params: vec![("r1.r".into(), 2000.0)], ..op(&["r1.r"]) };
    assert_eq!(run(set).unwrap_or_else(|f| panic!("{f}")).rows[0][1], Cell::Real(2000.0));
    let tran = Overrides { analysis: Some("transient".into()), stop: Some(1e-6), print_step: Some(5e-7), params: vec![("r1.r".into(), 2000.0)], print: vec!["r1.r".into(), "mid".into()], ..Overrides::default() };
    let table = run(tran).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(table.rows.len(), 3);
    for row in &table.rows {
        assert_eq!(real(&row[1]), 2000.0, "{row:?}");
    }
}

#[test]
fn the_flow_of_a_single_flow_contribution_prints() {
    // HII-09 AC 2: `r1` has no potential contribution and one flow contribution: `i(r1)` is its
    // flow at the solution, (vin - mid) over 1 kOhm
    let table = run(op(&["i(r1)", "i(r2)"])).unwrap_or_else(|f| panic!("{f}"));
    // `pr` also loads `vin` and `mid`, so the expected flows come from the solved `mid`
    let (r1, r2) = (real(&table.rows[0][1]), real(&table.rows[1][1]));
    assert!(r1 > 0.0 && r2 > 0.0, "{table:?}");
    let (vin, mid) = (10.0, real(&run(op(&["mid"])).unwrap_or_else(|f| panic!("{f}")).rows[0][1]));
    assert!((r1 - (vin - mid) / 1000.0).abs() < 1e-9, "i(r1) = {r1}");
    assert!((r2 - mid / 3000.0).abs() < 1e-9, "i(r2) = {r2}");
}

#[test]
fn a_flow_of_several_contributions_is_e0801() {
    // HII-09 AC 3: `pr` has two flow contributions and no potential one: E0801 naming the instance
    // and its contributions
    let Err(Failure::Host { code, message }) = run(op(&["i(pr)"])) else { panic!("several flow contributions are refused") };
    assert_eq!(code, "E0801");
    assert!(message.contains("Top.pr") && message.contains("b0.flow") && message.contains("b1.flow"), "{message}");
}
