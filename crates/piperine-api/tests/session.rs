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
    Session::describe(&project, &spec, &|| Ok(Settings { rustc: None, cache: PathBuf::from("/nonexistent/cache") })).unwrap_or_else(|f| panic!("the design opens: {f}"));
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
    module Leaf(p: Electrical) { n: u8 = 4; md: Mode = Mode::Fast; rs: Option<f64>; rating: Rating = Rating::Conductance(1.0); big: u64 = 0; }\n\
    analog for Leaf { i(p, gnd) +> (n as f64) * v(p) / 1k + match md { Mode::Fast => 1.0, Mode::Slow => 3.0 } * v(p) / rs.unwrap_or(1k); }\n\
    module Top() { signal a: Electrical; let v1 = Vsource(a, gnd) { dc: 1.0 }; let l = Leaf(a); }\n";

/// The session of `TYPED_PARAMS` and what it shows: the owner of the helpers the typed-param tests share.
struct Typed;

impl Typed {
    /// A Rust-backend session of `TYPED_PARAMS` in a cache of its own, with the `--param` texts `texts` given before the run.
    fn session_with(texts: &[(&str, &str)]) -> Result<(Session, tempfile::TempDir), Failure> {
        let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let cache = tempfile::tempdir().expect("a cache");
        let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
        let param_texts = texts.iter().map(|(entry, text)| (entry.to_string(), text.to_string())).collect();
        let overrides = Overrides { param_texts, ..flags("op", "rust", Some("Top")) };
        open(TYPED_PARAMS, &overrides, &settings).map(|session| (session, cache))
    }

    /// A session with no `--param`.
    fn session() -> (Session, tempfile::TempDir) {
        Typed::session_with(&[]).unwrap_or_else(|f| panic!("{f}"))
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
}

#[test]
fn an_integer_param_set_by_text_in_range() {
    // PI-01 AC 4 (D24): a decimal, `0x`, `0b` literal with `_` in the range of `u8` is the value.
    let (mut session, _cache) = Typed::session();
    for (text, wanted) in [("6", 6), ("0x1F", 31), ("0b101", 5), ("2_0", 20), ("255", 255)] {
        session.set_text("Top.l.n", text).unwrap_or_else(|f| panic!("`{text}`: {f}"));
        assert_eq!(Typed::shown(&session, "n"), piperine_api::Cell::Int(wanted), "`{text}`");
    }
}

#[test]
fn an_integer_out_of_range_is_e0802() {
    // PI-01 AC 4: 256 does not fit `u8`, `-1` is no `u8`, and `1.5` is no integer literal: each is E0802 naming
    // the param and the value, and the param keeps its value.
    let (mut session, _cache) = Typed::session();
    for text in ["256", "-1", "1.5", "six"] {
        match session.set_text("Top.l.n", text) {
            Err(Failure::Host { code: "E0802", message }) => assert!(message.contains("Top.l.n") && message.contains(text), "{message}"),
            other => panic!("`{text}`: E0802, found {other:?}"),
        }
    }
    assert_eq!(Typed::shown(&session, "n"), piperine_api::Cell::Int(4), "a refused set changes nothing");
}

#[test]
fn an_enum_param_set_by_variant_name() {
    // PI-02 AC 3: `Slow` is the variant, bare or as `Mode::Slow`.
    let (mut session, _cache) = Typed::session();
    assert_eq!(Typed::shown(&session, "md"), piperine_api::Cell::Text("Fast".into()), "the default");
    session.set_text("Top.l.md", "Slow").unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(Typed::shown(&session, "md"), piperine_api::Cell::Text("Slow".into()));
    session.set_text("Top.l.md", "Mode::Fast").unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(Typed::shown(&session, "md"), piperine_api::Cell::Text("Fast".into()));
}

#[test]
fn an_unknown_variant_is_e0802_listing_the_variants() {
    // PI-02 AC 4: a name `Mode` does not declare is E0802 naming the param and listing the variants.
    let (mut session, _cache) = Typed::session();
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
    let (mut session, _cache) = Typed::session();
    assert_eq!(Typed::literal(Typed::shown(&session, "rs")), "None", "left out: `None`");
    session.set_text("Top.l.rs", "2500").unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(Typed::literal(Typed::shown(&session, "rs")), "Some(2500)");
    session.set_text("Top.l.rs", "None").unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(Typed::literal(Typed::shown(&session, "rs")), "None");
    session.set_text("Top.l.rs", "Some(10)").unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(Typed::literal(Typed::shown(&session, "rs")), "Some(10)");
    assert!(matches!(session.set_text("Top.l.rs", "lots"), Err(Failure::Host { code: "E0802", .. })));
}

#[test]
fn a_payload_enum_param_is_set_by_its_leaves_not_by_text() {
    // PI-02, §8.1.15: an enum with fields is a composite param, shown as its variant and set by leaves like any composite (D13):
    // a text for the whole param is E0802 naming the param and pointing at its leaves.
    let (mut session, _cache) = Typed::session();
    assert!(matches!(Typed::shown(&session, "rating"), piperine_api::Cell::Composite(_)), "{:?}", Typed::shown(&session, "rating"));
    match session.set_text("Top.l.rating", "Resistance") {
        Err(Failure::Host { code: "E0802", message }) => assert!(message.contains("Top.l.rating") && message.contains("leaves"), "{message}"),
        other => panic!("E0802, found {other:?}"),
    }
}

#[test]
fn a_param_flag_is_read_by_the_params_type_before_the_run() {
    // PI-01 AC 4, PI-02 AC 4, PI-03 AC 4, D24: a `--param` is read by the type of its param whatever it looks like: a number for an
    // integer param is an integer literal of the type and range (a point, an exponent, a negative for an unsigned or a value past the
    // range is `E0802`), a unit enum takes a variant name only (a number is `E0802`), and a value of `T` for an `Option<T>` is `Some`.
    for (entry, text) in [("Top.l.n", "6.5"), ("Top.l.n", "1e1"), ("Top.l.n", "-1"), ("Top.l.n", "256"), ("Top.l.md", "1"), ("Top.l.md", "Medium")] {
        match Typed::session_with(&[(entry, text)]) {
            Err(Failure::Host { code: "E0802", message }) => assert!(message.contains(entry) && message.contains(text), "{entry}={text}: {message}"),
            Err(other) => panic!("{entry}={text}: E0802, found {other}"),
            Ok(_) => panic!("{entry}={text}: accepted"),
        }
    }
    let (session, _cache) = Typed::session_with(&[("Top.l.n", "0x6"), ("Top.l.md", "Slow"), ("Top.l.rs", "10")]).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(Typed::shown(&session, "n"), piperine_api::Cell::Int(6));
    assert_eq!(Typed::shown(&session, "md"), piperine_api::Cell::Text("Slow".into()));
    assert_eq!(Typed::literal(Typed::shown(&session, "rs")), "Some(10)", "a number for an `Option<f64>` param is `Some` of it");
}

#[test]
fn an_integer_above_2_pow_53_is_set_exactly() {
    // PI-01 AC 4, D24: a literal of a `u64` is the value: 2^53 + 1 is not rounded to 2^53, and the highest `u64` is accepted.
    for (text, wanted) in [("9007199254740993", 9_007_199_254_740_993_i128), ("18446744073709551615", i128::from(u64::MAX))] {
        let (session, _cache) = Typed::session_with(&[("Top.l.big", text)]).unwrap_or_else(|f| panic!("{text}: {f}"));
        assert_eq!(Typed::shown(&session, "big"), piperine_api::Cell::Int(wanted), "{text}");
    }
    // the description of the design (`Session::describe`, no backend) shows the same exact value
    let (_dir, file) = lone(TYPED_PARAMS);
    let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
    let overrides = Overrides { param_texts: vec![("Top.l.big".to_string(), "9007199254740993".to_string())], ..flags("op", "interp", Some("Top")) };
    let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
    let (_, metadata) = Session::describe(&project, &spec, &|| Ok(Settings { rustc: None, cache: PathBuf::from("/nonexistent/cache") })).unwrap_or_else(|f| panic!("{f}"));
    let shown = metadata.params.into_iter().find(|p| p.path == "Top.l.big").expect("the param").value;
    assert_eq!(shown, piperine_api::Cell::Int(9_007_199_254_740_993));
}

/// A design whose branches are selected by structural params: a switch that forces its node to 0.25 V unless it is `Open`, and a
/// source that forces its node to its value when it has one; both nodes hang on a 1 V supply behind 1 kOhm.
const STRUCTURAL: &str = "#[derive(Repr, PartialEq)]\nenum Mode { Short, Open }\n\
    module Supply(o: Electrical) { signal s: Electrical; let v1 = Level(s); let r1 = Resistor(s, o) { r: 1k }; }\n\
    module Level(p: Electrical) { volts: f64 = 1.0; }\nanalog for Level { v(p, gnd) +> volts; }\n\
    module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\n\
    module Sw(a: Electrical) { md: Mode = Mode::Short; }\nanalog for Sw { match md { Mode::Short => { v(a, gnd) +> 0.25; } Mode::Open => {} } }\n\
    module Ow(c: Electrical) { vs: Option<f64>; }\nanalog for Ow { match vs { Some(x) => { v(c, gnd) +> x; } None => {} } }\n\
    module SwTop() { signal a: Electrical; let sup = Supply(a); let m = Sw(a); }\n\
    module OwTop() { signal c: Electrical; let sup = Supply(c); let o = Ow(c); }\n\
    module SwWrap(a: Electrical) { md: Mode = Mode::Short; let s = Sw { md: md }(a); }\n\
    module SwWrapTop() { signal a: Electrical; let sup = Supply(a); let w = SwWrap(a); }\n";

/// The runs of `STRUCTURAL` with `--param` texts for structural params.
struct Structural;

impl Structural {
    /// The voltage of `node` at the operating point of `top` with the `--param` texts `texts`; the failure when it does not open.
    fn volts(top: &str, node: &str, texts: &[(&str, &str)]) -> Result<f64, Failure> {
        let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let cache = tempfile::tempdir().expect("a cache");
        let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
        let param_texts = texts.iter().map(|(entry, text)| (entry.to_string(), text.to_string())).collect();
        let overrides = Overrides { param_texts, print: vec![node.to_string()], ..flags("op", "rust", Some(top)) };
        let mut session = open(STRUCTURAL, &overrides, &settings)?;
        let table = session.run(Rc::new(RefCell::new(Quiet)))?;
        match &table.rows[0][1] {
            piperine_api::Cell::Real(value) => Ok(*value),
            other => panic!("a real, found {other:?}"),
        }
    }
}

/// A sink that keeps nothing: the table is read.
struct Quiet;

impl RecordSink for Quiet {
    fn record(&mut self, _: &Record) {}
}

#[test]
fn a_structural_params_text_selects_its_variant() {
    // PI-02 AC 3, PI-03 AC 2 and AC 4, D24: the text of a `--param` of a structural param reaches elaboration as that value, read by the
    // param's type: the variant `Open` of an enum, `Some(0.5)`, a plain `0.5` (`Some` of it) and `None` of an `Option<f64>`.
    let near = |wanted: f64, got: f64| assert!((got - wanted).abs() < 1e-9, "wanted {wanted}, got {got}");
    near(0.25, Structural::volts("SwTop", "a", &[]).unwrap_or_else(|f| panic!("{f}")));
    near(1.0, Structural::volts("SwTop", "a", &[("m.md", "Open")]).unwrap_or_else(|f| panic!("{f}")));
    near(0.25, Structural::volts("SwTop", "a", &[("m.md", "Short")]).unwrap_or_else(|f| panic!("{f}")));
    near(1.0, Structural::volts("OwTop", "c", &[]).unwrap_or_else(|f| panic!("{f}")));
    near(0.5, Structural::volts("OwTop", "c", &[("o.vs", "Some(0.5)")]).unwrap_or_else(|f| panic!("{f}")));
    near(0.5, Structural::volts("OwTop", "c", &[("o.vs", "0.5")]).unwrap_or_else(|f| panic!("{f}")));
    near(1.0, Structural::volts("OwTop", "c", &[("o.vs", "None")]).unwrap_or_else(|f| panic!("{f}")));
}

#[test]
fn a_structural_params_bad_text_is_e0802() {
    // D24: a text that is no value of the structural param's type is `E0802` at elaboration, never ignored: a variant the enum lacks, a
    // number for an enum, text for an `Option<f64>`.
    for (entry, text) in [("m.md", "Medium"), ("m.md", "1"), ("o.vs", "lots")] {
        let top = if entry.starts_with("m.") { "SwTop" } else { "OwTop" };
        match Structural::volts(top, "a", &[(entry, text)]) {
            Err(Failure::Diagnostics { found, .. }) => assert!(found.iter().any(|d| d.diagnostic.code == "E0802" && d.diagnostic.message.contains(text)), "{entry}={text}: {found:?}"),
            other => panic!("{entry}={text}: E0802 at elaboration, found {other:?}"),
        }
    }
}

#[test]
fn a_leaf_text_is_read_by_the_leafs_type() {
    // composite-values D13 with D24: a leaf of a composite param takes the literal syntax of its type: a `bool` leaf `true` or `false`
    // (`1` is E0802), an integer leaf an integer literal (`0x3`), a real leaf a number.
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let source = "struct Cfg { en: bool, n: u8, g: f64 }\nmodule Top() { cfg: Cfg = Cfg { en: false, n: 1, g: 1.0 }; signal a: Electrical; }\nanalog for Top { v(a, gnd) +> 0.0; }\n";
    let cache = tempfile::tempdir().expect("a cache");
    let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
    let with = |entry: &str, text: &str| {
        let overrides = Overrides { param_texts: vec![(entry.to_string(), text.to_string())], ..flags("op", "rust", Some("Top")) };
        open(source, &overrides, &settings)
    };
    with("Top.cfg.en", "true").unwrap_or_else(|f| panic!("{f}"));
    with("Top.cfg.n", "0x3").unwrap_or_else(|f| panic!("{f}"));
    with("Top.cfg.g", "2.5").unwrap_or_else(|f| panic!("{f}"));
    for (entry, text) in [("Top.cfg.en", "1"), ("Top.cfg.n", "1.5"), ("Top.cfg.n", "true"), ("Top.cfg.g", "yes")] {
        assert!(matches!(with(entry, text), Err(Failure::Host { code: "E0802", .. })), "{entry}={text}");
    }
}

/// Params passed through a wrapper's placement (`{ n: n }`): a design whose inner module reads an integer, an enum with no derive and an
/// `Option`, and whose wrapper hands its own params down.
const WRAPPED: &str = "enum Mode { Fast, Slow }\n\
    module Vsource(p: Electrical) { dc: f64 = 1.0; }\nanalog for Vsource { v(p, gnd) +> dc; }\n\
    module Inner(p: Electrical) { n: u8 = 1; md: Mode = Mode::Fast; rs: Option<f64>; big: u64 = 0; md2: Mode = Mode::Fast; rs2: Option<f64>; md3: Mode = Mode::Fast; rs3: Option<f64>; rs4: Option<f64>; r: f64 = 0.0; }\n\
    analog for Inner { i(p, gnd) +> (n as f64) * v(p) / 1k + match md { Mode::Fast => 1.0, Mode::Slow => 3.0 } * v(p) / 1k + v(p) / rs.unwrap_or(1k) + (if big > 9007199254740992 { 1.0 } else { 0.0 }) * v(p) / 1k + match md2 { Mode::Fast => 0.0, Mode::Slow => 1.0 } * v(p) / 1k; }\n\
    module Wrap(p: Electrical) { n: u8 = 4; md: Mode = Mode::Fast; rs: Option<f64>; big: u64 = 0; slow: bool = false; rt: f64 = 3000.0; k: u8 = 0; a: f64 = 1.0; b: f64 = 2.0; \
      let i = Inner { rs3: rt * 2.0, rs4: a + b, r: match md { Mode::Fast => 1000.0, Mode::Slow => 3000.0 }, rs2: rt, md3: match k { 0 => Mode::Fast, _ => Mode::Slow }, n: n, md: md, rs: rs, big: big, md2: if slow { Mode::Slow } else { Mode::Fast } }(p); }\n\
    module Top() { signal a: Electrical; let v1 = Vsource(a); let w = Wrap { rs: 2k }(a); }\n";

const PASSED: &str = "enum Mode { Fast, Slow }\n#[derive(Repr, PartialEq)]\nstruct Cfg { g: f64, n: u8 }\n\
    module Inner(p: Electrical) { n: u8 = 1; md: Mode = Mode::Fast; rs: Option<f64>; cfg: Cfg = Cfg { g: 1.0, n: 1 }; r: f64 = 1.0; big: u64 = 0; }\n\
    analog for Inner { i(p, gnd) +> (n as f64) * v(p) / 1k + match md { Mode::Fast => 1.0, Mode::Slow => 3.0 } * v(p) / rs.unwrap_or(1k) + cfg.g * v(p) / 1k + r * v(p) / 1k; }\n\
    module Wrap(p: Electrical) { n: u8 = 4; md: Mode = Mode::Fast; rs: Option<f64>; cfg: Cfg = Cfg { g: 1.0, n: 1 }; rt: f64 = 3000.0; big: u64 = 0; \
      let i = Inner { n: n, md: md, rs: rs, cfg: cfg, r: rt * 2.0, big: big }(p); }\n\
    module Top() { signal a: Electrical; let w = Wrap(a); }\n";

#[test]
fn a_description_shows_the_values_a_run_reads_for_every_param_kind() {
    // F38, HII-10: `Session::describe` and `Session::open` apply a host value through the one write path (`Pins`), so the child of every
    // param a placement passes down reads the same in the description as in the run: an integer, an enum, an `Option`, a leaf of a
    // composite, a scalar a placement scales, and an integer above 2^53
    let (_dir, file) = lone(PASSED);
    let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
    let cache = tempfile::tempdir().expect("a cache");
    let settings = || Settings { rustc: None, cache: cache.path().to_path_buf() };
    let texts = [("Top.w.n", "6"), ("Top.w.md", "Slow"), ("Top.w.rs", "3000"), ("Top.w.cfg.g", "2.5"), ("Top.w.rt", "2500"), ("Top.w.big", "9007199254740993")];
    let overrides = Overrides { param_texts: texts.iter().map(|(entry, text)| (entry.to_string(), text.to_string())).collect(), ..flags("op", "rust", Some("Top")) };
    let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
    let (_, described) = Session::describe(&project, &spec, &|| Ok(settings())).unwrap_or_else(|f| panic!("{f}"));
    let session = Session::open(&project, &spec, &settings()).unwrap_or_else(|f| panic!("{f}"));
    let run = session.metadata();
    let value = |metadata: &piperine_api::Metadata, path: &str| metadata.params.iter().find(|p| p.path == path).map(|p| p.value.clone()).unwrap_or_else(|| panic!("no `{path}`"));
    for param in ["n", "md", "rs", "cfg", "r", "big"] {
        let path = format!("Top.w.i.{param}");
        assert_eq!(value(&described, &path), value(&run, &path), "{path}: the description is what the run reads");
    }
    // and they are the values written, not the defaults
    assert_eq!(value(&described, "Top.w.i.n"), piperine_api::Cell::Int(6));
    assert_eq!(value(&described, "Top.w.i.md"), piperine_api::Cell::Text("Slow".into()));
    assert_eq!(Typed::literal(value(&described, "Top.w.i.rs")), "Some(3000)");
    assert_eq!(value(&described, "Top.w.i.r"), piperine_api::Cell::Real(5000.0));
    assert_eq!(value(&described, "Top.w.i.big"), piperine_api::Cell::Int(9_007_199_254_740_993));
    let cfg = |metadata: &piperine_api::Metadata| format!("{:?}", value(metadata, "Top.w.i.cfg"));
    assert!(cfg(&described).contains("2.5"), "a leaf of a composite reaches the child: {}", cfg(&described));
}

#[test]
fn a_described_option_and_leaf_reach_the_child() {
    // F38: `Top.w.rs=3000` shows `Some(3000)` in the child, not `None`; `Top.w.cfg.g=2.5` shows 2.5, not 1.0 (numeric `--param` form too)
    let (_dir, file) = lone(PASSED);
    let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
    let cache = tempfile::tempdir().expect("a cache");
    let overrides = Overrides { params: vec![("Top.w.cfg.g".into(), 2.5)], param_texts: vec![("Top.w.rs".to_string(), "3000".to_string())], ..flags("op", "rust", Some("Top")) };
    let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
    let (_, metadata) = Session::describe(&project, &spec, &|| Ok(Settings { rustc: None, cache: cache.path().to_path_buf() })).unwrap_or_else(|f| panic!("{f}"));
    let value = |path: &str| metadata.params.iter().find(|p| p.path == path).map(|p| p.value.clone()).unwrap_or_else(|| panic!("no `{path}`"));
    assert_eq!(Typed::literal(value("Top.w.i.rs")), "Some(3000)");
    assert!(format!("{:?}", value("Top.w.i.cfg")).contains("2.5"), "{:?}", value("Top.w.i.cfg"));
}

#[test]
fn describing_a_set_the_interpreter_cannot_derive_names_the_interpreter_not_the_plan() {
    // F34: `Session::describe` with a host value for `Top.w.n`, which a placement passes to an integer param of the child, cannot be
    // evaluated by the interpreter's walk (it holds `f64` and `bool` params only, PI-05): the refusal says so and names `--backend rust`
    let (_dir, file) = lone(WRAPPED);
    let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
    let overrides = Overrides { param_texts: vec![("Top.w.n".to_string(), "6".to_string())], ..flags("op", "interp", Some("Top")) };
    let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
    match Session::describe(&project, &spec, &|| Ok(Settings { rustc: None, cache: PathBuf::from("/nonexistent/cache") })) {
        Err(Failure::Host { code: "E0802", message }) => assert!(message.contains("--backend rust") && !message.contains("does not match"), "{message}"),
        Err(other) => panic!("E0802, found {other}"),
        Ok(_) => panic!("the interpreter cannot recompute an integer the child holds"),
    }
}

#[test]
fn describing_a_rust_request_shows_the_values_a_run_derives_of_every_param_type() {
    // F36, HII-10: `describe` of a Rust request with `Top.w.n = 6` and `Top.w.md = Slow` evaluates what the placement passes to the child
    // (an integer, an enum) through the compiled derive, so it shows the values a run uses, not a refusal
    let (_dir, file) = lone(WRAPPED);
    let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
    let cache = tempfile::tempdir().expect("a cache");
    let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
    let overrides = Overrides { param_texts: vec![("Top.w.n".to_string(), "6".to_string()), ("Top.w.md".to_string(), "Slow".to_string())], ..flags("op", "rust", Some("Top")) };
    let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
    let (_, metadata) = Session::describe(&project, &spec, &|| Ok(Settings { rustc: settings.rustc.clone(), cache: settings.cache.clone() })).unwrap_or_else(|f| panic!("{f}"));
    let value = |path: &str| metadata.params.iter().find(|p| p.path == path).map(|p| p.value.clone());
    assert_eq!(value("Top.w.i.n"), Some(piperine_api::Cell::Int(6)));
    assert_eq!(value("Top.w.i.md"), Some(piperine_api::Cell::Text("Slow".into())));
}

#[test]
fn params_passed_through_a_wrappers_placement_run() {
    // §3.2 and §8.4: a placement binds the child's param to the parent's own (`{ n: n }`, `{ md: md }`, `{ rs: rs }`, an `Option` to an
    // `Option`): the child reads the parent's value, and a `set` of the parent's integer or enum reaches the child. The `Option`
    // keeps the value it elaborated to (a composite is not recomputed): setting it is E0802 pointing at the child.
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let cache = tempfile::tempdir().expect("a cache");
    let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
    let mut session = open(WRAPPED, &flags("op", "rust", Some("Top")), &settings).unwrap_or_else(|f| panic!("{f}"));
    let shown = |session: &Session, param: &str| session.metadata().params.into_iter().find(|p| p.path == format!("Top.w.i.{param}")).unwrap_or_else(|| panic!("no `{param}`")).value;
    assert_eq!(shown(&session, "n"), piperine_api::Cell::Int(4));
    assert_eq!(shown(&session, "md"), piperine_api::Cell::Text("Fast".into()));
    assert!(matches!(shown(&session, "rs"), piperine_api::Cell::Composite(_)), "the `Option` reached the child: {:?}", shown(&session, "rs"));
    session.set_text("Top.w.n", "6").unwrap_or_else(|f| panic!("{f}"));
    session.set_text("Top.w.md", "Slow").unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(shown(&session, "n"), piperine_api::Cell::Int(6), "the child follows the wrapper's integer");
    assert_eq!(shown(&session, "md"), piperine_api::Cell::Text("Slow".into()), "and its enum");
    // §8.1.23, PI-03 AC 4: a `set` of the wrapper's `Option` recomputes the value bound from it, a wrapper with no pin included
    session.set_text("Top.w.rs", "3000").unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(Typed::literal(shown(&session, "rs")), "Some(3000)", "a plain value is `Some` of it in the child");
    session.set_text("Top.w.rs", "None").unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(Typed::literal(shown(&session, "rs")), "None");
    // a derived enum value (an `if` over a `bool` param) is recomputed as the variant it selects (F29)
    assert_eq!(shown(&session, "md2"), piperine_api::Cell::Text("Fast".into()));
    session.set_text("Top.w.slow", "true").unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(shown(&session, "md2"), piperine_api::Cell::Text("Slow".into()));
    // an `Option` bound to a plain value of the parent's (`{ rs2: rt }`) is `Some` of it, and follows a `set`; a `match` over an integer picks the variant
    assert_eq!(Typed::literal(shown(&session, "rs2")), "Some(3000)");
    session.set_text("Top.w.rt", "2500").unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(Typed::literal(shown(&session, "rs2")), "Some(2500)");
    assert_eq!(shown(&session, "md3"), piperine_api::Cell::Text("Fast".into()));
    session.set_text("Top.w.k", "2").unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(shown(&session, "md3"), piperine_api::Cell::Text("Slow".into()));
    // an integer above 2^53 crosses a derive function exactly (PI-01 AC 4): the child holds the wrapper's value, and the param a placement
    // gives takes no write of its own
    session.set_text("Top.w.big", "9007199254740993").unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(shown(&session, "big"), piperine_api::Cell::Int(9_007_199_254_740_993), "the child reads the exact integer");
    for entry in ["Top.w.i.big"] {
        match session.set_text(entry, "5") {
            Err(Failure::Host { code: "E0802", .. }) => {}
            other => panic!("{entry}: E0802, found {other:?}"),
        }
        match session.set_text(entry, "9007199254740993") {
            Err(Failure::Host { code: "E0802", .. }) => {}
            other => panic!("{entry}: E0802, found {other:?}"),
        }
    }
    // an `Option` bound to an expression over params follows them (§8.1.19, §8.1.23): `rt * 2.0` and `a + b`
    assert_eq!(Typed::literal(shown(&session, "rs3")), "Some(5000)");
    assert_eq!(Typed::literal(shown(&session, "rs4")), "Some(3)");
    session.set_text("Top.w.b", "5").unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(Typed::literal(shown(&session, "rs4")), "Some(6)", "`a + b` over two params");
    // a value computed by `match` over an enum param, before and after a set
    assert_eq!(shown(&session, "r"), piperine_api::Cell::Real(3000.0), "`md` was set to Slow above");
    session.set_text("Top.w.md", "Fast").unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(shown(&session, "r"), piperine_api::Cell::Real(1000.0));
    // a wrapper with no pin: `None` where the child already holds `None` changes nothing and succeeds (F30)
    session.set_text("Top.w.rs", "None").unwrap_or_else(|f| panic!("{f}"));
    session.set_text("Top.w.rs", "None").unwrap_or_else(|f| panic!("{f}"));
}

#[test]
fn a_structural_enum_passed_through_a_wrapper_selects_the_variant() {
    // a structural param handed down (`{ md: md }`): the wrapper's `--param w.md=Open` reaches the inner `Sw` at elaboration, so its
    // node is the supply's 1 V; the default `Short` forces 0.25 V.
    let near = |wanted: f64, got: f64| assert!((got - wanted).abs() < 1e-9, "wanted {wanted}, got {got}");
    near(0.25, Structural::volts("SwWrapTop", "a", &[]).unwrap_or_else(|f| panic!("{f}")));
    near(1.0, Structural::volts("SwWrapTop", "a", &[("w.md", "Open")]).unwrap_or_else(|f| panic!("{f}")));
}

#[test]
fn a_refused_text_is_named_as_the_user_wrote_it() {
    // a write the circuit refuses (the instance runs nothing, so it has no pin for the param) names the param and the text the user
    // gave, `Open` or `Some(0.5)`, never the enum's tag or the `value` leaf of an `Option`.
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let cache = tempfile::tempdir().expect("a cache");
    let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
    let source = "enum Mode { Short, Open }\nmodule Top() { md: Mode = Mode::Short; vs: Option<f64>; signal a: Electrical; }\n";
    for (entry, text) in [("Top.md", "Open"), ("Top.vs", "Some(0.5)")] {
        let overrides = Overrides { param_texts: vec![(entry.to_string(), text.to_string())], ..flags("op", "rust", Some("Top")) };
        match open(source, &overrides, &settings) {
            Err(Failure::Host { code: "E0802", message }) => assert!(message.contains(entry) && message.contains(&format!("`{text}`")) && !message.contains("value`"), "{message}"),
            Err(other) => panic!("{entry}={text}: E0802 naming the text, found {other}"),
            Ok(_) => panic!("{entry}={text}: accepted"),
        }
    }
}
