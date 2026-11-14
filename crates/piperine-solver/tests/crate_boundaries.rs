//! Structural guards for the three-crate solver split (SR-04, SR-05):
//! `piperine-solver` owns the events and the public face; `piperine-analog-solver`
//! and `piperine-digital-solver` depend on nothing in the tree, name nothing of
//! each other's domain, and hold the modules the design assigns them.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn crate_dir(name: &str) -> PathBuf {
    repo_root().join("crates").join(name)
}

/// Every `.rs` file under `dir`, recursively.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read dir") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// `file: name` for every `names` entry found in a `.rs` under `crate_name`.
fn names_in_crate(crate_name: &str, names: &[&str]) -> Vec<String> {
    let mut files = Vec::new();
    rust_files(&crate_dir(crate_name), &mut files);
    let mut found = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(&file).expect("read source");
        for n in names.iter().filter(|n| text.contains(**n)) {
            found.push(format!("{}: {n}", file.display()));
        }
    }
    found
}

fn manifest(crate_name: &str) -> String {
    std::fs::read_to_string(crate_dir(crate_name).join("Cargo.toml"))
        .unwrap_or_else(|e| panic!("read the manifest of {crate_name}: {e}"))
}

/// The `piperine-*` dependency keys of a manifest, in any table.
fn workspace_dependencies(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim_start)
        .filter(|l| l.starts_with("piperine-"))
        .map(|l| l.split(['=', ' ', '.']).next().unwrap_or("").to_string())
        .collect()
}

const ANALOG: &str = "piperine-analog-solver";
const DIGITAL: &str = "piperine-digital-solver";
const OWNER: &str = "piperine-solver";

const ANALOG_MODULES: &[&str] = &[
    "error.rs",
    "math/mod.rs",
    "math/num.rs",
    "math/linear.rs",
    "math/circular_array.rs",
    "math/iv.rs",
    "math/faer.rs",
    "math/newton_raphson.rs",
    "math/integration.rs",
    "netlist.rs",
    "tolerances.rs",
    "config.rs",
    "convergence.rs",
    "stamp_validation.rs",
];

const DIGITAL_MODULES: &[&str] = &[
    "events.rs",
    "drivers.rs",
    "interface.rs",
    "state.rs",
    "topology.rs",
    "scheduler.rs",
    "limits.rs",
    "error.rs",
];

#[test]
fn the_three_solver_crates_exist() {
    for name in [OWNER, ANALOG, DIGITAL] {
        let dir = crate_dir(name);
        assert!(dir.join("Cargo.toml").is_file(), "{name} has no Cargo.toml");
        assert!(dir.join("src/lib.rs").is_file(), "{name} has no src/lib.rs");
    }
}

#[test]
fn sub_solvers_depend_on_no_workspace_crate() {
    for name in [ANALOG, DIGITAL] {
        let text = manifest(name);
        assert!(!text.contains("path ="), "{name} manifest holds a path dependency");
        let deps = workspace_dependencies(&text);
        assert!(deps.is_empty(), "{name} manifest names workspace crates: {deps:?}");
    }
}

#[test]
fn owner_depends_on_both_sub_solvers_only() {
    let mut deps = workspace_dependencies(&manifest(OWNER));
    deps.sort();
    deps.dedup();
    assert_eq!(deps, vec![ANALOG.to_string(), DIGITAL.to_string()]);
}

#[test]
fn analog_sources_name_no_digital_type() {
    let found = names_in_crate(
        ANALOG,
        &["LogicValue", "DigitalNet", "DigitalEvent", "DigitalState", "DigitalDevice"],
    );
    assert!(found.is_empty(), "analog crate names digital types:\n{}", found.join("\n"));
}

#[test]
fn digital_sources_name_no_analog_type() {
    let found = names_in_crate(
        DIGITAL,
        &["Stamp", "Netlist", "AnalogReference", "Tolerances", "NewtonRaphsonSolver"],
    );
    assert!(found.is_empty(), "digital crate names analog types:\n{}", found.join("\n"));
}

#[test]
fn each_sub_solver_holds_its_modules() {
    for (name, modules) in [(ANALOG, ANALOG_MODULES), (DIGITAL, DIGITAL_MODULES)] {
        for m in modules {
            let path = crate_dir(name).join("src").join(m);
            assert!(path.is_file(), "{name} is missing src/{m}");
        }
    }
    for gone in ["math", "analog.rs", "digital", "error.rs"] {
        let path = crate_dir(OWNER).join("src").join(gone);
        assert!(!path.exists(), "the owner still has src/{gone}");
    }
}
