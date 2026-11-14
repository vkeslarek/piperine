//! Structural guards over the workspace manifests (feature `solver-restructure`).
//! Covers SR-01: the solver's own manifest lists no in-tree crate as a
//! dev-dependency and none of the dependencies the restructure removed.
//! Covers SR-02: retired crates have no directory and no manifest names them,
//! the root `tests/` holds no Verilog-AMS end-to-end target, and `examples/`
//! holds no Verilog-AMS or Python source, `default-members` lists the green
//! crates and the CI workflows name no retired crate.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every crate this feature has removed so far; each task that removes
/// crates extends this list.
const RETIRED_CRATES: &[&str] = &[
    "piperine-codegen",
    "piperine-ir",
    "piperine-python",
    "piperine-plugin",
    "piperine-plugin-macros",
];

/// The workspace's `default-members`: the crates that build and test green.
/// Later tasks extend this list as the sub-solver crates appear.
const DEFAULT_MEMBERS: &[&str] = &["crates/piperine-lang", "crates/piperine-solver"];

/// The root manifest and every `crates/*/Cargo.toml`.
fn all_manifests() -> Vec<PathBuf> {
    let root = repo_root();
    let mut out = vec![root.join("Cargo.toml")];
    for entry in std::fs::read_dir(root.join("crates")).expect("read crates/") {
        let manifest = entry.expect("dir entry").path().join("Cargo.toml");
        if manifest.exists() {
            out.push(manifest);
        }
    }
    out
}

/// The text of one `[section]` of a manifest, up to the next section header.
fn section(manifest: &str, name: &str) -> String {
    let header = format!("[{name}]");
    let mut out = String::new();
    let mut inside = false;
    for line in manifest.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            inside = t == header;
            continue;
        }
        if inside {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

#[test]
fn solver_manifest_has_no_in_tree_dev_dependency() {
    let path = repo_root().join("crates/piperine-solver/Cargo.toml");
    let manifest = std::fs::read_to_string(&path).expect("read the solver manifest");
    let dev = section(&manifest, "dev-dependencies");
    assert!(
        !dev.contains("path ="),
        "a dev-dependency points into the tree:\n{dev}"
    );
    for name in ["piperine-lang", "piperine-codegen", "piperine-ir"] {
        assert!(!dev.contains(name), "dev-dependency {name} remains");
    }
    for name in ["rayon", "rand_distr"] {
        assert!(!manifest.contains(name), "{name} remains in the solver manifest");
    }
}

#[test]
fn retired_crates_are_gone() {
    for name in RETIRED_CRATES {
        let dir = repo_root().join("crates").join(name);
        assert!(!dir.exists(), "{} still exists", dir.display());
    }
}

#[test]
fn no_manifest_names_a_retired_crate() {
    for path in all_manifests() {
        let text = std::fs::read_to_string(&path).expect("read a manifest");
        for name in RETIRED_CRATES {
            assert!(!text.contains(name), "{} names retired crate {name}", path.display());
        }
    }
    let root = std::fs::read_to_string(repo_root().join("Cargo.toml")).expect("read the root manifest");
    assert!(
        !root.contains("[profile.dev.package.cranelift"),
        "the root manifest keeps a Cranelift profile entry"
    );
}

/// Every file under `dir`, recursively.
fn files_under(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read a directory") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            files_under(&path, out);
        } else {
            out.push(path);
        }
    }
}

#[test]
fn root_tests_hold_no_end_to_end_target() {
    let tests = repo_root().join("tests");
    let mut files = Vec::new();
    files_under(&tests, &mut files);
    let ext = |p: &PathBuf, e: &str| p.extension().is_some_and(|x| x == e);
    let vams: Vec<_> = files.iter().filter(|p| ext(p, "vams")).collect();
    assert!(vams.is_empty(), ".vams files remain under tests/: {vams:?}");
    let cir = files
        .iter()
        .filter(|p| ext(p, "cir") && p.starts_with(tests.join("ngspice")))
        .count();
    assert!(cir > 0, "tests/ngspice/ lost its .cir fixtures");
    for entry in std::fs::read_dir(&tests).expect("read tests/") {
        let path = entry.expect("dir entry").path();
        if path.is_file() && ext(&path, "rs") {
            assert!(
                path.file_name().is_some_and(|n| n == "suite_hygiene.rs"),
                "{} is a root test target besides the hygiene guard",
                path.display()
            );
        }
    }
}

#[test]
fn examples_hold_no_vams_or_python() {
    let mut files = Vec::new();
    files_under(&repo_root().join("examples"), &mut files);
    for path in files {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        assert!(
            !matches!(ext, "vams" | "py" | "png") && name != "requirements.txt",
            "{} remains under examples/",
            path.display()
        );
    }
    for kept in ["Piperine.toml", "Piperine.lock"] {
        assert!(repo_root().join("examples").join(kept).exists(), "examples/{kept} was removed");
    }
}

#[test]
fn default_members_are_the_green_crates() {
    let root = std::fs::read_to_string(repo_root().join("Cargo.toml")).expect("read the root manifest");
    let workspace = section(&root, "workspace");
    let line = workspace
        .lines()
        .find(|l| l.trim_start().starts_with("default-members"))
        .expect("the root manifest has no default-members");
    let listed: Vec<&str> = line
        .split('"')
        .enumerate()
        .filter(|(i, _)| i % 2 == 1)
        .map(|(_, s)| s)
        .collect();
    assert_eq!(listed, DEFAULT_MEMBERS, "default-members drifted");
}

#[test]
fn workflows_name_no_retired_crate() {
    let dir = repo_root().join(".github/workflows");
    for entry in std::fs::read_dir(&dir).expect("read .github/workflows") {
        let path = entry.expect("dir entry").path();
        let text = std::fs::read_to_string(&path).expect("read a workflow");
        for name in RETIRED_CRATES {
            assert!(!text.contains(name), "{} names retired crate {name}", path.display());
        }
    }
    let ci = std::fs::read_to_string(dir.join("ci.yml")).expect("read ci.yml");
    assert!(!ci.contains("--workspace"), "ci.yml passes --workspace");
    assert!(!ci.contains("requirements.txt"), "ci.yml installs Python requirements");
}
