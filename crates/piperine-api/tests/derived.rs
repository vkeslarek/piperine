//! Scope: a host `set` recomputes the params derived from the one it changes (feature `host-ii`,
//! X9, HII-18 to HII-20): `--param`, the `dc` sweep and a timed `Session::set_at` of `Mid`'s `rr`
//! reach the `Resistor` `Mid` places with `r: rr` (S2 of `GAPS.md`). Expected values are the
//! divider formulas: 1 V over `R1 = 1 kOhm` into the child `r` to ground gives `r / (r + 1k)`.
#![cfg(unix)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;

use piperine_api::{Cell, Failure, Overrides, Project, Record, RecordSink, Session, Table};
use piperine_transpile::Settings;

struct Kept;

impl RecordSink for Kept {
    fn record(&mut self, _: &Record) {}
}

fn cache() -> &'static tempfile::TempDir {
    static CACHE: OnceLock<tempfile::TempDir> = OnceLock::new();
    CACHE.get_or_init(|| tempfile::tempdir().expect("tmp"))
}

fn settings() -> Settings {
    Settings { rustc: None, cache: cache().path().to_path_buf() }
}

const PARTS: &str = "pub module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\npub module Vsource(p: Electrical, n: Electrical) { dc: f64 = 1.0; }\nanalog for Vsource { v(p, n) +> dc; }\n";

/// The one design: `Top` is `Vsource` at `a`, `R1 = 1k` from `a` to `b`, and `Mid` from `b` to
/// ground, whose resistor is `r: <derived>`; `Mid`'s own behavior reads `rr`, so a `set` of `rr`
/// has a pin (without it the run stops at the known H5 error).
fn source(derived: &str, rr: &str) -> String {
    format!(
        "{PARTS}module Mid(p: Electrical, n: Electrical) {{\n    rr: f64 = {rr};\n    let res = Resistor(p, n) {{ r: {derived} }};\n}}\nanalog for Mid {{ i(p, n) +> 0.0 * rr; }}\n\nmodule Top() {{\n    signal a: Electrical;\n    signal b: Electrical;\n    let v1 = Vsource(a, gnd) {{ dc: 1.0 }};\n    let r1 = Resistor(a, b) {{ r: 1k }};\n    let m = Mid(b, gnd);\n}}\n"
    )
}

fn open(text: &str, overrides: Overrides) -> Session {
    let dir = tempfile::tempdir().expect("tmp");
    let file = dir.path().join("bench.msl");
    std::fs::write(&file, text).expect("write");
    let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
    let print = if overrides.print.is_empty() { vec!["b".into()] } else { overrides.print.clone() };
    let overrides = Overrides { backend: Some("rust".into()), top: Some("Top".into()), print, ..overrides };
    let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
    Session::open(&project, &spec, &settings()).unwrap_or_else(|f| panic!("{f}"))
}

fn op() -> Overrides {
    Overrides { analysis: Some("op".into()), ..Overrides::default() }
}

fn run(session: &mut Session) -> Result<Table, Failure> {
    session.run(Rc::new(RefCell::new(Kept)))
}

fn real(cell: &Cell) -> f64 {
    match cell {
        Cell::Real(v) => *v,
        other => panic!("a real, found {other:?}"),
    }
}

fn close(got: f64, wanted: f64, what: &str) {
    assert!((got - wanted).abs() <= 1e-6, "{what}: {got} against {wanted}");
}

#[test]
fn the_s2_probe_prints_three_quarters() {
    // HII-18 AC 1 and 6: `--param Top.m.rr=3000` reaches the child (`r = rr`): the middle node is
    // 3000 / (3000 + 1000) = 0.75 (it was 0.5 with the child at its elaborated 1k); with no `set`
    // the design prints what it always printed, 0.5 (HII-20 AC 3)
    let mut set = open(&source("rr", "1k"), Overrides { params: vec![("Top.m.rr".into(), 3000.0)], ..op() });
    let table = run(&mut set).unwrap_or_else(|f| panic!("{f}"));
    close(real(&table.rows[0][1]), 0.75, "rr = 3000");
    let mut plain = open(&source("rr", "1k"), op());
    let table = run(&mut plain).unwrap_or_else(|f| panic!("{f}"));
    close(real(&table.rows[0][1]), 0.5, "no set");
}

#[test]
fn a_sweep_recomputes_the_child_at_each_point() {
    // HII-18 AC 2: three points of `rr`, three different rows: r / (r + 1k) for 1k, 2k, 3k
    let sweep = Overrides { analysis: Some("dc".into()), sweep: Some("Top.m.rr".into()), from: Some(1000.0), to: Some(3000.0), step: Some(1000.0), ..Overrides::default() };
    let mut session = open(&source("rr", "1k"), sweep);
    let table = run(&mut session).unwrap_or_else(|f| panic!("{f}"));
    let rows: Vec<f64> = table.rows.iter().map(|r| real(&r[1])).collect();
    assert_eq!(rows.len(), 3, "{rows:?}");
    for (got, wanted) in rows.iter().zip([0.5, 2.0 / 3.0, 0.75]) {
        close(*got, wanted, "a sweep point");
    }
}

#[test]
fn a_timed_set_recomputes_at_the_same_instant() {
    // HII-18 AC 3: `rr = 3000` at 1.5 us: the grid rows at 0 and 1 us hold 0.5, the rows at 2 and
    // 3 us hold 0.75, so the child moved with its parent at that instant
    let tran = Overrides { analysis: Some("transient".into()), stop: Some(3e-6), max_step: Some(1e-7), print_step: Some(1e-6), ..Overrides::default() };
    let mut session = open(&source("rr", "1k"), tran);
    session.set_at("Top.m.rr", 3000.0, 1.5e-6).unwrap_or_else(|f| panic!("{f}"));
    let table = run(&mut session).unwrap_or_else(|f| panic!("{f}"));
    let rows: Vec<f64> = table.rows.iter().map(|r| real(&r[1])).collect();
    assert_eq!(rows.len(), 4, "{rows:?}");
    for (got, wanted) in rows.iter().zip([0.5, 0.5, 0.75, 0.75]) {
        close(*got, wanted, "a grid row");
    }
}

#[test]
fn a_child_failing_where_refuses_the_set() {
    // HII-18 AC 4: the child's `r = rr - 500` must satisfy `r > 0`; `rr = 400` makes it -100: E0802
    // naming the child's param, and nothing changed: the run still gives r = 1000 (rr = 1500)
    let mut session = open(&source("rr - 500.0", "1500.0"), op());
    match session.set_at("Top.m.rr", 400.0, 0.0) {
        Err(Failure::Host { code: "E0802", message }) => assert!(message.contains("Top.m.res.r"), "{message}"),
        other => panic!("expected E0802, found {other:?}"),
    }
    let table = run(&mut session).unwrap_or_else(|f| panic!("{f}"));
    close(real(&table.rows[0][1]), 0.5, "the refused set changed nothing");
    // and the derived param itself is no door: HII-20 AC 1
    let mut direct = open(&source("rr - 500.0", "1500.0"), op());
    match direct.set_at("Top.m.res.r", 2000.0, 0.0) {
        Err(Failure::Host { code: "E0802", message }) => assert!(message.contains("rr") && message.contains("Top.m"), "{message}"),
        other => panic!("expected E0802, found {other:?}"),
    }
}

#[test]
fn a_set_neither_elaborates_nor_compiles() {
    // HII-18 AC 5: the cache holds the same entries after a `set` and a run as before them: no
    // second `rustc` build and no new design
    let entries = || {
        let mut names: Vec<_> = walk(cache().path());
        names.sort();
        names
    };
    let mut session = open(&source("rr", "1k"), op());
    let before = entries();
    assert!(!before.is_empty(), "the open compiled the design into the cache");
    session.set_at("Top.m.rr", 2000.0, 0.0).unwrap_or_else(|f| panic!("{f}"));
    session.set_at("Top.m.rr", 3000.0, 0.0).unwrap_or_else(|f| panic!("{f}"));
    run(&mut session).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(entries(), before);
}

/// Every file under `dir`, by path.
fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir).expect("a readable cache").flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(walk(&path));
        } else {
            found.push(path);
        }
    }
    found
}

/// `Mid` is a resistor of `rr` written through a body `let`: `g = 1 / rr` inside its behavior.
const LET_DESIGN: &str = "module Mid(p: Electrical, n: Electrical) {\n    rr: f64 = 1k;\n}\nanalog for Mid { let g = 1.0 / rr; i(p, n) +> v(p, n) * g; }\n\nmodule Top() {\n    signal a: Electrical;\n    signal b: Electrical;\n    let v1 = Vsource(a, gnd) { dc: 1.0 };\n    let r1 = Resistor(a, b) { r: 1k };\n    let m = Mid(b, gnd);\n}\n";

#[test]
fn a_body_let_reads_the_new_param() {
    // HII-19 AC 1: `g = 1.0 / rr` is a `let` of `Mid`'s behavior reading `rr`; after `rr = 3000`
    // it reads 3000 on every evaluation, so the node is 3000 / (3000 + 1000) = 0.75 (0.5 before)
    let text = format!("{PARTS}{LET_DESIGN}");
    let mut before = open(&text, op());
    close(real(&run(&mut before).unwrap_or_else(|f| panic!("{f}")).rows[0][1]), 0.5, "rr = 1k");
    let mut after = open(&text, op());
    after.set_at("Top.m.rr", 3000.0, 0.0).unwrap_or_else(|f| panic!("{f}"));
    close(real(&run(&mut after).unwrap_or_else(|f| panic!("{f}")).rows[0][1]), 0.75, "rr = 3000");
}

/// `Scale` defines `y = a * k`: a definition of a carrier that reads the param `k`; the top's `a`
/// is driven by the host.
const DEFINITION_DESIGN: &str = "module Scale(in a: Bit<f64>, out y: Bit<f64>) {\n    k: f64 = 2.0;\n    y = a * k;\n}\n\nmodule Top(in a: Bit<f64>, out y: Bit<f64>) {\n    signal s: Electrical;\n    let v1 = Vsource(s, gnd) { dc: 1.0 };\n    let r1 = Resistor(s, gnd) { r: 1k };\n    let sc = Scale(a, y);\n}\n";

#[test]
fn a_definition_is_recomputed_at_the_set() {
    // HII-19 AC 2: `y = a * k` with `a = 4`: 8 while `k = 2`; `k = 5` at 1.5 us recomputes the
    // definition at that instant: the grid rows at 0 and 1 us hold 8, the rows at 2 and 3 us hold 20
    let tran = Overrides { analysis: Some("transient".into()), stop: Some(3e-6), max_step: Some(1e-7), print_step: Some(1e-6), print: vec!["y".into()], ..Overrides::default() };
    let mut session = open(&format!("{PARTS}{DEFINITION_DESIGN}"), tran);
    session.drive("a", piperine_api::ParamValue::Real(4.0), 0.0).unwrap_or_else(|f| panic!("{f}"));
    session.set_at("Top.sc.k", 5.0, 1.5e-6).unwrap_or_else(|f| panic!("{f}"));
    let table = run(&mut session).unwrap_or_else(|f| panic!("{f}"));
    let rows: Vec<f64> = table.rows.iter().map(|r| real(&r[1])).collect();
    assert_eq!(rows, [8.0, 8.0, 20.0, 20.0]);
}
