//! `Session::run` and the tables it gives (CLI-05 AC 1, 2, 7 and 8): the operating point as
//! signal and value in print order, the divider of `tests/ngspice/divider.msl` and the RC of
//! `tests/ngspice/tran_rc.msl` on both backends. Expected values are computed here from the
//! circuit each source writes; the tolerances are the spec's (1e-9 for exact arithmetic, 1e-3
//! for the RC step). Rust-backend sessions build into a cache directory of this target.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};

use piperine_api::{Cell, Failure, Overrides, Project, Record, RecordSink, Session, Table};
use piperine_transpile::Settings;

/// A sink that keeps nothing: these tests read tables.
struct Ignore;

impl RecordSink for Ignore {
    fn record(&mut self, _: &Record) {}
}

fn twin(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/ngspice").join(name)
}

/// One cache directory for every Rust-backend session of this target.
/// One build at a time into the shared cache: `Cache::at` probes the directory with a name
/// that is the process's (`.probe-<pid>`), so two builds of one process at once collide on it.
static BUILD: Mutex<()> = Mutex::new(());

fn settings() -> Settings {
    static CACHE: OnceLock<tempfile::TempDir> = OnceLock::new();
    let dir = CACHE.get_or_init(|| tempfile::tempdir().expect("tmp"));
    Settings { rustc: None, cache: dir.path().to_path_buf() }
}

/// The run of the lone file `path` with `overrides`, on `backend`.
fn run(path: &Path, backend: &str, overrides: Overrides) -> Result<Table, Failure> {
    let project = Project::file(path).unwrap_or_else(|f| panic!("{f}"));
    let spec = project.request(None, &Overrides { backend: Some(backend.into()), ..overrides }).unwrap_or_else(|f| panic!("{f}"));
    let mut session = {
        let _turn = BUILD.lock().unwrap_or_else(|e| e.into_inner());
        Session::open(&project, &spec, &settings())?
    };
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

/// The value of the row named `signal` of an operating point.
fn value<'t>(table: &'t Table, signal: &str) -> &'t Cell {
    let row = table.rows.iter().find(|r| r[0] == Cell::Text(signal.into())).unwrap_or_else(|| panic!("no row `{signal}` in {table:?}"));
    &row[1]
}

#[test]
fn an_operating_point_is_signal_and_value_in_print_order() {
    // CLI-05 AC 1: divider.msl, 10 V over 1 kOhm then 3 kOhm
    let table = run(&twin("divider.msl"), "interp", op(&["mid", "vin", "i(v1)"])).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(table.columns, ["signal", "value"]);
    let names: Vec<&Cell> = table.rows.iter().map(|r| &r[0]).collect();
    assert_eq!(names, [&Cell::Text("Divider.mid".into()), &Cell::Text("Divider.vin".into()), &Cell::Text("i(Divider.v1)".into())]);
    assert!((real(&table.rows[1][1]) - 10.0).abs() <= 1e-9, "vin is the source's 10 V");
    // the source drives 10 V / 4 kOhm = 2.5 mA; the sign of a branch flow is the solver's
    // convention, which the spec does not fix, so the magnitude is asserted
    assert!((real(&table.rows[2][1]).abs() - 2.5e-3).abs() <= 1e-9, "{:?}", table.rows[2][1]);
    // a discrete net is decoded by its type: `flag` holds its initial `false`, the undriven
    // in-port `en` is unknown (spec edge 5)
    let dir = tempfile::tempdir().expect("tmp");
    let file = dir.path().join("mixed.msl");
    let text = "pub module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\npub module Vsource(p: Electrical, n: Electrical) { dc: f64 = 1.0; }\nanalog for Vsource { v(p, n) +> dc; }\n\nmodule Top(in en: Bit) {\n    signal a: Electrical;\n    signal flag: Bit = false;\n    let v1 = Vsource(a, gnd) { dc: 2.0 };\n    let r1 = Resistor(a, gnd) { r: 1k };\n}\n";
    std::fs::write(&file, text).expect("write");
    let overrides = Overrides { top: Some("Top".into()), ..op(&["flag", "a", "en"]) };
    let table = run(&file, "rust", overrides).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(table.rows, [
        vec![Cell::Text("Top.flag".into()), Cell::Bool(false)],
        vec![Cell::Text("Top.a".into()), table.rows[1][1].clone()],
        vec![Cell::Text("Top.en".into()), Cell::Unknown],
    ]);
    assert!((real(&table.rows[1][1]) - 2.0).abs() <= 1e-9, "a is the source's 2 V");
}

#[test]
fn the_divider_mid_is_7_5_on_both_backends() {
    // CLI-05 AC 7: 10 * 3k / (1k + 3k) = 7.5, within 1e-9 (exact arithmetic)
    for backend in ["interp", "rust"] {
        let table = run(&twin("divider.msl"), backend, op(&[])).unwrap_or_else(|f| panic!("{backend}: {f}"));
        let mid = real(value(&table, "Divider.mid"));
        assert!((mid - 7.5).abs() <= 1e-9, "{backend}: Divider.mid = {mid}");
    }
}

fn transient(stop: f64, print: &[&str]) -> Overrides {
    Overrides { analysis: Some("transient".into()), stop: Some(stop), print: print.iter().map(|s| s.to_string()).collect(), ..Overrides::default() }
}

fn times(table: &Table) -> Vec<f64> {
    table.rows.iter().map(|r| real(&r[0])).collect()
}

#[test]
fn a_transient_has_one_row_per_accepted_step() {
    // CLI-05 AC 2: tran_rc.msl to 10 us, two printed signals
    let table = run(&twin("tran_rc.msl"), "interp", transient(10e-6, &["vin", "out"])).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(table.columns, ["time", "TranRc.vin", "TranRc.out"]);
    assert!(table.rows.iter().all(|r| r.len() == 3), "one cell per column");
    let t = times(&table);
    assert!(t.len() > 2, "{} rows", t.len());
    assert_eq!(t[0], 0.0, "the first row is the operating point at 0");
    assert!(t.windows(2).all(|w| w[0] < w[1]), "times increase strictly");
    assert_eq!(t[t.len() - 1], 10e-6, "the last accepted step is `stop`");
    // the held node starts at 0 V (tran_rc.msl: `out = 0.0`)
    assert_eq!(table.rows[0][2], Cell::Real(0.0));
}

#[test]
fn the_rc_step_follows_one_minus_exp_on_both_backends() {
    // CLI-05 AC 8: R = 1 kOhm, C = 1.5915494309189535 nF; at the first row at or after RC,
    // `out` is within 1e-3 of 1 - exp(-t / RC)
    let rc = 1000.0 * 1.5915494309189535e-9;
    for backend in ["interp", "rust"] {
        let table = run(&twin("tran_rc.msl"), backend, transient(10e-6, &["out"])).unwrap_or_else(|f| panic!("{backend}: {f}"));
        let row = table.rows.iter().find(|r| real(&r[0]) >= rc).unwrap_or_else(|| panic!("{backend}: no row at or after RC"));
        let (t, out) = (real(&row[0]), real(&row[1]));
        let expected = 1.0 - (-t / rc).exp();
        assert!((out - expected).abs() <= 1e-3, "{backend}: out({t}) = {out}, expected {expected}");
    }
}
