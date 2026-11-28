//! `Session::run` and the tables it gives (CLI-05 AC 1, 2, 7 and 8): the operating point as
//! signal and value in print order, the divider of `tests/e2e/twins/divider.msl` and the RC of
//! `tests/e2e/twins/tran_rc.msl` on both backends. Expected values are computed here from the
//! circuit each source writes; the tolerances are the spec's (1e-9 for exact arithmetic, 1e-3
//! for the RC step). Rust-backend sessions build into a cache directory of this target.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use piperine_api::{Cell, Failure, Overrides, Project, Session, Table};
use piperine_parity::compile_cache::CompileCache;
use piperine_parity::sinks::Ignore;

/// A design of the analysis tests run on a backend: a twin file, a source text or the PSS file.
struct AnalysisRun;

impl AnalysisRun {
    fn twin(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/e2e/twins").join(name)
    }

    /// The run of the lone file `path` with `overrides`, on `backend`.
    fn run(path: &Path, backend: &str, overrides: Overrides) -> Result<Table, Failure> {
        let project = Project::file(path).unwrap_or_else(|f| panic!("{f}"));
        let spec = project.request(None, &Overrides { backend: Some(backend.into()), ..overrides }).unwrap_or_else(|f| panic!("{f}"));
        let mut session = Session::open(&project, &spec, &CompileCache::shared().settings())?;
        session.run(Rc::new(RefCell::new(Ignore)))
    }

    /// The lone file `text` run with `overrides`: a source over a divider, `Top`.
    fn run_text(text: &str, overrides: Overrides) -> Result<Table, Failure> {
        let dir = tempfile::tempdir().expect("tmp");
        let file = dir.path().join("bench.msl");
        std::fs::write(&file, text).expect("write");
        AnalysisRun::run(&file, "interp", overrides)
    }

    fn message(failure: &Failure) -> String {
        format!("{failure}")
    }

    fn pss_file() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("tmp");
        let file = dir.path().join("bench.msl");
        std::fs::write(&file, PSS_RC).expect("write");
        (dir, file)
    }

    fn pss_run(overrides: Overrides) -> Result<Table, Failure> {
        let (_dir, file) = AnalysisRun::pss_file();
        AnalysisRun::run(&file, "rust", overrides)
    }
}

/// The overrides that ask for one analysis.
struct AnalysisOverrides;

impl AnalysisOverrides {
    fn op(print: &[&str]) -> Overrides {
        Overrides { analysis: Some("op".into()), print: print.iter().map(|s| s.to_string()).collect(), ..Overrides::default() }
    }

    fn transient(stop: f64, print: &[&str]) -> Overrides {
        Overrides { analysis: Some("transient".into()), stop: Some(stop), print: print.iter().map(|s| s.to_string()).collect(), ..Overrides::default() }
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

    fn with_params(overrides: Overrides, params: &[(&str, f64)]) -> Overrides {
        Overrides { params: params.iter().map(|(p, v)| (p.to_string(), *v)).collect(), ..overrides }
    }

    fn small_signal(analysis: &str) -> Overrides {
        Overrides { analysis: Some(analysis.into()), top: Some("Top".into()), backend: Some("interp".into()), ..Overrides::default() }
    }

    /// The RC of `PSS_RC`: period 4 us, RC 1 us, high for the first half.
    fn pss(period: f64, tstab: f64, print_step: Option<f64>) -> Overrides {
        Overrides {
            analysis: Some("pss".into()),
            top: Some("PssRc".into()),
            period: Some(period),
            tstab: Some(tstab),
            print_step,
            // the source has 1 ps edges and no breakpoint: a step cap of 1 ns keeps the orbit true
            max_step: Some(1e-9),
            print: vec!["vout".into()],
            ..Overrides::default()
        }
    }
}

/// The cells, times and columns of a table an analysis returned.
struct TableReading;

impl TableReading {
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

    fn times(table: &Table) -> Vec<f64> {
        table.rows.iter().map(|r| TableReading::real(&r[0])).collect()
    }

    fn column_of(table: &Table, name: &str) -> usize {
        table.columns.iter().position(|c| c == name).unwrap_or_else(|| panic!("no column `{name}` in {:?}", table.columns))
    }
}

#[test]
fn an_operating_point_is_signal_and_value_in_print_order() {
    // CLI-05 AC 1: divider.msl, 10 V over 1 kOhm then 3 kOhm
    let table = AnalysisRun::run(&AnalysisRun::twin("divider.msl"), "interp", AnalysisOverrides::op(&["mid", "vin", "i(v1)"])).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(table.columns, ["signal", "value"]);
    let names: Vec<&Cell> = table.rows.iter().map(|r| &r[0]).collect();
    assert_eq!(names, [&Cell::Text("Divider.mid".into()), &Cell::Text("Divider.vin".into()), &Cell::Text("i(Divider.v1)".into())]);
    assert!((TableReading::real(&table.rows[1][1]) - 10.0).abs() <= 1e-9, "vin is the source's 10 V");
    // the source drives 10 V / 4 kOhm = 2.5 mA; the sign of a branch flow is the solver's
    // convention, which the spec does not fix, so the magnitude is asserted
    assert!((TableReading::real(&table.rows[2][1]).abs() - 2.5e-3).abs() <= 1e-9, "{:?}", table.rows[2][1]);
    // a discrete net is decoded by its type: `flag` holds its initial `false`, the undriven
    // in-port `en` holds its zero bytes (spec edge 5)
    let dir = tempfile::tempdir().expect("tmp");
    let file = dir.path().join("mixed.msl");
    let text = "pub module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\npub module Vsource(p: Electrical, n: Electrical) { dc: f64 = 1.0; }\nanalog for Vsource { v(p, n) +> dc; }\n\nmodule Top(in en: Bit = false) {\n    signal a: Electrical;\n    signal flag: Bit = false;\n    let v1 = Vsource(a, gnd) { dc: 2.0 };\n    let r1 = Resistor(a, gnd) { r: 1k };\n}\n";
    std::fs::write(&file, text).expect("write");
    let overrides = Overrides { top: Some("Top".into()), ..AnalysisOverrides::op(&["flag", "a", "en"]) };
    let table = AnalysisRun::run(&file, "rust", overrides).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(table.rows, [
        vec![Cell::Text("Top.flag".into()), Cell::Bool(false)],
        vec![Cell::Text("Top.a".into()), table.rows[1][1].clone()],
        vec![Cell::Text("Top.en".into()), Cell::Bool(false)],
    ]);
    assert!((TableReading::real(&table.rows[1][1]) - 2.0).abs() <= 1e-9, "a is the source's 2 V");
}

#[test]
fn the_divider_mid_is_7_5_on_both_backends() {
    // CLI-05 AC 7: 10 * 3k / (1k + 3k) = 7.5, within 1e-9 (exact arithmetic)
    for backend in ["interp", "rust"] {
        let table = AnalysisRun::run(&AnalysisRun::twin("divider.msl"), backend, AnalysisOverrides::op(&[])).unwrap_or_else(|f| panic!("{backend}: {f}"));
        let mid = TableReading::real(TableReading::value(&table, "Divider.mid"));
        assert!((mid - 7.5).abs() <= 1e-9, "{backend}: Divider.mid = {mid}");
    }
}

#[test]
fn a_transient_has_one_row_per_accepted_step() {
    // CLI-05 AC 2: tran_rc.msl to 10 us, two printed signals
    let table = AnalysisRun::run(&AnalysisRun::twin("tran_rc.msl"), "interp", AnalysisOverrides::transient(10e-6, &["vin", "out"])).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(table.columns, ["time", "TranRc.vin", "TranRc.out"]);
    assert!(table.rows.iter().all(|r| r.len() == 3), "one cell per column");
    let t = TableReading::times(&table);
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
        let table = AnalysisRun::run(&AnalysisRun::twin("tran_rc.msl"), backend, AnalysisOverrides::transient(10e-6, &["out"])).unwrap_or_else(|f| panic!("{backend}: {f}"));
        let row = table.rows.iter().find(|r| TableReading::real(&r[0]) >= rc).unwrap_or_else(|| panic!("{backend}: no row at or after RC"));
        let (t, out) = (TableReading::real(&row[0]), TableReading::real(&row[1]));
        let expected = 1.0 - (-t / rc).exp();
        assert!((out - expected).abs() <= 1e-3, "{backend}: out({t}) = {out}, expected {expected}");
    }
}

#[test]
fn a_dc_sweep_has_one_row_per_point() {
    // CLI-09 AC 1: the first column is the swept param's full path, one row per `from + k*step`
    // while not past `to`: 0, 2.5, 5, 7.5, 10 on divider.msl; `mid` is 3k / (1k + 3k) of the source
    for backend in ["interp", "rust"] {
        let table = AnalysisRun::run(&AnalysisRun::twin("divider.msl"), backend, AnalysisOverrides::dc("v1.dc", 0.0, 10.0, 2.5, &["mid"])).unwrap_or_else(|f| panic!("{backend}: {f}"));
        assert_eq!(table.columns, ["Divider.v1.dc", "Divider.mid"], "{backend}");
        let swept: Vec<f64> = table.rows.iter().map(|r| TableReading::real(&r[0])).collect();
        assert_eq!(swept, [0.0, 2.5, 5.0, 7.5, 10.0], "{backend}");
        for row in &table.rows {
            let (v, mid) = (TableReading::real(&row[0]), TableReading::real(&row[1]));
            assert!((mid - 0.75 * v).abs() <= 1e-9, "{backend}: mid({v}) = {mid}");
        }
    }
    // a step that does not divide the range stops before `to`: 0, 3, 6, 9
    let table = AnalysisRun::run(&AnalysisRun::twin("divider.msl"), "interp", AnalysisOverrides::dc("v1.dc", 0.0, 10.0, 3.0, &["mid"])).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(table.rows.len(), 4);
    // a downward sweep: 10, 5, 0
    let table = AnalysisRun::run(&AnalysisRun::twin("divider.msl"), "interp", AnalysisOverrides::dc("v1.dc", 10.0, 0.0, -5.0, &["mid"])).unwrap_or_else(|f| panic!("{f}"));
    let swept: Vec<f64> = table.rows.iter().map(|r| TableReading::real(&r[0])).collect();
    assert_eq!(swept, [10.0, 5.0, 0.0]);
}

#[test]
fn the_diode_sweep_has_37_rows() {
    // CLI-09 AC 8: diode_iv.msl sweeps v1.dc from -1 to 0.8 by 0.05: (0.8 + 1) / 0.05 + 1 = 37
    for backend in ["interp", "rust"] {
        let table = AnalysisRun::run(&AnalysisRun::twin("diode_iv.msl"), backend, AnalysisOverrides::dc("v1.dc", -1.0, 0.8, 0.05, &["a"])).unwrap_or_else(|f| panic!("{backend}: {f}"));
        assert_eq!(table.rows.len(), 37, "{backend}");
        assert_eq!(TableReading::real(&table.rows[0][0]), -1.0);
        assert!((TableReading::real(&table.rows[36][0]) - 0.8).abs() <= 1e-9);
    }
}

#[test]
fn a_sweep_of_no_param_is_e0801() {
    // CLI-09 AC 9: a path that names no param is E0801 naming it, at open: no analysis ran
    for sweep in ["v1.nothing", "nobody.dc", "dc"] {
        match AnalysisRun::run(&AnalysisRun::twin("divider.msl"), "interp", AnalysisOverrides::dc(sweep, 0.0, 1.0, 0.5, &[])) {
            Err(Failure::Host { code: "E0801", message }) => assert!(message.contains(sweep), "{message}"),
            other => panic!("`{sweep}`: expected E0801, found {other:?}"),
        }
    }
}

#[test]
fn an_ac_sweep_has_mag_and_phase_in_degrees() {
    // CLI-09 AC 2: `freq`, then `mag(<path>)` and `phase(<path>)` per printed signal; ac_rc.msl
    // from 1e3 to 1e7 at 10 per decade is 41 rows, the first at 1e3 and the last at 1e7; far
    // below the corner the RC passes everything (mag 1, phase near 0), far above it the phase
    // heads for -90 degrees
    for backend in ["interp", "rust"] {
        let table = AnalysisRun::run(&AnalysisRun::twin("ac_rc.msl"), backend, AnalysisOverrides::ac(1e3, 1e7, Some(10), None, &["out", "vin"])).unwrap_or_else(|f| panic!("{backend}: {f}"));
        assert_eq!(table.columns, ["freq", "mag(AcRc.out)", "phase(AcRc.out)", "mag(AcRc.vin)", "phase(AcRc.vin)"], "{backend}");
        assert_eq!(table.rows.len(), 41, "{backend}");
        assert!((TableReading::real(&table.rows[0][0]) - 1e3).abs() <= 1e-6 && (TableReading::real(&table.rows[40][0]) - 1e7).abs() <= 1.0, "{backend}");
        let (mag, phase) = (TableReading::real(&table.rows[0][1]), TableReading::real(&table.rows[0][2]));
        assert!((mag - 1.0).abs() <= 1e-3, "{backend}: |out| at 1 kHz = {mag}");
        // the RC of ac_rc.msl (1 kOhm, 1.5915494309189535 nF): phase -atan(2 pi f R C), in degrees,
        // -0.5729 at 1 kHz; the spec's tolerance, 1e-2 degrees
        let expected = -(2.0 * std::f64::consts::PI * TableReading::real(&table.rows[0][0]) * 1000.0 * 1.5915494309189535e-9).atan().to_degrees();
        assert!((expected + 0.5729).abs() <= 1e-4, "the analytic phase at 1 kHz is {expected}");
        assert!((phase - expected).abs() <= 1e-2, "{backend}: phase {phase} degrees at 1 kHz, expected {expected}");
        assert!(TableReading::real(&table.rows[40][2]) < -85.0, "{backend}: phase at 10 MHz");
        // the source is the 1 V stimulus: magnitude 1, phase 0
        assert!((TableReading::real(&table.rows[10][3]) - 1.0).abs() <= 1e-9 && TableReading::real(&table.rows[10][4]).abs() <= 1e-9, "{backend}");
    }
}

#[test]
fn points_sweeps_linearly() {
    // CLI-09 AC 6: `points = 5` from 1e3 to 5e3 is 1e3, 2e3, 3e3, 4e3, 5e3
    let table = AnalysisRun::run(&AnalysisRun::twin("ac_rc.msl"), "interp", AnalysisOverrides::ac(1e3, 5e3, None, Some(5), &["out"])).unwrap_or_else(|f| panic!("{f}"));
    let freqs: Vec<f64> = table.rows.iter().map(|r| TableReading::real(&r[0])).collect();
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
        let table = AnalysisRun::run(&AnalysisRun::twin("ac_rc.msl"), backend, AnalysisOverrides::ac(1e3, 1e7, Some(10), None, &["out"])).unwrap_or_else(|f| panic!("{backend}: {f}"));
        let row = table.rows.iter().find(|r| (TableReading::real(&r[0]) / 1e5 - 1.0).abs() <= 1e-9).unwrap_or_else(|| panic!("{backend}: no row at 1e5"));
        let (mag, phase) = (TableReading::real(&row[1]), TableReading::real(&row[2]));
        assert!((mag - 0.7071067811865476).abs() <= 1e-3, "{backend}: mag {mag}");
        assert!((phase + 45.0).abs() <= 1e-2, "{backend}: phase {phase}");
    }
}

#[test]
fn params_are_set_in_order_table_then_flags() {
    // CLI-10 AC 1 and 4: divider.msl is 10 V over r1 = 1k then r2 = 3k; r2.r = 1000 gives
    // 10 * 1000 / (1000 + 1000) = 5 for `mid`; the entries apply in order, so a later write of
    // the same param wins: r2.r = 9000 then r2.r = 1000 is 5, and 1000 then 9000 is 10 * 9/10 = 9
    for backend in ["interp", "rust"] {
        let mid = |params: &[(&str, f64)]| {
            let table = AnalysisRun::run(&AnalysisRun::twin("divider.msl"), backend, AnalysisOverrides::with_params(AnalysisOverrides::op(&["mid"]), params)).unwrap_or_else(|f| panic!("{backend}: {f}"));
            TableReading::real(TableReading::value(&table, "Divider.mid"))
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
        match AnalysisRun::run(&AnalysisRun::twin("divider.msl"), "rust", AnalysisOverrides::with_params(AnalysisOverrides::op(&[]), &[(path, 1.0)])) {
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
        match AnalysisRun::run(&AnalysisRun::twin("divider.msl"), backend, AnalysisOverrides::with_params(AnalysisOverrides::op(&[]), &[("r2.r", -5.0)])) {
            Err(Failure::Host { code: "E0802", message }) => assert!(message.contains("r2.r") && message.contains("-5"), "{backend}: {message}"),
            other => panic!("{backend}: expected E0802, found {other:?}"),
        }
    }
}

/// `Top`: 10 V over 1 kOhm then 3 kOhm, built from the shared parts.
const DIVIDER: &str = "pub module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\npub module Vsource(p: Electrical, n: Electrical) { dc: f64 = 1.0; }\nanalog for Vsource { v(p, n) +> dc; }\nmodule Top() {\n    signal vin: Electrical;\n    signal mid: Electrical;\n    let v1 = Vsource(vin, gnd) { dc: 10.0 };\n    let r1 = Resistor(vin, mid) { r: 1k };\n    let r2 = Resistor(mid, gnd) { r: 3k };\n}\n";

#[test]
fn tf_input_must_be_a_voltage_source() {
    // HII-04 AC 4: a resistor has no potential contribution: P0010 naming the instance and saying
    // that the input must be a voltage source
    let tf = |input: &str| Overrides { input: Some(input.into()), output: Some("mid".into()), ..AnalysisOverrides::small_signal("tf") };
    let Err(Failure::Host { code, message }) = AnalysisRun::run_text(DIVIDER, tf("r1")) else { panic!("a resistor input is a P0010") };
    assert_eq!(code, "P0010");
    assert!(message.contains("Top.r1") && message.contains("voltage source"), "{message}");

    // and a voltage source is accepted by its entry or by its design path alike: 10 V over 1 kOhm
    // then 3 kOhm gives gain 3/4, input resistance 4 kOhm, output resistance 750 Ohm
    for input in ["v1", "Top.v1"] {
        let table = AnalysisRun::run_text(DIVIDER, tf(input)).unwrap_or_else(|f| panic!("{input}: {f}"));
        for (quantity, wanted) in [("gain", 0.75), ("input_impedance", 4000.0), ("output_impedance", 750.0)] {
            let got = TableReading::real(TableReading::value(&table, quantity));
            assert!((got - wanted).abs() <= 1e-9 * wanted, "{input}: {quantity} = {got}, wanted {wanted}");
        }
    }
}

#[test]
fn tf_output_naming_nothing_is_e0801() {
    // HII-04 AC 5, and an input that names no instance, are E0801 naming the entry
    let tf = |input: &str, output: &str| Overrides { input: Some(input.into()), output: Some(output.into()), ..AnalysisOverrides::small_signal("tf") };
    for (input, output, named) in [("v1", "nowhere", "nowhere"), ("ghost", "mid", "ghost")] {
        let Err(Failure::Host { code, message }) = AnalysisRun::run_text(DIVIDER, tf(input, output)) else { panic!("{input}/{output} is refused") };
        assert_eq!(code, "E0801");
        assert!(message.contains(named), "{message}");
    }
}

#[test]
fn pz_of_a_circuit_without_poles_fails() {
    // HII-05 AC 3: a resistive divider has no finite pole: the solver's error, no table
    let pz = Overrides { input: Some("v1".into()), output: Some("mid".into()), ..AnalysisOverrides::small_signal("pz") };
    let Err(failure) = AnalysisRun::run_text(DIVIDER, pz) else { panic!("a circuit with no poles gives no table") };
    assert!(matches!(&failure, Failure::Solver { analysis: "pz", .. }), "{failure:?}");
    assert!(AnalysisRun::message(&failure).contains("no finite poles"), "{failure}");
}

#[test]
fn sens_wrt_naming_nothing_is_e0801() {
    // HII-06 AC 4: a `wrt` entry that names no param is E0801 naming it
    for entry in ["r1.nope", "ghost.r"] {
        let sens = Overrides { wrt: vec![entry.into()], print: vec!["mid".into()], ..AnalysisOverrides::small_signal("sens") };
        let Err(Failure::Host { code, message }) = AnalysisRun::run_text(DIVIDER, sens) else { panic!("{entry} is refused") };
        assert_eq!(code, "E0801");
        assert!(message.contains(entry), "{message}");
    }
}

#[test]
fn sens_of_a_discrete_net_is_refused() {
    // HII-06 AC 5: a printed discrete net has no sensitivity: P0010 naming it
    let sens = Overrides { wrt: vec!["r1.r".into()], print: vec!["mid".into(), "flag".into()], ..AnalysisOverrides::small_signal("sens") };
    let with_flag = DIVIDER.replace("    let v1", "    signal flag: Bit = false;\n    let v1");
    let Err(Failure::Host { code, message }) = AnalysisRun::run_text(&with_flag, sens) else { panic!("a discrete net is refused") };
    assert_eq!(code, "P0010");
    assert!(message.contains("Top.flag"), "{message}");
}

#[test]
fn sens_wrt_a_derived_param_is_refused() {
    // HII-18's edge: `Mid.rr` is the value its child's `r` derives from (the probe of GAPS S2), so
    // a finite difference of `rr` alone would skip the child: P0010 naming both
    let text = "pub module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\npub module Vsource(p: Electrical, n: Electrical) { dc: f64 = 1.0; }\nanalog for Vsource { v(p, n) +> dc; }\nmodule Mid(a: Electrical, b: Electrical) {\n    rr: f64 = 1k;\n    let r = Resistor { r: rr }(a, b);\n}\nmodule Top() {\n    signal a: Electrical;\n    signal b: Electrical;\n    let v1 = Vsource(a, gnd) { dc: 1.0 };\n    let m = Mid(a, b);\n    let r2 = Resistor(b, gnd) { r: 3k };\n}\n";
    let sens = Overrides { wrt: vec!["m.rr".into()], print: vec!["b".into()], ..AnalysisOverrides::small_signal("sens") };
    let Err(Failure::Host { code, message }) = AnalysisRun::run_text(text, sens) else { panic!("a param a child derives from is refused") };
    assert_eq!(code, "P0010");
    assert!(message.contains("Top.m.rr") && message.contains("r.r"), "{message}");
    // the child's own param is not derived from anything: it is accepted
    let own = Overrides { wrt: vec!["r2.r".into()], print: vec!["b".into()], ..AnalysisOverrides::small_signal("sens") };
    let table = AnalysisRun::run_text(text, own).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(table.columns, ["param", "d(Top.b)"]);
}

/// `Top` of `PssRc`: the pulse source of the `pss_rc` twin (period 4 us, high for 2 us, 1 ps
/// edges, reading `now()` and its params) over 1 kOhm and 1 nF, written here from its parts.
const PSS_RC: &str = "pub module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\npub module Capacitor(p: Electrical, n: Electrical) { c: f64 where c >= 0.0; }\nanalog for Capacitor { i(p, n) +> ddt(c * v(p, n)); }\nmodule Pulse(p: Electrical) { amp: f64 = 1.0; width: f64 = 2u; period: f64 = 4u; edge: f64 = 1p; }\nanalog for Pulse {\n    let ph = now() - period * (now() / period).floor();\n    let level = if ph < edge { ph / edge } else if ph < width { 1.0 } else if ph < width + edge { 1.0 - (ph - width) / edge } else { 0.0 };\n    v(p) +> amp * level;\n}\nmodule PssRc() {\n    signal vin: Electrical;\n    signal vout: Electrical;\n    let src = Pulse(vin);\n    let r1 = Resistor { r: 1k }(vin, vout);\n    let c1 = Capacitor { c: 1n }(vout, gnd);\n}\n";

#[test]
fn pss_gives_one_period_of_rows() {
    // HII-07 AC 1: `time` first, one column per printed signal, rows from t0 to t0 + T
    let table = AnalysisRun::pss_run(AnalysisOverrides::pss(4e-6, 8e-6, None)).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(table.columns[0], "time");
    assert_eq!(table.columns.len(), 2, "{:?}", table.columns);
    let t = TableReading::times(&table);
    assert!(t.len() > 4, "one row per accepted step: {}", t.len());
    assert!((t[0] - 8e-6).abs() < 1e-12, "starts at t0: {}", t[0]);
    assert!((t[t.len() - 1] - 12e-6).abs() < 1e-12, "ends at t0 + T: {}", t[t.len() - 1]);
    assert!(t.windows(2).all(|w| w[0] < w[1]), "time increases");
    // the orbit closes: the last row of the period is the first
    let v = TableReading::column_of(&table, &table.columns[1].clone());
    let (first, last) = (TableReading::real(&table.rows[0][v]), TableReading::real(&table.rows[table.rows.len() - 1][v]));
    assert!((first - last).abs() < 1e-3, "x(t0) = {first}, x(t0 + T) = {last}");
}

#[test]
fn pss_on_a_grid_gives_the_grid_rows() {
    // HII-07 AC 2: one row per t0 + k*h up to t0 + T (101 rows for h = T / 100)
    let table = AnalysisRun::pss_run(AnalysisOverrides::pss(4e-6, 8e-6, Some(4e-8))).unwrap_or_else(|f| panic!("{f}"));
    let t = TableReading::times(&table);
    assert_eq!(t.len(), 101, "{} rows", t.len());
    for (k, time) in t.iter().enumerate() {
        let want = 8e-6 + k as f64 * 4e-8;
        assert!((time - want).abs() < 1e-12, "row {k}: {time} against {want}");
    }
}

#[test]
fn pss_starts_its_orbit_at_tstab() {
    // HII-07 AC 3: `t0` is `tstab`, whatever it is: the first row is at `tstab`, the last at
    // `tstab + T`, and the orbit does not depend on it (the end of the high half, at 10 us, is
    // 1 / (1 + exp(-2)) of the 1 V drive, reached from a start at 8 us or at 9 us)
    let closed = 1.0 / (1.0 + (-2.0_f64).exp());
    for tstab in [8e-6, 9e-6] {
        let table = AnalysisRun::pss_run(AnalysisOverrides::pss(4e-6, tstab, Some(1e-7))).unwrap_or_else(|f| panic!("tstab {tstab}: {f}"));
        let t = TableReading::times(&table);
        assert!((t[0] - tstab).abs() < 1e-12 && (t[t.len() - 1] - (tstab + 4e-6)).abs() < 1e-12, "tstab {tstab}: {} to {}", t[0], t[t.len() - 1]);
        let row = table.rows.iter().find(|r| (TableReading::real(&r[0]) - 10e-6).abs() < 1e-12).unwrap_or_else(|| panic!("tstab {tstab}: no row at 10 us"));
        assert!((TableReading::real(&row[1]) - closed).abs() < 1e-3, "tstab {tstab}: {} against {closed}", TableReading::real(&row[1]));
    }
}

#[test]
fn pss_that_does_not_converge_fails() {
    // HII-07 AC 6: a period the drive does not have (4 us drive, period 3 us) has no orbit: the
    // solver's own error names the residual, and there is no table
    let Err(failure) = AnalysisRun::pss_run(AnalysisOverrides::pss(3e-6, 0.0, None)) else { panic!("a period the drive does not have gives no orbit") };
    assert!(matches!(&failure, Failure::Solver { analysis: "pss", .. }), "{failure:?}");
    let text = format!("{failure}");
    // the shot over 3 us of a 4 us drive lands a full volt off the next one: the residual named is 1
    assert!(text.contains("does not repeat: |x(2T) \u{2212} x(T)| = 1.000e0"), "{text}");
}

#[test]
fn a_pss_orbit_that_does_not_close_at_the_step_says_so() {
    // HII-07 AC 6 (F8): the drive is periodic, but with no step cap a step misplaces its 1 ps
    // edges and the orbit does not repeat; the error says the step is a possible cause instead
    // of blaming the drive alone, and there is no table
    let Err(failure) = AnalysisRun::pss_run(Overrides { max_step: None, ..AnalysisOverrides::pss(4e-6, 8e-6, None) }) else { panic!("an uncapped step misplaces the edges of the twin's source") };
    let text = format!("{failure}");
    assert!(text.contains("does not repeat") && text.contains("misplaces") && text.contains("maximum step"), "{text}");
}

#[test]
fn a_pss_plot_draws_its_rows() {
    // HII-07 AC 7: the plot of a pss run is drawn from its table, as a transient's
    let (_dir, file) = AnalysisRun::pss_file();
    let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
    let overrides = Overrides { backend: Some("rust".into()), plot: Some("p.svg".into()), ..AnalysisOverrides::pss(4e-6, 8e-6, Some(4e-8)) };
    let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
    let mut session = Session::open(&project, &spec, &CompileCache::shared().settings()).unwrap_or_else(|f| panic!("{f}"));
    let table = session.run(Rc::new(RefCell::new(Ignore))).unwrap_or_else(|f| panic!("{f}"));
    let plot = session.plot().unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(table.rows.len(), 101);
    let points: usize = plot.panels.iter().flat_map(|p| &p.series).map(|s| s.runs.iter().map(Vec::len).sum::<usize>()).sum();
    assert_eq!(points, 101, "the panel draws the 101 rows of the table");
    assert_eq!(plot.panels[0].x.title, "time");
}

#[test]
fn pss_takes_the_sessions_step_budget() {
    // HII-02 AC 9 reaches every transient of the session, the shots of a pss among them: a
    // budget of 100 attempts cannot cover a period of 4000 steps of 1 ns
    let (period, budget) = (4e-6, 100);
    let Err(failure) = AnalysisRun::pss_run(Overrides { max_steps: Some(budget), ..AnalysisOverrides::pss(period, 8e-6, None) }) else { panic!("{budget} attempts cannot cover a period") };
    assert!(matches!(&failure, Failure::Solver { analysis: "pss", .. }), "{failure:?}");
}

#[test]
fn a_where_holds_at_equality_and_reads_the_other_params() {
    // Part III 8.1 and §0.3: `x >= 0.0` takes 0; `v_release < v_pick` is checked against the
    // pick's value now. Each entry of a table is one host `set`, applied in order, and §8.1.23
    // re-checks every dependent `where` at each: a table that passes through an invalid state is
    // refused even when its end state is valid
    // `fixtures/bounded_params.msl`: a `Thresh` with a param bounded below by a constant
    // (`x >= 0.0`) and one bounded by another param (`v_release < v_pick`); the CLI tests share it
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bounded_params.msl");
    let overrides = |params: &[(&str, f64)]| AnalysisOverrides::with_params(Overrides { top: Some("Top".into()), ..AnalysisOverrides::op(&["a"]) }, params);
    for backend in ["interp", "rust"] {
        for accepted in [&[("Top.t.x", 0.0)][..], &[("Top.t.v_release", 1.5)], &[("Top.t.v_release", 1.2)], &[("Top.t.v_release", 0.5), ("Top.t.v_pick", 1.0)]] {
            AnalysisRun::run(&file, backend, overrides(accepted)).unwrap_or_else(|f| panic!("{backend} {accepted:?}: {f}"));
        }
        for refused in [&[("Top.t.x", -1.0)][..], &[("Top.t.v_release", 2.0)], &[("Top.t.v_release", 2.5)], &[("Top.t.v_pick", 1.0)], &[("Top.t.v_pick", 1.0), ("Top.t.v_release", 0.5)], &[("Top.t.v_pick", 1.0), ("Top.t.v_release", 1.5)]] {
            match AnalysisRun::run(&file, backend, overrides(refused)) {
                Err(Failure::Host { code: "E0802", .. }) => {}
                other => panic!("{backend} {refused:?}: expected E0802, found {other:?}"),
            }
        }
    }
}
