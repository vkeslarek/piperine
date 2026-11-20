//! `Session::open` over MSL on disk (CLI-04 AC 1 to 3, 5, 6 and 9; spec edges 5 and 6): the
//! choice of the top, the backend's refusals, one compile per design, and a top whose ports are
//! discrete or continuous. Every Rust-backend session builds into a cache directory of its own
//! test, passed through `Settings`; no test sets a variable or writes the user's cache.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use piperine_api::{Built, Failure, Overrides, Project, RunSpec, Session};
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
fn a_run_fails_on_an_unreached_error_with_the_check_diagnostic() {
    // GN-09 AC 1: `broken` is called by nothing and the top is clean, yet `open` fails with the
    // diagnostic `Types::check_model` (and so `piperine check`) gives
    let text = format!("{DEVICES}\n{}\nfn broken(x: f64) -> bool {{ x + 1.0 }}\n", portless("Only"));
    let (_dir, file) = lone(&text);
    let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
    let spec = project.request(None, &flags("op", "interp", None)).unwrap_or_else(|f| panic!("{f}"));
    let design = project.design(&spec).unwrap_or_else(|f| panic!("{f}"));
    let expected = piperine_lang::types::Types::check_model(&design).expect_err("the model check fails");
    match Session::open(&project, &spec, &unused()) {
        Err(Failure::Diagnostics { stage: "typeck", found, .. }) => {
            assert_eq!(found, expected);
            assert_eq!(found.iter().map(|d| d.diagnostic.code.to_string()).collect::<Vec<_>>(), ["P0002"]);
        }
        Err(other) => panic!("a typeck failure, found {other}"),
        Ok(s) => panic!("a session with top {}", s.top()),
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
fn a_top_with_discrete_in_ports_runs_with_them_unknown() {
    // spec edge 5: the port `en` is a net of the top that nothing writes (contract "The top")
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let cache = tempfile::tempdir().expect("tmp");
    let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
    let text = format!(
        "{DEVICES}\nmodule Top(in en: Bit) {{\n    signal a: Electrical;\n    let v1 = Vsource(a, gnd) {{ dc: 1.0 }};\n    let r1 = Resistor(a, gnd) {{ r: 1k }};\n}}\n"
    );
    let mut session = open(&text, &flags("op", "rust", Some("Top")), &settings).unwrap_or_else(|f| panic!("{f}"));
    assert!(session.ir().net_at("Top.en").is_some(), "the port is a net of the top");
    let circuit = session.circuit().expect("a circuit");
    let result = circuit.transient(TransientAnalysisOptions::new(1e-9, 1e-10), Context::default()).expect("tran").solve().unwrap_or_else(|e| panic!("{e}"));
    // the first step is the operating point; a `Bit` is laid out as its value then its known
    // byte (abi.md §2): known byte 0 is unknown
    let first = result.get(0).expect("the operating point");
    let en = first.digital_named("Top.en").expect("the net `Top.en` in the result");
    assert_eq!(en.get(1), Some(&0), "`en` is unknown: {en:?}");
}

#[test]
fn a_top_with_a_continuous_port_is_the_front_ends_diagnostic() {
    // spec edge 6: an `Electrical` port of the top has nothing to connect to
    let text = format!("{DEVICES}\nmodule Top(a: Electrical) {{\n    let r1 = Resistor(a, gnd) {{ r: 1k }};\n}}\n");
    match open(&text, &flags("op", "interp", Some("Top")), &unused()) {
        Err(Failure::Diagnostics { stage, found, .. }) => {
            assert_eq!(stage, "elaborate");
            assert_eq!(found.len(), 1, "{found:?}");
            assert_eq!(found[0].diagnostic.code, "P0001", "{found:?}");
            assert!(found[0].diagnostic.message.contains("port"), "{}", found[0].diagnostic.message);
        }
        Err(other) => panic!("the front end's diagnostic, found {other}"),
        Ok(_) => panic!("a continuous port of the top is refused"),
    }
}
