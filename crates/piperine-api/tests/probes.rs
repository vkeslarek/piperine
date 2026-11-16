//! `Probe` over elaborated MSL (CLI-05 AC 3 to 6; spec edge 3): the default columns, the three
//! kinds of entry in full or relative spelling, the column names, and an entry that names
//! nothing, refused by `Session::open` before any analysis. The divider is the twin
//! `tests/ngspice/divider.msl`; the mixed top is written by the test.

use std::path::{Path, PathBuf};

use piperine_api::{Failure, Overrides, Probe, Project, Session};
use piperine_ir::{Ir, ValueType};
use piperine_lang::elab::Elaborator;
use piperine_lang::pom::Design;
use piperine_transpile::Settings;

fn divider() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/ngspice/divider.msl")
}

/// `top` of the file `path`, elaborated.
fn elaborated(path: &Path, top: &str) -> Ir {
    let design = Design::load(path);
    assert_eq!(design.diagnostics(), &[], "{}", path.display());
    Elaborator::new(&design).run(top).unwrap_or_else(|d| panic!("{d:?}"))
}

/// A top with a discrete port, a node and a discrete signal, in that order, over a resistor.
const MIXED: &str = "pub module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\n\nmodule Top(in en: Bit) {\n    signal a: Electrical;\n    signal flag: Bit = false;\n    let r1 = Resistor(a, gnd) { r: 1k };\n}\n";

fn mixed() -> (tempfile::TempDir, Ir) {
    let dir = tempfile::tempdir().expect("tmp");
    let file = dir.path().join("mixed.msl");
    std::fs::write(&file, MIXED).expect("write");
    let ir = elaborated(&file, "Top");
    (dir, ir)
}

fn labels(probes: &[Probe]) -> Vec<String> {
    probes.iter().map(Probe::label).collect()
}

/// A session of the divider on the interpreter, printing `print`.
fn divider_session(print: &[&str]) -> Result<Session, Failure> {
    let project = Project::file(&divider()).unwrap_or_else(|f| panic!("{f}"));
    let overrides = Overrides {
        analysis: Some("op".into()),
        backend: Some("interp".into()),
        print: print.iter().map(|s| s.to_string()).collect(),
        ..Overrides::default()
    };
    let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
    Session::open(&project, &spec, &Settings { rustc: None, cache: PathBuf::from("/nonexistent/cache") })
}

#[test]
fn the_default_is_the_tops_own_signals_in_order() {
    // CLI-05 AC 3: divider.msl declares `vin` then `mid`; the nodes of `v1`, `r1` and `r2` are
    // the top's, and no child's own signal is listed
    let ir = elaborated(&divider(), "Divider");
    assert_eq!(labels(&Probe::defaults(&ir, "Divider")), ["Divider.vin", "Divider.mid"]);
    // ports and signals together, in declaration order: `en`, then `a`, then `flag`
    let (_dir, ir) = mixed();
    assert_eq!(labels(&Probe::defaults(&ir, "Top")), ["Top.en", "Top.a", "Top.flag"]);
    // the session takes them when the run has no `print`
    let session = divider_session(&[]).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(labels(session.probes()), ["Divider.vin", "Divider.mid"]);
}

#[test]
fn a_node_a_net_and_a_flow_resolve_relative_or_full() {
    // CLI-05 AC 4
    let ir = elaborated(&divider(), "Divider");
    let resolve = |entry: &str| Probe::resolve(&ir, "Divider", entry).unwrap_or_else(|f| panic!("{entry}: {f}"));
    let mid = resolve("mid");
    assert!(matches!(&mid, Probe::Node { path, .. } if path == "Divider.mid"), "{mid:?}");
    assert_eq!(resolve("Divider.mid"), mid, "the full spelling is the same probe");
    let flow = resolve("i(v1)");
    assert!(matches!(&flow, Probe::Flow { instance, .. } if instance == "Divider.v1"), "{flow:?}");
    assert_eq!(resolve("i(Divider.v1)"), flow);
    let (_dir, ir) = mixed();
    let net = Probe::resolve(&ir, "Top", "flag").unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(net, Probe::Net { path: "Top.flag".into(), ty: ValueType::Bool });
    assert_eq!(Probe::resolve(&ir, "Top", "Top.flag").ok(), Some(net));
}

#[test]
fn a_column_is_named_by_the_full_path() {
    // CLI-05 AC 5
    let ir = elaborated(&divider(), "Divider");
    let probes: Vec<Probe> = ["mid", "i(v1)"].iter().map(|e| Probe::resolve(&ir, "Divider", e).unwrap_or_else(|f| panic!("{f}"))).collect();
    assert_eq!(labels(&probes), ["Divider.mid", "i(Divider.v1)"]);
}

#[test]
fn an_entry_that_names_nothing_is_e0801_before_any_analysis() {
    // CLI-05 AC 6: `Session::open` refuses, so no analysis can run
    for entry in ["nope", "i(nope)", "i(mid)"] {
        match divider_session(&[entry]) {
            Err(Failure::Host { code: "E0801", message }) => assert!(message.contains(&format!("`{entry}`")), "{message}"),
            Err(other) => panic!("E0801, found {other}"),
            Ok(_) => panic!("`{entry}` names nothing"),
        }
    }
}

#[test]
fn a_signal_printed_twice_is_two_columns() {
    // spec edge 3
    let session = divider_session(&["mid", "mid"]).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(labels(session.probes()), ["Divider.mid", "Divider.mid"]);
}
