//! Structural guards over the workspace manifests (feature `solver-restructure`).
//! Covers SR-01: the solver's own manifest lists no in-tree crate as a
//! dev-dependency and none of the dependencies the restructure removed.
//! Covers SR-02: retired crates have no directory and no manifest names them.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every crate this feature has removed so far; each task that removes
/// crates extends this list.
const RETIRED_CRATES: &[&str] = &["piperine-codegen", "piperine-ir"];

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
