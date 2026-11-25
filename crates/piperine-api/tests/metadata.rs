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
    run_text(DESIGN, overrides)
}

/// The table of the design `text` run with `overrides`.
fn run_text(text: &str, overrides: Overrides) -> Result<Table, Failure> {
    let dir = tempfile::tempdir().expect("tmp");
    let file = dir.path().join("bench.msl");
    std::fs::write(&file, text).expect("write");
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

/// A design with a discrete port and a stream: a counter with a bounded param.
const PORTS: &str = "module Counter(in clk: Bit, out q: Bit<u8> = 0) { limit: f64 = 9.0 where limit > 0.0; wide: bool = true; }\ndigital for Counter { on clk.rise() { q <- q + 1; emit!(\"tick\", n = q); } }\nmodule Top() {\n    signal clk: Bit = false;\n    signal q: Bit<u8>;\n    let c = Counter(clk, q) { limit: 7.0 };\n}\n";

fn opened(text: &str, overrides: Overrides) -> Session {
    let dir = tempfile::tempdir().expect("tmp");
    let file = dir.path().join("bench.msl");
    std::fs::write(&file, text).expect("write");
    let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
    let spec = project.request(None, &Overrides { backend: Some("rust".into()), top: Some("Top".into()), ..overrides }).unwrap_or_else(|f| panic!("{f}"));
    Session::open(&project, &spec, &settings()).unwrap_or_else(|f| panic!("{f}"))
}

#[test]
fn metadata_lists_every_param_with_type_value_where_and_structural() {
    // HII-10 AC 1: full path, type, value (a set one shows), `where` text or none, structural, in instance order
    let set = Overrides { params: vec![("r1.r".into(), 2000.0)], ..op(&[]) };
    let session = opened(DESIGN, set);
    let params = session.metadata().params;
    let find = |path: &str| params.iter().find(|p| p.path == path).unwrap_or_else(|| panic!("no {path} in {params:?}"));
    let r1 = find("Top.r1.r");
    assert_eq!((r1.name.as_str(), r1.ty.as_str(), r1.structural), ("r", "f64", false));
    assert_eq!(r1.value, Cell::Real(2000.0), "a set value shows");
    assert!(r1.check.as_deref().is_some_and(|c| c.contains('r') && c.contains("0.0")), "{:?}", r1.check);
    assert_eq!(find("Top.r2.r").value, Cell::Real(3000.0));
    assert_eq!(find("Top.v1.dc").check, None, "no `where` is none");
    assert_eq!(find("Top.v1.dc").value, Cell::Real(10.0));
    let order: Vec<&str> = params.iter().map(|p| p.path.as_str()).collect();
    assert!(order.iter().position(|p| *p == "Top.v1.dc") < order.iter().position(|p| *p == "Top.r1.r"), "instance order: {order:?}");
}

#[test]
fn metadata_lists_ports_and_streams() {
    // HII-10 AC 2: the top's ports with direction and type in declaration order, and every stream
    // with its fields and types
    let session = opened(PORTS, op(&[]));
    let metadata = session.metadata();
    let counter: Vec<&str> = metadata.params.iter().filter(|p| p.path.starts_with("Top.c.")).map(|p| p.path.as_str()).collect();
    assert_eq!(counter, ["Top.c.limit", "Top.c.wide"]);
    assert_eq!(metadata.params[0].value, Cell::Real(7.0));
    assert_eq!((metadata.params[1].ty.as_str(), &metadata.params[1].value), ("bool", &Cell::Bool(true)));
    assert!(metadata.params[0].check.is_some());
    assert_eq!(metadata.streams.len(), 1, "{:?}", metadata.streams);
    assert_eq!(metadata.streams[0].path, "Top.c.tick");
    assert_eq!(metadata.streams[0].fields, [("n".to_string(), "u8".to_string())]);
    // the top of this design declares no port: its list is empty; a port is listed for `Counter` as a top
    assert!(metadata.ports.is_empty(), "{:?}", metadata.ports);
    let as_top = Overrides { top: Some("Counter".into()), ..op(&[]) };
    let dir = tempfile::tempdir().expect("tmp");
    let file = dir.path().join("bench.msl");
    std::fs::write(&file, PORTS).expect("write");
    let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
    let spec = project.request(None, &Overrides { backend: Some("rust".into()), ..as_top }).unwrap_or_else(|f| panic!("{f}"));
    let session = Session::open(&project, &spec, &settings()).unwrap_or_else(|f| panic!("{f}"));
    let ports = session.metadata().ports;
    assert_eq!(ports.iter().map(|p| (p.name.as_str(), p.direction, p.ty.as_str())).collect::<Vec<_>>(), [("clk", "in", "bool"), ("q", "out", "u8")]);
}

#[test]
fn metadata_runs_no_analysis() {
    // HII-10 AC 3: the session answers before `run`, and a run afterwards still gives its table
    let mut session = opened(DESIGN, op(&["mid"]));
    let before = session.metadata();
    assert!(!before.params.is_empty());
    let table = session.run(Rc::new(RefCell::new(Ignore))).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(table.rows.len(), 1);
}

/// A design with a bundle: a producer holds `valid` high and `data` at 5, and the top declares one `Stream` signal, joins the
/// producer to it and a consumer that holds `ready` low.
const STREAM: &str = "struct Stream { out valid: Bit, in ready: Bit, out data: Bit<u8> }\n\
module Prod(tx: Stream) { tx.valid = true; tx.data = 5; }\n\
module Cons(rx: flip Stream) { rx.ready = false; }\n\
module Top() { signal s: Stream; let p = Prod(s); let c = Cons(s); }\n";

#[test]
fn a_bundle_field_is_named_by_its_dotted_path() {
    // BN-07 AC 1, §7 rule 12: a field of a bundle is a carrier named by its path under the instance, so `s.valid` prints as
    // `Top.s.valid`, its value as the carrier holds it, and the top's own bundle port lists one port per field by path
    let table = run_text(STREAM, op(&["s.valid", "Top.s.data", "s.ready"])).unwrap_or_else(|f| panic!("{f}"));
    let names: Vec<&Cell> = table.rows.iter().map(|row| &row[0]).collect();
    assert_eq!(names, [&Cell::Text("Top.s.valid".into()), &Cell::Text("Top.s.data".into()), &Cell::Text("Top.s.ready".into())]);
    assert_eq!(table.rows[0][1], Cell::Bool(true), "the producer's `valid`");
    assert_eq!(table.rows[1][1], Cell::Int(5), "the producer's `data`");
    assert_eq!(table.rows[2][1], Cell::Bool(false), "the consumer's `ready`");
    // the top of this design is `Prod`, its port `tx`: one entry per field, by path, with the direction the view gives
    let dir = tempfile::tempdir().expect("tmp");
    let file = dir.path().join("bench.msl");
    std::fs::write(&file, STREAM).expect("write");
    let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
    let spec = project.request(None, &Overrides { backend: Some("rust".into()), top: Some("Prod".into()), ..op(&[]) }).unwrap_or_else(|f| panic!("{f}"));
    let ports = Session::open(&project, &spec, &settings()).unwrap_or_else(|f| panic!("{f}")).metadata().ports;
    assert_eq!(
        ports.iter().map(|p| (p.name.as_str(), p.direction, p.ty.as_str())).collect::<Vec<_>>(),
        [("tx.valid", "out", "bool"), ("tx.ready", "in", "bool"), ("tx.data", "out", "u8")]
    );
}

/// A design with arrays of bundles: two lanes, lane 0 raised and lane 1 lowered, each echoed, and two taps of a node each,
/// held at 1 V and 2 V.
const LANES: &str = "struct Lane { out a: Bit, in b: Bit }\n\
struct Tap { p: Electrical }\n\
module Raise(tx: Lane) { level: bool = true; tx.a = level; }\n\
module Echo(rx: flip Lane) { rx.b = rx.a; }\n\
module Hold(t: Tap) { volts: f64 = 1.0; }\n\
analog for Hold { v(t.p) +> volts; }\n\
module Top() { signal lanes: [Lane; 2]; signal taps: [Tap; 2]; let r = Raise(lanes[0]) { level: true }; let q = Raise(lanes[1]) { level: false }; \
for k in 0..2 { let e = Echo(lanes[k]); } let h = Hold(taps[0]) { volts: 1.0 }; let g = Hold(taps[1]) { volts: 2.0 }; }\n";

#[test]
fn a_field_of_an_element_of_an_array_of_bundles_is_named_element_first() {
    // BN-07 AC 1, §7 rules 11 and 12: the field of an element is named as the source writes it, `lanes[1].a`, whether it is a
    // carrier (the element `1` of the array net `lanes.a`, struct of arrays) or a node (`taps[1].p`), relative or in full
    let entries = ["lanes[0].a", "Top.lanes[1].a", "lanes[1].b", "taps[1].p", "Top.taps[0].p"];
    let table = run_text(LANES, op(&entries)).unwrap_or_else(|f| panic!("{f}"));
    let names: Vec<&Cell> = table.rows.iter().map(|row| &row[0]).collect();
    let written = ["Top.lanes[0].a", "Top.lanes[1].a", "Top.lanes[1].b", "Top.taps[1].p", "Top.taps[0].p"].map(|name| Cell::Text(name.into()));
    assert_eq!(names, written.iter().collect::<Vec<_>>());
    assert_eq!(table.rows[0][1], Cell::Bool(true), "lane 0 is raised");
    assert_eq!(table.rows[1][1], Cell::Bool(false), "lane 1 is lowered");
    assert_eq!(table.rows[2][1], Cell::Bool(false), "the echo of lane 1 follows its `a`");
    assert!((real(&table.rows[3][1]) - 2.0).abs() < 1e-9, "tap 1 is held at 2 V: {:?}", table.rows[3][1]);
    assert!((real(&table.rows[4][1]) - 1.0).abs() < 1e-9, "tap 0 is held at 1 V: {:?}", table.rows[4][1]);
}
