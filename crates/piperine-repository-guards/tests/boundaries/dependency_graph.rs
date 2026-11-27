//! The dependency arrows of `CLAUDE.md` "Pipeline", encoded as data: each crate's `[dependencies]`
//! on other crates of the tree is exactly the row below, so an added or dropped arrow fails here
//! and is a decision, not an accident.

use piperine_repository_guards::SourceTree;

/// Each crate and the in-tree crates its `[dependencies]` table names. The leaves (`piperine-abi`,
/// `piperine-ir`, `piperine-numbers`) name none; the sub-solvers name only `piperine-abi`; the owner
/// names both sub-solvers; the guards crate names nothing and nothing names it.
const DOCUMENTED_ARROWS: [(&str, &[&str]); 17] = [
    ("piperine", &["piperine-api"]),
    ("piperine-abi", &[]),
    ("piperine-analog-solver", &["piperine-abi"]),
    ("piperine-api", &["piperine-abi", "piperine-digital-solver", "piperine-interp", "piperine-ir", "piperine-lang", "piperine-numbers", "piperine-plan", "piperine-project", "piperine-solver", "piperine-transpile"]),
    ("piperine-cli", &["piperine-api", "piperine-lang", "piperine-transpile"]),
    ("piperine-device-fixtures", &["piperine-abi"]),
    ("piperine-digital-solver", &["piperine-abi"]),
    ("piperine-interp", &["piperine-abi", "piperine-ir", "piperine-plan", "piperine-solver"]),
    ("piperine-ir", &[]),
    ("piperine-lang", &["piperine-ir", "piperine-numbers"]),
    ("piperine-numbers", &[]),
    ("piperine-parity", &["piperine-api", "piperine-interp", "piperine-ir", "piperine-lang", "piperine-solver", "piperine-transpile"]),
    ("piperine-plan", &["piperine-abi", "piperine-ir", "piperine-solver"]),
    ("piperine-project", &["piperine-lang"]),
    ("piperine-repository-guards", &[]),
    ("piperine-solver", &["piperine-abi", "piperine-analog-solver", "piperine-digital-solver"]),
    ("piperine-transpile", &["piperine-abi", "piperine-ir", "piperine-plan", "piperine-solver"]),
];

/// Reads the arrows the manifests actually hold.
struct ManifestArrows;

impl ManifestArrows {
    /// The in-tree crates named by the `[dependencies]` table of the manifest text, sorted.
    fn dependencies_of(manifest: &str) -> Vec<String> {
        let table = SourceTree::manifest_section(manifest, "dependencies");
        let mut named: Vec<String> = table.lines().map(str::trim).filter(|line| line.starts_with("piperine-")).map(|line| line.split(['=', ' ']).next().unwrap_or("").to_string()).collect();
        named.sort();
        named
    }

    /// The manifest of a documented crate: the root one for `piperine`.
    fn manifest_of(name: &str) -> String {
        if name == "piperine" {
            return std::fs::read_to_string(SourceTree::workspace_root().join("Cargo.toml")).expect("read the root manifest");
        }
        SourceTree::crate_manifest(name)
    }
}

#[test]
fn the_dependency_graph_is_the_documented_one() {
    for (name, documented) in DOCUMENTED_ARROWS {
        let held = ManifestArrows::dependencies_of(&ManifestArrows::manifest_of(name));
        assert_eq!(held, documented, "{name}: [dependencies] on crates of the tree");
    }
}

#[test]
fn every_crate_of_the_tree_has_a_documented_row() {
    let documented: Vec<&str> = DOCUMENTED_ARROWS.iter().map(|(name, _)| *name).collect();
    for path in SourceTree::manifest_paths().iter().filter(|path| *path != &SourceTree::workspace_root().join("Cargo.toml")) {
        let name = path.parent().and_then(|directory| directory.file_name()).and_then(|name| name.to_str()).unwrap_or("");
        assert!(documented.contains(&name), "{name} has no row in DOCUMENTED_ARROWS");
    }
}

#[test]
fn the_leaves_and_the_sub_solvers_keep_their_dev_dependencies_out_of_the_tree() {
    for name in ["piperine-abi", "piperine-ir", "piperine-numbers", "piperine-analog-solver", "piperine-digital-solver", "piperine-repository-guards"] {
        let dev = SourceTree::manifest_section(&ManifestArrows::manifest_of(name), "dev-dependencies");
        let held = dev.lines().filter(|line| line.trim().starts_with("piperine-") && !line.trim().starts_with("piperine-device-fixtures"));
        assert!(held.count() == 0, "{name} has an in-tree dev-dependency other than the fixtures crate:\n{dev}");
    }
}
