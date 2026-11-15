//! Structural guards for the three-crate solver split (SR-04, SR-05):
//! `piperine-solver` owns the events and the public face; `piperine-analog-solver`
//! and `piperine-digital-solver` depend on nothing in the tree but the device ABI
//! crate `piperine-abi` (MD-61), which is a leaf; they name nothing of each
//! other's domain and hold the modules the design assigns them.

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

/// The `piperine-*` dependency keys of a manifest, in any table: a key line
/// (`piperine-x = ...`) and a table header (`[dependencies.piperine-x]`,
/// `[dev-dependencies.piperine-x]`, `[build-dependencies.piperine-x]`).
fn workspace_dependencies(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.starts_with("piperine-") {
            out.push(line.split(['=', ' ', '.']).next().unwrap_or("").to_string());
        } else if let Some(header) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            let table_key = ["dependencies.", "dev-dependencies.", "build-dependencies."]
                .iter()
                .find_map(|t| header.rsplit_once(t).filter(|(_, k)| k.starts_with("piperine-")));
            if let Some((_, key)) = table_key {
                out.push(key.trim_matches('"').to_string());
            }
        }
    }
    out
}

const ANALOG: &str = "piperine-analog-solver";
const DIGITAL: &str = "piperine-digital-solver";
const OWNER: &str = "piperine-solver";
const ABI: &str = "piperine-abi";

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
    "crossing.rs",
];

const DIGITAL_MODULES: &[&str] = &[
    "advance.rs",
    "dag.rs",
    "engine.rs",
    "error.rs",
    "instant.rs",
    "limits.rs",
    "net.rs",
    "observe.rs",
    "plan.rs",
    "queue.rs",
    "run.rs",
    "store.rs",
    "tick.rs",
];

#[test]
fn the_three_solver_crates_exist() {
    for name in [OWNER, ANALOG, DIGITAL] {
        let dir = crate_dir(name);
        assert!(dir.join("Cargo.toml").is_file(), "{name} has no Cargo.toml");
        assert!(dir.join("src/lib.rs").is_file(), "{name} has no src/lib.rs");
    }
}

/// The workspace crates a sub-solver manifest names other than `piperine-abi`
/// (MD-61), and its path dependencies on anything else.
fn sub_solver_violations(text: &str) -> Vec<String> {
    let mut out: Vec<String> = workspace_dependencies(text).into_iter().filter(|d| d != ABI).collect();
    for line in text.lines().map(str::trim) {
        if line.contains("path =") && !line.starts_with(ABI) {
            out.push(format!("path dependency: {line}"));
        }
    }
    out
}

#[test]
fn sub_solvers_depend_on_the_abi_crate_only() {
    for name in [ANALOG, DIGITAL] {
        let found = sub_solver_violations(&manifest(name));
        assert!(found.is_empty(), "{name} manifest names workspace crates other than {ABI}: {found:?}");
    }
    // The scan still catches a sub-solver depending on the other one.
    let planted = "[dependencies]\npiperine-abi = { path = \"../piperine-abi\" }\npiperine-digital-solver = { path = \"../piperine-digital-solver\" }\n";
    let found = sub_solver_violations(planted);
    assert!(found.iter().any(|f| f == DIGITAL), "a planted dependency on {DIGITAL} is not caught: {found:?}");
    assert!(sub_solver_violations("[dependencies]\npiperine-abi = { path = \"../piperine-abi\" }\n").is_empty());
}

#[test]
fn abi_crate_is_a_leaf() {
    let text = manifest(ABI);
    assert!(!text.contains("path ="), "{ABI} manifest holds a path dependency");
    let deps = workspace_dependencies(&text);
    assert!(deps.is_empty(), "{ABI} manifest names workspace crates: {deps:?}");
    let root = std::fs::read_to_string(repo_root().join("Cargo.toml")).expect("read the root manifest");
    assert!(root.contains("\"crates/piperine-abi\""), "the workspace does not list {ABI}");
}

#[test]
fn owner_depends_on_the_abi_and_both_sub_solvers_only() {
    let mut deps = workspace_dependencies(&manifest(OWNER));
    deps.sort();
    deps.dedup();
    assert_eq!(deps, vec![ABI.to_string(), ANALOG.to_string(), DIGITAL.to_string()]);
}

#[test]
fn analog_sources_name_no_digital_type() {
    let found = names_in_crate(
        ANALOG,
        &["LogicValue", "DigitalNet", "DigitalEvent", "DigitalState", "DigitalDevice", "DigitalPlan", "NetId"],
    );
    assert!(found.is_empty(), "analog crate names digital types:\n{}", found.join("\n"));
}

#[test]
fn digital_sources_name_no_analog_type() {
    let found = names_in_crate(
        DIGITAL,
        &["Stamp", "Netlist", "AnalogReference", "Tolerances", "NewtonRaphsonSolver", "AnalogPlan", "Assembly"],
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

#[test]
fn the_manifest_scan_reads_table_form_dependencies() {
    let planted = "[package]\nname = \"x\"\n\n[dependencies.piperine-lang]\npath = \"../piperine-lang\"\n\n[dev-dependencies.piperine-ir]\nworkspace = true\n";
    assert_eq!(workspace_dependencies(planted), vec!["piperine-lang", "piperine-ir"]);
    assert_eq!(workspace_dependencies("[dependencies]\npiperine-a = { path = \"a\" }\n"), vec!["piperine-a"]);
    assert!(workspace_dependencies("[dependencies]\nfaer = \"1\"\n").is_empty());
}
