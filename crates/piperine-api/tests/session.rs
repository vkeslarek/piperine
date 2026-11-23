//! `Session::open` over MSL on disk (CLI-04 AC 1 to 3, 5, 6 and 9; spec edges 5 and 6): the
//! choice of the top, the backend's refusals, one compile per design, and a top whose ports are
//! discrete or continuous. Every Rust-backend session builds into a cache directory of its own
//! test, passed through `Settings`; no test sets a variable or writes the user's cache.
#![cfg(unix)]

use std::cell::RefCell;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Mutex;

use piperine_api::{Built, Failure, Overrides, ParamValue, Project, Record, RecordSink, RunSpec, Session};
use piperine_solver::prelude::{Context, TransientAnalysisOptions};
use piperine_transpile::Settings;

/// Writing an executable and exec'ing it races with another thread's `fork` holding the write
/// descriptor (ETXTBSY); the tests that build take turns.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// The devices of `examples/rc/src/lib.msl`, in one file.
const DEVICES: &str = "pub module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\n\npub module Vsource(p: Electrical, n: Electrical) { dc: f64 = 1.0; }\nanalog for Vsource { v(p, n) +> dc; }\n";

/// `module <name>()` holding a source across a resistor.
fn portless(name: &str) -> String {
    format!("module {name}() {{\n    signal a: Electrical;\n    let v1 = Vsource(a, gnd) {{ dc: 1.0 }};\n    let r1 = Resistor(a, gnd) {{ r: 1k }};\n}}\n")
}

fn examples_rc() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/rc")
}

/// A lone file holding `text`, in its own temporary directory.
fn lone(text: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tmp");
    let file = dir.path().join("bench.msl");
    fs::write(&file, text).expect("write");
    (dir, file)
}

fn flags(analysis: &str, backend: &str, top: Option<&str>) -> Overrides {
    Overrides { analysis: Some(analysis.into()), backend: Some(backend.into()), top: top.map(str::to_string), ..Overrides::default() }
}

/// The interpreter needs no settings; these are never read by it.
fn unused() -> Settings {
    Settings { rustc: None, cache: PathBuf::from("/nonexistent/cache") }
}

/// A session of the lone file `text` with `overrides`.
fn open(text: &str, overrides: &Overrides, settings: &Settings) -> Result<Session, Failure> {
    let (_dir, file) = lone(text);
    let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
    let spec = project.request(None, overrides).unwrap_or_else(|f| panic!("{f}"));
    Session::open(&project, &spec, settings)
}

fn p0011(result: Result<Session, Failure>) -> String {
    match result {
        Err(Failure::Host { code: "P0011", message }) => message,
        Err(other) => panic!("P0011, found {other}"),
        Ok(s) => panic!("P0011, found a session with top {}", s.top()),
    }
}

/// A compiler wrapper in a directory of its own: logs each call's arguments, then runs the
/// real `rustc`.
struct Counting {
    _dir: tempfile::TempDir,
    log: PathBuf,
    settings: Settings,
}

impl Counting {
    fn new() -> Counting {
        let dir = tempfile::tempdir().expect("a temp dir");
        let program = dir.path().join("rustc");
        let log = dir.path().join("rustc.log");
        let text = format!("#!/bin/sh\nREAL=$(command -v rustc)\necho \"$@\" >> {}\nexec \"$REAL\" \"$@\"\n", log.display());
        fs::write(&program, text).expect("write the wrapper");
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("chmod");
        let settings = Settings { rustc: Some(program), cache: dir.path().join("cache") };
        Counting { _dir: dir, log, settings }
    }

    /// The builds of a design's library so far (`--crate-name design`); the ABI library and
    /// the `-vV` probes are not counted.
    fn design_builds(&self) -> usize {
        fs::read_to_string(&self.log).unwrap_or_default().lines().filter(|l| l.contains("--crate-name design")).count()
    }
}

#[test]
fn the_top_key_names_the_module() {
    // CLI-04 AC 1: two modules without ports; `top` picks one
    let text = format!("{DEVICES}\n{}\n{}", portless("A"), portless("B"));
    let session = open(&text, &flags("op", "interp", Some("B")), &unused()).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(session.top(), "B");
    assert_eq!(session.ir().top().path(), "B", "the elaborated top is B");
}

#[test]
fn the_one_portless_module_is_the_top() {
    // CLI-04 AC 2: `Resistor` and `Vsource` have ports; `Only` is the one without
    let text = format!("{DEVICES}\n{}", portless("Only"));
    let session = open(&text, &flags("op", "interp", None), &unused()).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(session.top(), "Only");
}

#[test]
fn no_top_or_several_is_p0011_with_candidates() {
    // CLI-04 AC 3
    let none = p0011(open(DEVICES, &flags("op", "interp", None), &unused()));
    assert!(none.contains("no module without ports"), "{none}");
    let text = format!("{DEVICES}\n{}\n{}", portless("A"), portless("B"));
    let several = p0011(open(&text, &flags("op", "interp", None), &unused()));
    assert!(several.contains("A, B"), "both candidates named: {several}");
}

#[test]
fn a_missing_rustc_names_rustc_and_the_interpreter() {
    // CLI-04 AC 5
    let cache = tempfile::tempdir().expect("tmp");
    let settings = Settings { rustc: Some(PathBuf::from("/nonexistent/bin/rustc")), cache: cache.path().to_path_buf() };
    let text = format!("{DEVICES}\n{}", portless("Top"));
    match open(&text, &flags("op", "rust", None), &settings) {
        Err(Failure::Backend(message)) => assert!(message.contains("rustc") && message.contains("interp"), "{message}"),
        Err(other) => panic!("a backend failure, found {other}"),
        Ok(_) => panic!("no rustc, no session"),
    }
}

#[test]
fn the_interpreter_refusal_names_the_body_and_the_rust_backend() {
    // CLI-04 AC 6: `examples/rc`'s `Monitor` holds a reaction, which the interpreter does not
    // run; its refusal names the body and the backend that runs it
    let project = Project::at(&examples_rc()).unwrap_or_else(|f| panic!("{f}"));
    let spec = project.request(Some("step"), &Overrides { backend: Some("interp".into()), ..Overrides::default() }).unwrap_or_else(|f| panic!("{f}"));
    match Session::open(&project, &spec, &unused()) {
        Err(Failure::Backend(message)) => assert!(message.contains("Monitor") && message.contains("--backend rust"), "{message}"),
        Err(other) => panic!("a backend failure, found {other}"),
        Ok(_) => panic!("the interpreter refuses a reaction"),
    }
}

#[test]
fn a_run_compiles_once_and_never_during_an_analysis() {
    // CLI-04 AC 9, under a compiler wrapper that counts
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let compiler = Counting::new();
    let project = Project::at(&examples_rc()).unwrap_or_else(|f| panic!("{f}"));
    let spec: RunSpec = project.request(Some("step"), &Overrides::default()).unwrap_or_else(|f| panic!("{f}"));
    let mut session = Session::open(&project, &spec, &compiler.settings).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(compiler.design_builds(), 1, "one build of the design");
    assert!(matches!(session.built(), Built::Compiled { .. }), "{:?}", session.built());
    // a transient of the bound circuit: 5 us, the `step` run's stop
    let circuit = session.circuit().expect("a circuit before any run");
    let result = circuit.transient(TransientAnalysisOptions::new(5e-6, 5e-9), Context::default()).expect("tran").solve().unwrap_or_else(|e| panic!("{e}"));
    assert!(result.len() > 1, "the transient ran");
    assert_eq!(compiler.design_builds(), 1, "no analysis ran the compiler");
    // a second session on the same cache builds nothing
    let again = Session::open(&project, &spec, &compiler.settings).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(compiler.design_builds(), 1, "the second session found the design in the cache");
    assert!(matches!(again.built(), Built::Cached { .. }), "{:?}", again.built());
}

#[test]
fn a_top_with_discrete_in_ports_starts_them_at_their_initial_values() {
    // spec edge 5: the port `en` is a net of the top that nothing writes after its initial value
    // `true` (contract "The top")
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let cache = tempfile::tempdir().expect("tmp");
    let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
    let text = format!(
        "{DEVICES}\nmodule Top(in en: Bit = true) {{\n    signal a: Electrical;\n    let v1 = Vsource(a, gnd) {{ dc: 1.0 }};\n    let r1 = Resistor(a, gnd) {{ r: 1k }};\n}}\n"
    );
    let mut session = open(&text, &flags("op", "rust", Some("Top")), &settings).unwrap_or_else(|f| panic!("{f}"));
    assert!(session.ir().net_at("Top.en").is_some(), "the port is a net of the top");
    let circuit = session.circuit().expect("a circuit");
    let result = circuit.transient(TransientAnalysisOptions::new(1e-9, 1e-10), Context::default()).expect("tran").solve().unwrap_or_else(|e| panic!("{e}"));
    // the first step is the operating point; a `Bit` is laid out as its value byte (abi.md P3):
    // nothing wrote `en` after its initial value, so it holds `true`
    let first = result.get(0).expect("the operating point");
    let en = first.digital_named("Top.en").expect("the net `Top.en` in the result");
    assert_eq!(en.get(0), Some(&1), "`en` holds its initial value: {en:?}");
}

#[test]
fn an_undriven_top_port_without_default_refuses_the_analysis() {
    // EU-04 (§3.3.4): an `in` port with no initial value, no `Default` and no value from the host
    // when the analysis starts refuses it with `E0187` naming the port; a drive at 0 gives it its
    // first value and the analysis runs
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let cache = tempfile::tempdir().expect("tmp");
    let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
    let text = format!(
        "{DEVICES}\nmodule Top(in en: Bit) {{\n    signal a: Electrical;\n    let v1 = Vsource(a, gnd) {{ dc: 1.0 }};\n    let r1 = Resistor(a, gnd) {{ r: 1k }};\n}}\n"
    );
    let mut refused = open(&text, &flags("op", "rust", Some("Top")), &settings).unwrap_or_else(|f| panic!("{f}"));
    match refused.run(std::rc::Rc::new(std::cell::RefCell::new(NoRecords))) {
        Err(Failure::Host { code: "E0187", message }) => assert!(message.contains("`en`"), "the port is named: {message}"),
        other => panic!("E0187 naming `en`, found {other:?}"),
    }
    let mut driven = open(&text, &flags("op", "rust", Some("Top")), &settings).unwrap_or_else(|f| panic!("{f}"));
    driven.drive("en", ParamValue::Bool(true), 0.0).expect("a drive at 0");
    driven.run(std::rc::Rc::new(std::cell::RefCell::new(NoRecords))).unwrap_or_else(|f| panic!("{f}"));
}

/// A sink that keeps no record.
struct NoRecords;

impl RecordSink for NoRecords {
    fn record(&mut self, _record: &Record) {}
}

#[test]
fn a_top_with_a_node_port_is_refused_by_the_interpreter_naming_the_host_source() {
    // PI-07 (D8, R2): a node port of the top elaborates (the IR gains the host source), and the
    // frozen interpreter, which does not run the clock the source reads, refuses it by name
    let text = format!("{DEVICES}\nmodule Top(a: Electrical) {{\n    let r1 = Resistor(a, gnd) {{ r: 1k }};\n}}\n");
    match open(&text, &flags("op", "interp", Some("Top")), &unused()) {
        Err(failure) => assert!(failure.to_string().contains("HostSource"), "{failure}"),
        Ok(_) => panic!("the interpreter ran a host source"),
    }
}

#[test]
fn a_module_typed_param_is_not_set_by_text() {
    // PI-04, design D13: a param of module type is bound in the source, so a request param that
    // names it is E0802 at elaboration; the design itself opens
    let devices = format!("{DEVICES}\nmodule Cell(p: Electrical) {{ r: f64 = 1k; }}\nanalog for Cell {{ i(p, gnd) +> v(p, gnd) / r; }}\n");
    let top = "module Top() {\n    cell: Cell = Cell { r: 2k };\n    signal a: Electrical;\n    let v1 = Vsource(a, gnd) { dc: 1.0 };\n    let c1 = cell(a);\n}\n";
    let text = format!("{devices}\n{top}");
    // the interpreter runs `f64` and `bool` params only (PI-05), so "the design opens" is its description: elaborated, no backend
    let (_dir, file) = lone(&text);
    let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
    let spec = project.request(None, &flags("op", "interp", Some("Top"))).unwrap_or_else(|f| panic!("{f}"));
    Session::describe(&project, &spec).unwrap_or_else(|f| panic!("the design opens: {f}"));
    let overrides = Overrides { params: vec![("Top.cell".to_string(), 1.0)], ..flags("op", "interp", Some("Top")) };
    match open(&text, &overrides, &unused()) {
        Err(Failure::Diagnostics { found, .. }) => {
            assert_eq!(found.iter().map(|d| d.diagnostic.code.to_string()).collect::<Vec<_>>(), ["E0802"], "{found:?}");
            assert!(found[0].diagnostic.message.contains("Top.cell"), "{}", found[0].diagnostic.message);
        }
        Err(other) => panic!("E0802 from elaboration, found {other}"),
        Ok(_) => panic!("a text value was given to a module-typed param"),
    }
}

/// A sink that keeps nothing: the statistics test reads the table.
struct Ignore;

impl RecordSink for Ignore {
    fn record(&mut self, _: &Record) {}
}

#[test]
fn a_session_returns_the_statistics_of_its_last_transient() {
    // TP-01 AC 4: no print grid, so the table has one row per accepted step after the operating
    // point's; before any run the session has no statistics
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/e2e/twins/tran_rc.msl");
    let project = Project::file(&path).unwrap_or_else(|f| panic!("{f}"));
    let overrides = Overrides {
        analysis: Some("transient".into()),
        backend: Some("interp".into()),
        stop: Some(10e-6),
        print: vec!["out".into()],
        ..Overrides::default()
    };
    let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
    let mut session = Session::open(&project, &spec, &unused()).unwrap_or_else(|f| panic!("{f}"));
    assert!(session.statistics().is_none(), "no statistics before a run");

    let table = session.run(Rc::new(RefCell::new(Ignore))).unwrap_or_else(|f| panic!("{f}"));

    let statistics = session.statistics().expect("statistics after a transient");
    assert!(statistics.steps().accepted() > 10);
    assert_eq!(statistics.steps().accepted() as usize, table.rows.len() - 1, "one row per accepted step after the first");
    assert_eq!(statistics.steps().accepted() as usize, statistics.analog().steps_accepted);
}

/// A leaf with an integer, an enum and an `Option` param, on a source across a resistor, as
/// PI-01 to PI-03 write them.
const TYPED_PARAMS: &str = "#[derive(Repr, PartialEq)]\nenum Mode { Fast, Slow }\n#[derive(Repr, PartialEq)]\nenum Rating { Conductance(f64), Resistance(f64) }\n\
    pub module Vsource(p: Electrical, n: Electrical) { dc: f64 = 1.0; }\nanalog for Vsource { v(p, n) +> dc; }\n\
    module Leaf(p: Electrical) { n: u8 = 4; md: Mode = Mode::Fast; rs: Option<f64>; rating: Rating = Rating::Conductance(1.0); }\n\
    analog for Leaf { i(p, gnd) +> (n as f64) * v(p) / 1k + match md { Mode::Fast => 1.0, Mode::Slow => 3.0 } * v(p) / rs.unwrap_or(1k); }\n\
    module Top() { signal a: Electrical; let v1 = Vsource(a, gnd) { dc: 1.0 }; let l = Leaf(a); }\n";

/// A Rust-backend session of `TYPED_PARAMS` in a cache of its own.
fn typed_session() -> (Session, tempfile::TempDir) {
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let cache = tempfile::tempdir().expect("a cache");
    let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
    let session = open(TYPED_PARAMS, &flags("op", "rust", Some("Top")), &settings).unwrap_or_else(|f| panic!("{f}"));
    (session, cache)
}

/// The value the session shows for `Top.l.<param>`.
fn shown(session: &Session, param: &str) -> piperine_api::Cell {
    session.metadata().params.into_iter().find(|p| p.path == format!("Top.l.{param}")).unwrap_or_else(|| panic!("no param `{param}`")).value
}

/// The language's literal of the composite value `cell` shows (`Some(2500)`, `None`).
fn literal(cell: piperine_api::Cell) -> String {
    match cell {
        piperine_api::Cell::Composite(value) => value.literal(&|leaf| match leaf {
            piperine_api::Cell::Real(v) => v.to_string(),
            other => format!("{other:?}"),
        }),
        other => panic!("a composite value, found {other:?}"),
    }
}

#[test]
fn an_integer_param_set_by_text_in_range() {
    // PI-01 AC 4 (D24): a decimal, `0x`, `0b` literal with `_` in the range of `u8` is the value.
    let (mut session, _cache) = typed_session();
    for (text, wanted) in [("6", 6), ("0x1F", 31), ("0b101", 5), ("2_0", 20), ("255", 255)] {
        session.set_text("Top.l.n", text).unwrap_or_else(|f| panic!("`{text}`: {f}"));
        assert_eq!(shown(&session, "n"), piperine_api::Cell::Int(wanted), "`{text}`");
    }
}

#[test]
fn an_integer_out_of_range_is_e0802() {
    // PI-01 AC 4: 256 does not fit `u8`, `-1` is no `u8`, and `1.5` is no integer literal: each is E0802 naming
    // the param and the value, and the param keeps its value.
    let (mut session, _cache) = typed_session();
    for text in ["256", "-1", "1.5", "six"] {
        match session.set_text("Top.l.n", text) {
            Err(Failure::Host { code: "E0802", message }) => assert!(message.contains("Top.l.n") && message.contains(text), "{message}"),
            other => panic!("`{text}`: E0802, found {other:?}"),
        }
    }
    assert_eq!(shown(&session, "n"), piperine_api::Cell::Int(4), "a refused set changes nothing");
}

#[test]
fn an_enum_param_set_by_variant_name() {
    // PI-02 AC 3: `Slow` is the variant, bare or as `Mode::Slow`.
    let (mut session, _cache) = typed_session();
    assert_eq!(shown(&session, "md"), piperine_api::Cell::Text("Fast".into()), "the default");
    session.set_text("Top.l.md", "Slow").unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(shown(&session, "md"), piperine_api::Cell::Text("Slow".into()));
    session.set_text("Top.l.md", "Mode::Fast").unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(shown(&session, "md"), piperine_api::Cell::Text("Fast".into()));
}

#[test]
fn an_unknown_variant_is_e0802_listing_the_variants() {
    // PI-02 AC 4: a name `Mode` does not declare is E0802 naming the param and listing the variants.
    let (mut session, _cache) = typed_session();
    match session.set_text("Top.l.md", "Medium") {
        Err(Failure::Host { code: "E0802", message }) => {
            assert!(message.contains("Top.l.md") && message.contains("`Medium`"), "{message}");
            assert!(message.contains("`Fast`") && message.contains("`Slow`"), "the variants are listed: {message}");
        }
        other => panic!("E0802, found {other:?}"),
    }
}

#[test]
fn an_option_param_set_to_a_value_or_none() {
    // PI-03 AC 4: a value of `f64` is `Some` of it, bare or as `Some(..)`, and `None` is `None`.
    let (mut session, _cache) = typed_session();
    assert_eq!(literal(shown(&session, "rs")), "None", "left out: `None`");
    session.set_text("Top.l.rs", "2500").unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(literal(shown(&session, "rs")), "Some(2500)");
    session.set_text("Top.l.rs", "None").unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(literal(shown(&session, "rs")), "None");
    session.set_text("Top.l.rs", "Some(10)").unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(literal(shown(&session, "rs")), "Some(10)");
    assert!(matches!(session.set_text("Top.l.rs", "lots"), Err(Failure::Host { code: "E0802", .. })));
}

#[test]
fn a_payload_enum_param_is_set_by_its_leaves_not_by_text() {
    // PI-02, §8.1.15: an enum with fields is a composite param, shown as its variant and set by leaves like any composite (D13):
    // a text for the whole param is E0802 naming the param and pointing at its leaves.
    let (mut session, _cache) = typed_session();
    assert!(matches!(shown(&session, "rating"), piperine_api::Cell::Composite(_)), "{:?}", shown(&session, "rating"));
    match session.set_text("Top.l.rating", "Resistance") {
        Err(Failure::Host { code: "E0802", message }) => assert!(message.contains("Top.l.rating") && message.contains("leaves"), "{message}"),
        other => panic!("E0802, found {other:?}"),
    }
}
