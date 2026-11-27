//! Shared repo-source-walking helpers.
//!
//! Extracted from `tests/suite_hygiene.rs` (T3, `lrm-conformance-wave2`) so
//! the LRM section coverage gate (`tests/lrm_section_coverage.rs`) does not
//! copy-paste a second walker — both consume the same `(repo-relative path,
//! contents)` shape over the same tree.

use std::fs;
use std::path::Path;

use piperine_repository_guards::SourceTree;

/// `suite_hygiene.rs` itself: it *spells* the forbidden patterns its own
/// guards look for, so it is the one source the walk must always skip — the
/// rule book is not a subject of its own rules.
const SELF: &str = "crates/piperine-repository-guards/tests/suite/suite_hygiene.rs";

/// The root `Cargo.toml`, read for the test targets it declares.
pub struct RootManifest;

impl RootManifest {
    /// The `path` of every `[[test]]` table of manifest `text`, in order. A `path` in any other
    /// table (a dependency, a `[[bin]]`) is not a test target.
    pub fn test_paths(text: &str) -> Vec<String> {
        let mut paths = Vec::new();
        let mut in_test_table = false;
        for line in text.lines().map(str::trim) {
            if line.starts_with('[') {
                in_test_table = line == "[[test]]";
            } else if in_test_table
                && let Some(value) = line.strip_prefix("path").map(str::trim_start).and_then(|rest| rest.strip_prefix('='))
            {
                paths.push(value.trim().trim_matches('"').to_string());
            }
        }
        paths
    }

    /// The test paths the repository's own root manifest declares.
    pub fn declared_test_paths() -> Vec<String> {
        let text = fs::read_to_string(SourceTree::workspace_root().join("Cargo.toml")).expect("the root manifest is readable");
        Self::test_paths(&text)
    }
}

/// The sources of the repository, walked for the guards of this target.
pub struct SourceWalk;

impl SourceWalk {
    /// Every `.rs` file under `crates/`, `src/`, and `tests/`, skipping build
    /// output and `suite_hygiene.rs`. Returned as `(repo-relative path, contents)`.
    pub fn rust_sources() -> Vec<(String, String)> {
        let root = SourceTree::workspace_root();
        let mut out = Vec::new();
        for base in ["crates", "src", "tests"] {
            SourceWalk::collect(&root.join(base), &root, &mut out);
        }
        assert!(
            out.len() > 100,
            "the walk found only {} sources — the layout moved and this guard is blind",
            out.len()
        );
        out
    }

    pub fn collect(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            if path.is_dir() {
                if name != "target" && name != ".git" {
                    SourceWalk::collect(&path, root, out);
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

    /// Every integration-test target: `crates/*/tests/*.rs`, root `tests/*.rs` and a root
    /// `tests/<dir>/<file>.rs` that the root manifest names in a `[[test]]` table (any other file
    /// of a `tests/<dir>/` is a helper, not a target).
    pub fn integration_targets() -> Vec<(String, String)> {
        let declared = RootManifest::declared_test_paths();
        SourceWalk::rust_sources()
            .into_iter()
            .filter(|(path, _)| {
                let parts: Vec<&str> = path.split('/').collect();
                matches!(parts.as_slice(), ["tests", _file] | ["crates", _, "tests", _file]) || declared.contains(path)
            })
            .collect()
    }

    /// Every root `tests/` target, as `(path under tests/, contents)` — top level and the
    /// `[[test]]` paths of `tests/<dir>/`, so a helper module is not mistaken for a target.
    pub fn root_targets() -> Vec<(String, String)> {
        let targets: Vec<(String, String)> = SourceWalk::integration_targets()
            .into_iter()
            .filter_map(|(path, text)| {
                path.strip_prefix("tests/").map(|file| (file.to_string(), text))
            })
            .collect();
        targets
    }

    /// Every library source: each crate's `src` tree plus the root package's.
    /// Integration targets are excluded — a lint suppression in a test file hides
    /// nothing that ships.
    pub fn library_sources() -> Vec<(String, String)> {
        let sources: Vec<(String, String)> = SourceWalk::rust_sources()
            .into_iter()
            .filter(|(path, _)| {
                let parts: Vec<&str> = path.split('/').collect();
                matches!(parts.as_slice(), ["src", ..] | ["crates", _, "src", ..])
            })
            .collect();
        assert!(
            sources.len() > 100,
            "the walk found only {} library sources — the layout moved and the \
             source-tree guards are blind",
            sources.len()
        );
        sources
    }
}
