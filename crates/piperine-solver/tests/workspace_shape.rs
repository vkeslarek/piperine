//! Structural guards over the workspace manifests (feature `solver-restructure`).
//! Covers SR-01: the solver's own manifest lists no in-tree crate as a
//! dev-dependency and none of the dependencies the restructure removed.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
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
    for name in ["piperine-lang", "piperine-codegen"] {
        assert!(!dev.contains(name), "dev-dependency {name} remains");
    }
    for name in ["rayon", "rand_distr"] {
        assert!(!manifest.contains(name), "{name} remains in the solver manifest");
    }
}
