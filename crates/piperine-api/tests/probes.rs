//! `Probe` over elaborated MSL (CLI-05 AC 3 to 6; spec edge 3): the default columns, the three
//! kinds of entry in full or relative spelling, the column names, and an entry that names
//! nothing, refused by `Session::open` before any analysis. The divider is the twin
//! `tests/ngspice/divider.msl`; the mixed top is written by the test.

use std::path::{Path, PathBuf};

use piperine_api::{Column, Failure, Overrides, Probe, Project, Session};
use piperine_ir::{Ir, ValueType};
use piperine_lang::elab::Elaborator;
use piperine_lang::pom::Design;
use piperine_transpile::Settings;

fn divider() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/e2e/twins/divider.msl")
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

/// A top with a node, a source's flow and a discrete net, as a session on the compiled backend
/// (the interpreter runs no discrete net).
const BENCH: &str = "pub module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\npub module Vsource(p: Electrical, n: Electrical) { dc: f64 = 1.0; }\nanalog for Vsource { v(p, n) +> dc; }\n\nmodule Top(in en: Bit) {\n    signal a: Electrical;\n    signal flag: Bit = false;\n    let v1 = Vsource(a, gnd) { dc: 2.0 };\n    let r1 = Resistor(a, gnd) { r: 1k };\n}\n";

fn bench_session(dir: &tempfile::TempDir) -> Session {
    let file = dir.path().join("bench.msl");
    std::fs::write(&file, BENCH).expect("write");
    let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
    let overrides = Overrides { analysis: Some("op".into()), backend: Some("rust".into()), top: Some("Top".into()), ..Overrides::default() };
    let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
    Session::open(&project, &spec, &Settings { rustc: None, cache: dir.path().join("cache") }).unwrap_or_else(|f| panic!("{f}"))
}

#[test]
fn a_node_a_flow_and_a_net_resolve_to_their_columns() {
    // LR-03: a node and a flow to their index in the solution, a net to its id in the plan
    let dir = tempfile::tempdir().expect("tmp");
    let mut session = bench_session(&dir);
    let probes: Vec<Probe> = ["a", "i(v1)", "flag"].iter().map(|e| Probe::resolve(session.ir(), "Top", e).unwrap_or_else(|f| panic!("{f}"))).collect();
    let circuit = session.circuit().expect("the bound circuit");
    let (Column::Unknown { index: node_at }, Column::Unknown { index: flow_at }) =
        (probes[0].column(circuit).unwrap_or_else(|f| panic!("{f}")), probes[1].column(circuit).unwrap_or_else(|f| panic!("{f}")))
    else {
        panic!("a node and a flow are unknowns of the solution")
    };
    assert_ne!(node_at, flow_at, "two unknowns have two indices");
    let id = circuit.bound().expect("bound").digital.plan().nets.iter().position(|n| n.name == "Top.flag").expect("the plan holds Top.flag") as u32;
    assert_eq!(probes[2].column(circuit).unwrap_or_else(|f| panic!("{f}")), Column::Net { id, ty: ValueType::Bool });
}

#[test]
fn a_probe_with_no_value_in_the_circuit_is_a_failure_naming_it() {
    // LR-03 AC 3: a probe the circuit has no value for is refused before any step, by label
    let dir = tempfile::tempdir().expect("tmp");
    let mut session = bench_session(&dir);
    let circuit = session.circuit().expect("the bound circuit");
    let net = Probe::Net { path: "Top.nope".into(), ty: ValueType::Bool };
    let node = Probe::Node { path: "Top.ghost".into(), id: piperine_solver::prelude::NodeIdentifier::Anonymous(9999) };
    for probe in [net, node] {
        match probe.column(circuit) {
            Err(Failure::Backend(message)) => assert!(message.contains(&probe.label()), "{message}"),
            other => panic!("a Failure::Backend naming `{}`, found {other:?}", probe.label()),
        }
    }
}
