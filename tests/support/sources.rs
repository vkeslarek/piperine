//! Shared repo-source-walking helpers.
//!
//! Extracted from `tests/suite_hygiene.rs` (T3, `lrm-conformance-wave2`) so
//! the LRM section coverage gate (`tests/lrm_section_coverage.rs`) does not
//! copy-paste a second walker — both consume the same `(repo-relative path,
//! contents)` shape over the same tree.

use std::fs;
use std::path::{Path, PathBuf};

/// The repository root — every consumer of this module lives in the root
/// package's `tests/` tree.
pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// `suite_hygiene.rs` itself: it *spells* the forbidden patterns its own
/// guards look for, so it is the one source the walk must always skip — the
/// rule book is not a subject of its own rules.
const SELF: &str = "tests/suite_hygiene.rs";

/// Every `.rs` file under `crates/`, `src/`, and `tests/`, skipping build
/// output and `suite_hygiene.rs`. Returned as `(repo-relative path, contents)`.
pub fn rust_sources() -> Vec<(String, String)> {
    let root = repo_root();
    let mut out = Vec::new();
    for base in ["crates", "src", "tests"] {
        collect(&root.join(base), &root, &mut out);
    }
    assert!(
        out.len() > 100,
        "the walk found only {} sources — the layout moved and this guard is blind",
        out.len()
    );
    out
}

fn collect(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if path.is_dir() {
            if name != "target" && name != ".git" {
                collect(&path, root, out);
            }
        } else if path.extension().is_some_and(|e| e == "rs") {
            let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().to_string();
            if rel == SELF {
                continue;
            }
            if let Ok(text) = fs::read_to_string(&path) {
                out.push((rel, text));
            }
        }
    }
}

/// Every integration-test target: `crates/*/tests/*.rs` and root `tests/*.rs`
/// (top level only — a `tests/<dir>/` module file is a helper, not a target).
pub fn integration_targets() -> Vec<(String, String)> {
    rust_sources()
        .into_iter()
        .filter(|(path, _)| {
            let parts: Vec<&str> = path.split('/').collect();
            matches!(parts.as_slice(), ["tests", _file] | ["crates", _, "tests", _file])
        })
        .collect()
}

/// Every root `tests/*.rs` target, as `(file name, contents)` — top level only,
/// so a `tests/<dir>/` helper module is not mistaken for a target.
pub fn root_targets() -> Vec<(String, String)> {
    let targets: Vec<(String, String)> = integration_targets()
        .into_iter()
        .filter_map(|(path, text)| {
            path.strip_prefix("tests/").map(|file| (file.to_string(), text))
        })
        .collect();
    assert!(
        targets.len() > 20,
        "the walk found only {} root targets — the layout moved and the \
         root-naming guard is blind",
        targets.len()
    );
    targets
}
