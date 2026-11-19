//! Scope: the session options of a run (host-ii HII-01 AC 6, HII-02): the keys `reltol`,
//! `abstol`, `vntol`, `trtol`, `temp`, `resolution`, `maxrounds`, `maxmicro` and `nodeset` reach
//! the context and the binding of every session, and a run that gives none keeps the solver's
//! defaults. Each fixture is MSL written by the test; expected values come from the circuit's
//! closed form or from the option's own definition. Rust-backend sessions build into a cache
//! directory of this target.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::OnceLock;

use piperine_api::{Cell, Failure, Overrides, Project, Record, RecordSink, Session, Table};
use piperine_solver::prelude::Context;
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

/// A source across a resistor, the modules the fixtures share.
const PARTS: &str = "pub module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\npub module Vsource(p: Electrical, n: Electrical) { dc: f64 = 1.0; }\nanalog for Vsource { v(p, n) +> dc; }\n";

/// The session of `text` (a lone file, top `Top`) with `overrides`.
fn open(text: &str, overrides: Overrides) -> Result<Session, Failure> {
    let dir = tempfile::tempdir().expect("tmp");
    let file = dir.path().join("bench.msl");
    std::fs::write(&file, text).expect("write");
    open_file(&file, overrides)
}

fn open_file(path: &Path, overrides: Overrides) -> Result<Session, Failure> {
    let project = Project::file(path).unwrap_or_else(|f| panic!("{f}"));
    let overrides = Overrides { top: Some("Top".into()), ..overrides };
    let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
    Session::open(&project, &spec, &settings())
}

/// The table of one run of `text`, and the records it delivered.
fn run(text: &str, overrides: Overrides) -> Result<(Table, Vec<Record>), Failure> {
    let mut session = open(text, overrides)?;
    let kept = Rc::new(RefCell::new(Kept::default()));
    let table = session.run(kept.clone())?;
    let records = kept.borrow().0.clone();
    Ok((table, records))
}

fn op(print: &[&str]) -> Overrides {
    Overrides { analysis: Some("op".into()), print: print.iter().map(|s| s.to_string()).collect(), backend: Some("interp".into()), ..Overrides::default() }
}

/// The value of the row named `signal` of an operating point.
fn value(table: &Table, signal: &str) -> f64 {
    let row = table.rows.iter().find(|r| r[0] == Cell::Text(signal.into())).unwrap_or_else(|| panic!("no row `{signal}` in {table:?}"));
    match &row[1] {
        Cell::Real(v) => *v,
        other => panic!("a real, found {other:?}"),
    }
}

/// `Top`: a source of `temperature()` volts over a resistor.
const THERMOMETER: &str = "pub module Thermometer(p: Electrical, n: Electrical) {}\nanalog for Thermometer { v(p, n) +> temperature(); }\nmodule Top() {\n    signal a: Electrical;\n    let t = Thermometer(a, gnd);\n    let r = Resistor(a, gnd) { r: 1k };\n}\n";

/// `Top`: one node and a cubic conductance with the three roots 1, 2 and 3, so the operating
/// point it converges to depends on where Newton starts.
const CUBIC: &str = "pub module Cubic(p: Electrical, n: Electrical) {}\nanalog for Cubic { i(p, n) +> (v(p, n) - 1.0) * (v(p, n) - 2.0) * (v(p, n) - 3.0); }\nmodule Top() {\n    signal a: Electrical;\n    let c = Cubic(a, gnd);\n}\n";

fn with_parts(rest: &str) -> String {
    format!("{PARTS}{rest}")
}

#[test]
fn no_option_gives_the_default_context() {
    // HII-02 AC 1: a run with no option key builds the solver's own default context
    let session = open(&with_parts(THERMOMETER), op(&["a"])).unwrap_or_else(|f| panic!("{f}"));
    let (got, want) = (session.context(), Context::default());
    assert_eq!(got.tolerances.reltol, want.tolerances.reltol);
    assert_eq!(got.tolerances.abstol, want.tolerances.abstol);
    assert_eq!(got.tolerances.vntol, want.tolerances.vntol);
    assert_eq!(got.tolerances.trtol, want.tolerances.trtol);
    assert_eq!(got.tolerances.temperature, want.tolerances.temperature);
    assert_eq!(got.max_rounds, 20);
    assert_eq!(got.max_rounds, want.max_rounds);
}

#[test]
fn tolerance_keys_reach_the_context() {
    // HII-02 AC 2: each given tolerance replaces the default in the context, the others stay
    let overrides = Overrides { reltol: Some(1e-5), abstol: Some(1e-14), vntol: Some(2e-7), trtol: Some(3.0), ..op(&["a"]) };
    let session = open(&with_parts(THERMOMETER), overrides).unwrap_or_else(|f| panic!("{f}"));
    let got = &session.context().tolerances;
    assert_eq!((got.reltol, got.abstol, got.vntol, got.trtol), (1e-5, 1e-14, 2e-7, 3.0));
    let only = Overrides { reltol: Some(1e-5), ..op(&["a"]) };
    let session = open(&with_parts(THERMOMETER), only).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(session.context().tolerances.abstol, Context::default().tolerances.abstol, "an option not given keeps its default");
}

#[test]
fn temp_is_celsius_read_by_temperature() {
    // HII-02 AC 3: `temp = 75` makes `temperature()` read 75 + 273.15 kelvin; a design whose source
    // is `temperature()` volts shows it
    let hot = value(&run(&with_parts(THERMOMETER), Overrides { temp: Some(75.0), ..op(&["a"]) }).unwrap_or_else(|f| panic!("{f}")).0, "Top.a");
    let default = Context::default().tolerances.temperature;
    let cold = value(&run(&with_parts(THERMOMETER), op(&["a"])).unwrap_or_else(|f| panic!("{f}")).0, "Top.a");
    // The operating point stops Newton when the update is within `reltol * |x| + vntol` (the
    // solver's convergence criterion, `Tolerances::has_converged`), so a reading is exact only
    // to that bound: with `--reltol 1e-9 --vntol 1e-12` the same run reads 300.15 exactly. Each
    // reading is held to that bound at its own value, and the two to the sum of their bounds.
    let t = Context::default().tolerances;
    let bound = |x: f64| t.reltol * x.abs() + t.vntol;
    assert!((hot - 348.15).abs() <= bound(348.15), "temp = 75 reads 348.15 K: {hot}");
    assert!((cold - default).abs() <= bound(default), "no `temp` leaves the default {default} K: {cold}");
    assert!((hot - cold - (348.15 - default)).abs() <= bound(348.15) + bound(default), "the two runs differ by the 75 C step: {hot} against {cold}");
    // tightened, the same run converges to the exact value: the bound above is the criterion, not a defect
    let tight = Overrides { temp: Some(75.0), reltol: Some(1e-9), vntol: Some(1e-12), ..op(&["a"]) };
    let exact = value(&run(&with_parts(THERMOMETER), tight).unwrap_or_else(|f| panic!("{f}")).0, "Top.a");
    assert!((exact - 348.15).abs() < 1e-6, "{exact}");
}

#[test]
fn a_nodeset_seeds_the_operating_point_and_keeps_its_solution() {
    // HII-02 AC 4: the cubic's operating point is one of its roots 1, 2, 3; a seed near 3 starts
    // Newton there and converges to 3, a seed near 1 to 1, and each answer is still a root of the
    // circuit's own equation (the seed moved the start, not the equation)
    let at = |seed: f64| {
        let overrides = Overrides { nodeset: vec![("a".into(), seed)], ..op(&["a"]) };
        value(&run(&with_parts(CUBIC), overrides).unwrap_or_else(|f| panic!("{f}")).0, "Top.a")
    };
    let high = at(3.4);
    assert!((high - 3.0).abs() < 1e-6, "seeded at 3.4: {high}");
    let low = at(0.6);
    assert!((low - 1.0).abs() < 1e-6, "seeded at 0.6: {low}");
    for root in [high, low] {
        assert!(((root - 1.0) * (root - 2.0) * (root - 3.0)).abs() < 1e-5, "{root} is a root of the circuit");
    }
    // the full path names the same node as the relative one
    let full = Overrides { nodeset: vec![("Top.a".into(), 3.4)], ..op(&["a"]) };
    assert!((value(&run(&with_parts(CUBIC), full).unwrap_or_else(|f| panic!("{f}")).0, "Top.a") - 3.0).abs() < 1e-6);
}

#[test]
fn a_nodeset_naming_nothing_is_e0801() {
    // HII-01 AC 6: an entry that names no node is `E0801` naming it, before any analysis
    let overrides = Overrides { nodeset: vec![("nowhere".into(), 1.0)], ..op(&["a"]) };
    let error = open(&with_parts(CUBIC), overrides).err().expect("the entry names nothing");
    assert!(matches!(&error, Failure::Host { code: "E0801", message } if message.contains("nowhere")), "{error:?}");
}

fn rust(overrides: Overrides) -> Overrides {
    Overrides { backend: Some("rust".into()), ..overrides }
}

/// `Top`: the analog part, and the placements of a digital module `m`.
fn mixed(modules: &str, placements: &str) -> String {
    format!("{PARTS}\n{modules}\nmodule Top() {{\n    signal a: Electrical;\n    let v1 = Vsource(a, gnd) {{ dc: 1.0 }};\n    let r1 = Resistor(a, gnd) {{ r: 1k }};\n{placements}}}\n")
}

#[test]
fn resolution_rounds_an_at_to_its_tick() {
    // HII-02 AC 5: with `resolution = 1u` a timer written at 1.4 us fires on the tick, 1 us; with
    // the default tick it fires at 1.4 us
    let m = "module M() {}\ndigital for M { on at(1.4u) { emit!(\"s\", k = 1); } }\n";
    let design = mixed(m, "    let m = M();\n");
    let time_of = |overrides: Overrides| {
        let (_, records) = run(&design, Overrides { top: Some("Top".into()), max_step: Some(1e-7), ..rust(overrides) }).unwrap_or_else(|f| panic!("{f}"));
        assert_eq!(records.len(), 1, "{records:?}");
        records[0].time
    };
    let stop = Overrides { analysis: Some("transient".into()), stop: Some(3e-6), ..Overrides::default() };
    let rounded = time_of(Overrides { resolution: Some(1e-6), ..stop.clone() });
    assert!((rounded - 1e-6).abs() < 1e-12, "{rounded}");
    let exact = time_of(stop);
    assert!((exact - 1.4e-6).abs() < 1e-12, "{exact}");
}

/// `Top`: a driver raises `d` at time 0 and a converter turns it into a voltage the operating
/// point reads, so the point needs a second round after the first instant.
const TWO_ROUNDS: &str = "module Drv(out d: Bit = false) {}\ndigital for Drv { on at(0.0) { d <- true; } }\nmodule Top() {\n    signal d: Bit;\n    signal a: Electrical;\n    signal vdd: Electrical;\n    let v = Vsource(vdd, gnd) { dc: 1.0 };\n    let drv = Drv(d);\n    let conv = D2A(d, a, vdd);\n}\n";

#[test]
fn maxrounds_bounds_the_operating_point() {
    // HII-02 AC 6: with the default bound the point settles with the converter at 1 V; with
    // `maxrounds = 1` the second round it needs is past the bound and the run fails with E0506
    // naming 1
    let design = with_parts(TWO_ROUNDS);
    let (table, _) = run(&design, rust(op(&["a"]))).unwrap_or_else(|f| panic!("{f}"));
    assert!((value(&table, "Top.a") - 1.0).abs() < 2e-3, "{table:?}");
    let error = run(&design, rust(Overrides { maxrounds: Some(1), ..op(&["a"]) })).err().expect("one round is not enough");
    let text = error.to_string();
    assert!(text.contains("E0506"), "{text}");
    assert!(text.contains("1 rounds"), "{text}");
}

/// `Top`: a net that toggles itself with no delay once armed, so an instant never goes quiet.
const TOGGLER: &str = "module Flip(out q: Bit = false) {}\ndigital for Flip {\n    on at(0.0) || q.changed() { q <- !q; }\n}\n";

#[test]
fn maxmicro_bounds_an_instant() {
    // HII-02 AC 7: with `maxmicro = 5` the zero-delay toggle fails naming 5; with no key it
    // fails naming the engine's 1000
    let design = mixed(TOGGLER, "    signal q: Bit;\n    let f = Flip(q);\n");
    let bounded = run(&design, rust(Overrides { maxmicro: Some(5), ..op(&["a"]) })).err().expect("5 microsteps are not enough");
    let text = bounded.to_string();
    assert!(text.contains("E0501") && text.contains("5 microsteps"), "{text}");
    let default = run(&design, rust(op(&["a"]))).err().expect("an instant that never goes quiet fails");
    assert!(default.to_string().contains("1000 microsteps"), "{default}");
}

#[test]
fn a_smaller_trtol_takes_more_steps() {
    // HII-02 AC 8 (amended): `trtol` reaches the solver's step assessment; the RC charging curve of
    // `tran_rc` to 10 us with `trtol = 0.001` rejects steps the default (7) accepts, and the
    // retries make the accepted grid finer: more accepted steps than with no `trtol`
    let twin = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/e2e/twins/tran_rc.msl");
    let steps = |trtol: Option<f64>| {
        let overrides = Overrides { analysis: Some("transient".into()), stop: Some(10e-6), trtol, backend: Some("interp".into()), top: Some("TranRc".into()), ..Overrides::default() };
        let project = Project::file(&twin).unwrap_or_else(|f| panic!("{f}"));
        let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
        let mut session = Session::open(&project, &spec, &settings()).unwrap_or_else(|f| panic!("{f}"));
        session.run(Rc::new(RefCell::new(Kept::default()))).unwrap_or_else(|f| panic!("{f}")).rows.len()
    };
    let (default, tight) = (steps(None), steps(Some(0.001)));
    assert!(tight > default, "trtol = 0.001 took {tight} steps, the default {default}");
}

/// The transient of `tran_rc` to 10 us at `trtol = 0.001` (3241 steps, `a_smaller_trtol_takes_more_steps`)
/// with the budget `max_steps`.
fn tran_rc_with_budget(max_steps: Option<u32>) -> Result<Table, Failure> {
    let twin = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/e2e/twins/tran_rc.msl");
    let overrides = Overrides { analysis: Some("transient".into()), stop: Some(10e-6), trtol: Some(0.001), max_steps, backend: Some("interp".into()), top: Some("TranRc".into()), ..Overrides::default() };
    let project = Project::file(&twin).unwrap_or_else(|f| panic!("{f}"));
    let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
    Session::open(&project, &spec, &settings())?.run(Rc::new(RefCell::new(Kept::default())))
}

#[test]
fn a_run_past_max_steps_fails_and_succeeds_when_raised() {
    // HII-02 AC 9: a transient that needs about 3300 attempts fails at a budget of 1000 with the
    // solver's own message naming it, and succeeds with the budget raised to 100000
    let error = tran_rc_with_budget(Some(1000)).err().expect("1000 attempts are not enough");
    assert!(error.to_string().contains("exceeded the 1000 step budget"), "{error}");
    let table = tran_rc_with_budget(Some(100_000)).unwrap_or_else(|f| panic!("{f}"));
    assert!(table.rows.len() > 1000, "{} rows", table.rows.len());
}

#[test]
fn max_steps_defaults_to_two_million() {
    // HII-02 AC 9: with no `max_steps` the run gives the solver no budget of its own, and the
    // solver's default is 2,000,000 (the value before this feature)
    let session = open(&with_parts(THERMOMETER), op(&["a"])).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(session.spec().options.max_steps, None);
    assert_eq!(piperine_solver::prelude::TransientAnalysisOptions::new(1.0, 1e-3).max_steps, 2_000_000);
}
