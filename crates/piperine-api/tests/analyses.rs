//! `Session::run` and the tables it gives (CLI-05 AC 1, 2, 7 and 8): the operating point as
//! signal and value in print order, the divider of `tests/ngspice/divider.msl` and the RC of
//! `tests/ngspice/tran_rc.msl` on both backends. Expected values are computed here from the
//! circuit each source writes; the tolerances are the spec's (1e-9 for exact arithmetic, 1e-3
//! for the RC step). Rust-backend sessions build into a cache directory of this target.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::OnceLock;

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
fn settings() -> Settings {
    static CACHE: OnceLock<tempfile::TempDir> = OnceLock::new();
    let dir = CACHE.get_or_init(|| tempfile::tempdir().expect("tmp"));
    Settings { rustc: None, cache: dir.path().to_path_buf() }
}

/// The run of the lone file `path` with `overrides`, on `backend`.
fn run(path: &Path, backend: &str, overrides: Overrides) -> Result<Table, Failure> {
    let project = Project::file(path).unwrap_or_else(|f| panic!("{f}"));
    let spec = project.request(None, &Overrides { backend: Some(backend.into()), ..overrides }).unwrap_or_else(|f| panic!("{f}"));
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

fn dc(sweep: &str, from: f64, to: f64, step: f64, print: &[&str]) -> Overrides {
    Overrides {
        analysis: Some("dc".into()),
        sweep: Some(sweep.into()),
        from: Some(from),
        to: Some(to),
        step: Some(step),
        print: print.iter().map(|s| s.to_string()).collect(),
        ..Overrides::default()
    }
}

#[test]
fn a_dc_sweep_has_one_row_per_point() {
    // CLI-09 AC 1: the first column is the swept param's full path, one row per `from + k*step`
    // while not past `to`: 0, 2.5, 5, 7.5, 10 on divider.msl; `mid` is 3k / (1k + 3k) of the source
    for backend in ["interp", "rust"] {
        let table = run(&twin("divider.msl"), backend, dc("v1.dc", 0.0, 10.0, 2.5, &["mid"])).unwrap_or_else(|f| panic!("{backend}: {f}"));
        assert_eq!(table.columns, ["Divider.v1.dc", "Divider.mid"], "{backend}");
        let swept: Vec<f64> = table.rows.iter().map(|r| real(&r[0])).collect();
        assert_eq!(swept, [0.0, 2.5, 5.0, 7.5, 10.0], "{backend}");
        for row in &table.rows {
            let (v, mid) = (real(&row[0]), real(&row[1]));
            assert!((mid - 0.75 * v).abs() <= 1e-9, "{backend}: mid({v}) = {mid}");
        }
    }
    // a step that does not divide the range stops before `to`: 0, 3, 6, 9
    let table = run(&twin("divider.msl"), "interp", dc("v1.dc", 0.0, 10.0, 3.0, &["mid"])).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(table.rows.len(), 4);
    // a downward sweep: 10, 5, 0
    let table = run(&twin("divider.msl"), "interp", dc("v1.dc", 10.0, 0.0, -5.0, &["mid"])).unwrap_or_else(|f| panic!("{f}"));
    let swept: Vec<f64> = table.rows.iter().map(|r| real(&r[0])).collect();
    assert_eq!(swept, [10.0, 5.0, 0.0]);
}

#[test]
fn the_diode_sweep_has_37_rows() {
    // CLI-09 AC 8: diode_iv.msl sweeps v1.dc from -1 to 0.8 by 0.05: (0.8 + 1) / 0.05 + 1 = 37
    for backend in ["interp", "rust"] {
        let table = run(&twin("diode_iv.msl"), backend, dc("v1.dc", -1.0, 0.8, 0.05, &["a"])).unwrap_or_else(|f| panic!("{backend}: {f}"));
        assert_eq!(table.rows.len(), 37, "{backend}");
        assert_eq!(real(&table.rows[0][0]), -1.0);
        assert!((real(&table.rows[36][0]) - 0.8).abs() <= 1e-9);
    }
}

#[test]
fn a_sweep_of_no_param_is_e0801() {
    // CLI-09 AC 9: a path that names no param is E0801 naming it, at open: no analysis ran
    for sweep in ["v1.nothing", "nobody.dc", "dc"] {
        match run(&twin("divider.msl"), "interp", dc(sweep, 0.0, 1.0, 0.5, &[])) {
            Err(Failure::Host { code: "E0801", message }) => assert!(message.contains(sweep), "{message}"),
            other => panic!("`{sweep}`: expected E0801, found {other:?}"),
        }
    }
}

fn ac(from: f64, to: f64, per_decade: Option<u32>, points: Option<u32>, print: &[&str]) -> Overrides {
    Overrides {
        analysis: Some("ac".into()),
        from: Some(from),
        to: Some(to),
        per_decade,
        points,
        print: print.iter().map(|s| s.to_string()).collect(),
        ..Overrides::default()
    }
}

#[test]
fn an_ac_sweep_has_mag_and_phase_in_degrees() {
    // CLI-09 AC 2: `freq`, then `mag(<path>)` and `phase(<path>)` per printed signal; ac_rc.msl
    // from 1e3 to 1e7 at 10 per decade is 41 rows, the first at 1e3 and the last at 1e7; far
    // below the corner the RC passes everything (mag 1, phase near 0), far above it the phase
    // heads for -90 degrees
    for backend in ["interp", "rust"] {
        let table = run(&twin("ac_rc.msl"), backend, ac(1e3, 1e7, Some(10), None, &["out", "vin"])).unwrap_or_else(|f| panic!("{backend}: {f}"));
        assert_eq!(table.columns, ["freq", "mag(AcRc.out)", "phase(AcRc.out)", "mag(AcRc.vin)", "phase(AcRc.vin)"], "{backend}");
        assert_eq!(table.rows.len(), 41, "{backend}");
        assert!((real(&table.rows[0][0]) - 1e3).abs() <= 1e-6 && (real(&table.rows[40][0]) - 1e7).abs() <= 1.0, "{backend}");
        let (mag, phase) = (real(&table.rows[0][1]), real(&table.rows[0][2]));
        assert!((mag - 1.0).abs() <= 1e-3, "{backend}: |out| at 1 kHz = {mag}");
        assert!(phase < 0.0 && phase > -2.0, "{backend}: phase {phase} degrees at 1 kHz");
        assert!(real(&table.rows[40][2]) < -85.0, "{backend}: phase at 10 MHz");
        // the source is the 1 V stimulus: magnitude 1, phase 0
        assert!((real(&table.rows[10][3]) - 1.0).abs() <= 1e-9 && real(&table.rows[10][4]).abs() <= 1e-9, "{backend}");
    }
}

#[test]
fn points_sweeps_linearly() {
    // CLI-09 AC 6: `points = 5` from 1e3 to 5e3 is 1e3, 2e3, 3e3, 4e3, 5e3
    let table = run(&twin("ac_rc.msl"), "interp", ac(1e3, 5e3, None, Some(5), &["out"])).unwrap_or_else(|f| panic!("{f}"));
    let freqs: Vec<f64> = table.rows.iter().map(|r| real(&r[0])).collect();
    for (got, want) in freqs.iter().zip([1e3, 2e3, 3e3, 4e3, 5e3]) {
        assert!((got - want).abs() <= 1e-6, "{freqs:?}");
    }
    assert_eq!(freqs.len(), 5);
}

#[test]
fn the_rc_corner_is_minus_3_db_and_minus_45_degrees() {
    // CLI-09 AC 7: ac_rc.msl, 1e3 to 1e7 at 10 per decade, at 1e5 the magnitude is within 1e-3 of
    // 0.7071067811865476 and the phase within 1e-2 degrees of -45 (the corner is exactly 100 kHz)
    for backend in ["interp", "rust"] {
        let table = run(&twin("ac_rc.msl"), backend, ac(1e3, 1e7, Some(10), None, &["out"])).unwrap_or_else(|f| panic!("{backend}: {f}"));
        let row = table.rows.iter().find(|r| (real(&r[0]) / 1e5 - 1.0).abs() <= 1e-9).unwrap_or_else(|| panic!("{backend}: no row at 1e5"));
        let (mag, phase) = (real(&row[1]), real(&row[2]));
        assert!((mag - 0.7071067811865476).abs() <= 1e-3, "{backend}: mag {mag}");
        assert!((phase + 45.0).abs() <= 1e-2, "{backend}: phase {phase}");
    }
}

fn with_params(overrides: Overrides, params: &[(&str, f64)]) -> Overrides {
    Overrides { params: params.iter().map(|(p, v)| (p.to_string(), *v)).collect(), ..overrides }
}

#[test]
fn params_are_set_in_order_table_then_flags() {
    // CLI-10 AC 1 and 4: divider.msl is 10 V over r1 = 1k then r2 = 3k; r2.r = 1000 gives
    // 10 * 1000 / (1000 + 1000) = 5 for `mid`; the entries apply in order, so a later write of
    // the same param wins: r2.r = 9000 then r2.r = 1000 is 5, and 1000 then 9000 is 10 * 9/10 = 9
    for backend in ["interp", "rust"] {
        let mid = |params: &[(&str, f64)]| {
            let table = run(&twin("divider.msl"), backend, with_params(op(&["mid"]), params)).unwrap_or_else(|f| panic!("{backend}: {f}"));
            real(value(&table, "Divider.mid"))
        };
        assert!((mid(&[("r2.r", 1000.0)]) - 5.0).abs() <= 1e-9, "{backend}");
        assert!((mid(&[("r2.r", 9000.0), ("r2.r", 1000.0)]) - 5.0).abs() <= 1e-9, "{backend}");
        assert!((mid(&[("r2.r", 1000.0), ("r2.r", 9000.0)]) - 9.0).abs() <= 1e-9, "{backend}");
        // the full path names the same param
        assert!((mid(&[("Divider.r2.r", 1000.0)]) - 5.0).abs() <= 1e-9, "{backend}");
    }
}

#[test]
fn a_param_path_that_names_nothing_is_e0801() {
    // CLI-10 AC 2: E0801 naming the path, for a missing instance, a missing param and a path
    // with no instance; a typo is found before anything compiles
    for path in ["r9.r", "r2.nothing", "r"] {
        match run(&twin("divider.msl"), "rust", with_params(op(&[]), &[(path, 1.0)])) {
            Err(Failure::Host { code: "E0801", message }) => assert!(message.contains(path), "{message}"),
            other => panic!("`{path}`: expected E0801, found {other:?}"),
        }
    }
}

#[test]
fn a_refused_value_is_e0802_and_nothing_runs() {
    // CLI-10 AC 3: `Resistor`'s `r: f64 where r > 0.0` refuses -5: E0802 naming the path and the
    // value, and no table
    for backend in ["interp", "rust"] {
        match run(&twin("divider.msl"), backend, with_params(op(&[]), &[("r2.r", -5.0)])) {
            Err(Failure::Host { code: "E0802", message }) => assert!(message.contains("r2.r") && message.contains("-5"), "{backend}: {message}"),
            other => panic!("{backend}: expected E0802, found {other:?}"),
        }
    }
}
