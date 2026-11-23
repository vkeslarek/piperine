//! `Project` over packages on disk (CLI-02 AC 1, 5, 7 to 11; CLI-03 AC 8 and 9): discovery
//! upward, the runs of the package's own manifest, and the crate graph of a run, rooted at a
//! bench crate `bench` over the package and its direct dependencies, at the package's crate, or
//! at a lone file. Every package is written by the test into a temporary directory.

use std::fs;
use std::path::{Path, PathBuf};

use piperine_api::{Analysis, Failure, Overrides, Project};

/// A resistor, as `examples/rc/src/lib.msl` writes it.
const RESISTOR: &str = "/// Ideal resistor.\npub module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\n";
/// A capacitor, the dependency's one module.
const CAPACITOR: &str = "/// Ideal capacitor.\npub module Capacitor(p: Electrical, n: Electrical) { c: f64 where c > 0.0; }\nanalog for Capacitor { i(p, n) +> ddt(c * v(p, n)); }\n";
/// A bench that reaches the package `rc` and its dependency `parts` by their names.
const BENCH: &str = "use rc::Resistor;\nuse parts::Capacitor;\n\nmodule Top() {\n    signal a: Electrical;\n    let r1 = Resistor(a, gnd) { r: 1k };\n    let c1 = Capacitor(a, gnd) { c: 1n };\n}\n";

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
    fs::write(path, text).expect("write");
}

/// A package `name` at `dir` with `src/lib.msl` = `lib` and the manifest tail `rest`.
fn package(dir: &Path, name: &str, lib: &str, rest: &str) {
    write(&dir.join("Piperine.toml"), &format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n{rest}"));
    write(&dir.join("src/lib.msl"), lib);
}

/// The package `rc` depending on `parts` (a sibling directory), with `runs`.
fn rc_with_parts(root: &Path, runs: &str) -> PathBuf {
    let rc = root.join("rc");
    package(&root.join("parts"), "parts", CAPACITOR, "");
    package(&rc, "rc", RESISTOR, &format!("\n[dependencies]\nparts = {{ path = \"../parts\" }}\n\n{runs}"));
    write(&rc.join("benches/top.msl"), BENCH);
    rc
}

fn open(dir: &Path) -> Project {
    Project::at(dir).unwrap_or_else(|f| panic!("{f}"))
}

fn op() -> Overrides {
    Overrides { analysis: Some("op".into()), ..Overrides::default() }
}

#[test]
fn discover_takes_the_nearest_manifest_upward() {
    // CLI-02 AC 1
    let tmp = tempfile::tempdir().expect("tmp");
    let outer = tmp.path().join("outer");
    package(&outer, "outer", RESISTOR, "");
    fs::create_dir_all(outer.join("benches/deep")).expect("mkdir");
    let found = Project::discover(&outer.join("benches/deep")).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(found.dir(), outer.canonicalize().expect("canonical"));
    assert_eq!(found.name(), "outer");
    // an inner package wins over the outer one
    let inner = outer.join("inner");
    package(&inner, "inner", RESISTOR, "");
    fs::create_dir_all(inner.join("x")).expect("mkdir");
    let found = Project::discover(&inner.join("x")).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!((found.name(), found.dir()), ("inner", inner.canonicalize().expect("canonical").as_path()));
}

#[test]
fn a_dependencys_run_table_is_ignored() {
    // CLI-02 AC 5: `parts` holds a run table that is malformed by CLI-03 (an unknown key, no
    // analysis); the project loads and its runs are the root package's only
    let tmp = tempfile::tempdir().expect("tmp");
    let rc = rc_with_parts(tmp.path(), "[run.op]\nanalysis = \"op\"\n");
    write(&tmp.path().join("parts/Piperine.toml"), "[package]\nname = \"parts\"\nversion = \"0.1.0\"\n\n[run.broken]\nstopp = 1\n");
    let project = open(&rc);
    let names: Vec<&str> = project.runs().iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["op"]);
    assert!(project.faults().is_empty(), "the dependency's table is not read");
    let spec = project.request(None, &Overrides::default()).unwrap_or_else(|f| panic!("{f}"));
    let design = project.design(&spec).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(design.diagnostics(), &[], "the graph with the dependency loads");
}

#[test]
fn a_bench_is_the_root_crate_over_the_package_and_its_dependencies() {
    // CLI-02 AC 7: the bench uses `rc::Resistor` (the package by its name) and
    // `parts::Capacitor` (a direct dependency by its name); both resolve
    let tmp = tempfile::tempdir().expect("tmp");
    let rc = rc_with_parts(tmp.path(), "[run.top]\nbench = \"benches/top.msl\"\nanalysis = \"op\"\n");
    let project = open(&rc);
    let spec = project.request(Some("top"), &Overrides::default()).unwrap_or_else(|f| panic!("{f}"));
    let design = project.design(&spec).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(design.diagnostics(), &[], "both paths resolve from the bench");
    let root = design.root_crate();
    assert_eq!(root.name(), "bench");
    let deps: Vec<(&str, &str)> = root.dependencies().into_iter().map(|(n, c)| (n, c.name())).collect();
    assert_eq!(deps, [("rc", "rc"), ("parts", "parts")]);
    // the package keeps its own edge to `parts`
    let rc_crate = design.krate("rc").expect("the package crate");
    assert_eq!(rc_crate.dependency("parts").map(|c| c.name()), Some("parts"));
}

#[test]
fn a_run_without_a_bench_roots_at_the_package() {
    // CLI-02 AC 8
    let tmp = tempfile::tempdir().expect("tmp");
    let rc = rc_with_parts(tmp.path(), "[run.lib]\nanalysis = \"op\"\n");
    let project = open(&rc);
    let spec = project.request(Some("lib"), &Overrides::default()).unwrap_or_else(|f| panic!("{f}"));
    let design = project.design(&spec).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(design.root_crate().name(), "rc");
    assert_eq!(design.diagnostics(), &[]);
}

#[test]
fn a_lone_file_with_no_manifest_is_a_crate_alone() {
    // CLI-02 AC 9: the file and `std`, nothing else
    let tmp = tempfile::tempdir().expect("tmp");
    let file = tmp.path().join("divider.msl");
    write(&file, &format!("{RESISTOR}\nmodule Divider() {{\n    signal a: Electrical;\n    let r1 = Resistor(a, gnd) {{ r: 1k }};\n}}\n"));
    let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(project.name(), "divider");
    let spec = project.request(None, &op()).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!((spec.name.as_str(), &spec.analysis), ("divider", &Analysis::Op));
    let design = project.design(&spec).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(design.diagnostics(), &[]);
    let crates: Vec<&str> = design.crates().into_iter().map(|c| c.name()).collect();
    assert_eq!(crates, ["divider", "std"]);
}

#[test]
fn a_file_under_a_package_is_its_bench() {
    // CLI-02 AC 10
    let tmp = tempfile::tempdir().expect("tmp");
    let rc = rc_with_parts(tmp.path(), "");
    let project = Project::file(&rc.join("benches/top.msl")).unwrap_or_else(|f| panic!("{f}"));
    let spec = project.request(None, &op()).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(spec.name, "top", "a lone file's run is named by its stem");
    let design = project.design(&spec).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(design.diagnostics(), &[]);
    let root = design.root_crate();
    assert_eq!(root.name(), "bench");
    assert_eq!(root.dependency("rc").map(|c| c.name()), Some("rc"), "the root depends on the package");
}

#[test]
fn a_file_under_src_is_refused_with_p0010() {
    // CLI-02 AC 11
    let tmp = tempfile::tempdir().expect("tmp");
    let rc = rc_with_parts(tmp.path(), "");
    write(&rc.join("src/a.msl"), "module A() {}\n");
    let refused = Project::file(&rc.join("src/a.msl")).expect_err("a file of the crate is no bench");
    match &refused {
        Failure::Host { code, message } => {
            assert_eq!(*code, "P0010");
            assert!(message.contains("part of the crate"), "{message}");
        }
        other => panic!("P0010, found {other:?}"),
    }
}

#[test]
fn a_missing_bench_file_is_p0010_at_the_value() {
    // CLI-03 AC 8: the span is the string
    let tmp = tempfile::tempdir().expect("tmp");
    let rc = rc_with_parts(tmp.path(), "[run.none]\nbench = \"benches/none.msl\"\nanalysis = \"op\"\n");
    let refused = open(&rc).request(Some("none"), &Overrides::default()).expect_err("no such bench");
    match &refused {
        Failure::Request { diagnostic, text, file } => {
            assert_eq!(diagnostic.code, "P0010");
            let span = diagnostic.primary.0;
            assert_eq!(&text[span.start..span.end], "\"benches/none.msl\"");
            assert!(file.ends_with("Piperine.toml"), "{file:?}");
        }
        other => panic!("a located P0010, found {other:?}"),
    }
}

#[test]
fn the_only_run_is_run_without_a_name() {
    // CLI-03 AC 9
    let tmp = tempfile::tempdir().expect("tmp");
    let rc = rc_with_parts(tmp.path(), "[run.only]\nanalysis = \"op\"\n");
    let spec = open(&rc).request(None, &Overrides::default()).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!((spec.name.as_str(), &spec.analysis), ("only", &Analysis::Op));
    // two runs and no name: the request names both (CLI-03 AC 10's rule, in the library)
    let two = rc_with_parts(&tmp.path().join("two"), "[run.a]\nanalysis = \"op\"\n\n[run.b]\nanalysis = \"op\"\n");
    match open(&two).request(None, &Overrides::default()) {
        Err(Failure::NoRun { asked: None, runs }) => assert_eq!(runs, ["a", "b"]),
        other => panic!("NoRun naming a and b, found {other:?}"),
    }
}

/// The codes of the `typeck` diagnostics of a failed check, or a panic naming what else failed.
fn typeck_findings(checked: Result<piperine_api::Checked, Vec<Failure>>) -> Vec<String> {
    let failures = checked.expect_err("the check fails");
    assert_eq!(failures.len(), 1, "one failure: {failures:?}");
    match &failures[0] {
        Failure::Diagnostics { stage: "typeck", found, .. } => found.iter().map(|d| d.diagnostic.code.to_string()).collect(),
        other => panic!("a typeck failure, found {other:?}"),
    }
}

#[test]
fn a_library_package_with_no_run_is_type_checked() {
    // GN-08 AC 2: `rc` has no `[run]` table; a function nothing calls is wrong
    let tmp = tempfile::tempdir().expect("tmp");
    let rc = rc_with_parts(tmp.path(), "");
    let clean = open(&rc).check().unwrap_or_else(|f| panic!("{f:?}"));
    assert_eq!((clean.runs, clean.typed), (0, true), "the package is typed with no run");
    write(&rc.join("src/lib.msl"), &format!("{RESISTOR}fn broken(x: f64) -> bool {{ x + 1.0 }}\n"));
    assert_eq!(typeck_findings(open(&rc).check()), ["E0180"]);
}

#[test]
fn a_dependency_crate_is_type_checked() {
    // GN-08 AC 1: the error is in `parts`, a dependency of the package
    let tmp = tempfile::tempdir().expect("tmp");
    let rc = rc_with_parts(tmp.path(), "");
    write(&tmp.path().join("parts/src/lib.msl"), &format!("{CAPACITOR}pub fn broken(x: f64) -> bool {{ x + 1.0 }}\n"));
    assert_eq!(typeck_findings(open(&rc).check()), ["E0180"]);
}
