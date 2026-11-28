//! Structural guards over the workspace manifests (feature `solver-restructure`).
//! Covers SR-01: the solver's own manifest lists no in-tree crate as a
//! dev-dependency and none of the dependencies the restructure removed.
//! Covers SR-02: the root `tests/` holds no Verilog-AMS end-to-end target, `examples/` holds no
//! Verilog-AMS or Python source, and `default-members` lists the green crates. The instruction
//! files list the kept crates. The guards against a retired architecture (retired crates, removed
//! names) left with code-clarity T9: no source of it has a way back.

use std::path::PathBuf;

use piperine_repository_guards::SourceTree;

/// The workspace's `default-members`: the crates that build and test green.
/// Later tasks extend this list as the sub-solver crates appear.
const DEFAULT_MEMBERS: &[&str] = &[
    ".",
    "crates/piperine-abi",
    "crates/piperine-analog-solver",
    "crates/piperine-api",
    "crates/piperine-cli",
    "crates/piperine-device-fixtures",
    "crates/piperine-digital-solver",
    "crates/piperine-heap-fixtures",
    "crates/piperine-interp",
    "crates/piperine-ir",
    "crates/piperine-ir-fixtures",
    "crates/piperine-lang",
    "crates/piperine-lang-fixtures",
    "crates/piperine-numbers",
    "crates/piperine-numbers-fixtures",
    "crates/piperine-parity",
    "crates/piperine-plan",
    "crates/piperine-project",
    "crates/piperine-repository-guards",
    "crates/piperine-solver",
    "crates/piperine-transpile",
];

#[test]
fn solver_manifest_has_no_in_tree_dev_dependency() {
    let path = SourceTree::workspace_root().join("crates/piperine-solver/Cargo.toml");
    let manifest = std::fs::read_to_string(&path).expect("read the solver manifest");
    let dev = SourceTree::manifest_section(&manifest, "dev-dependencies");
    let into_the_tree: Vec<&str> = dev.lines().filter(|line| line.contains("path =") && !line.contains("piperine-device-fixtures")).collect();
    assert!(into_the_tree.is_empty(), "a dev-dependency points into the tree (only `piperine-device-fixtures` may):\n{dev}");
    for name in ["piperine-lang", "piperine-codegen", "piperine-ir"] {
        assert!(!dev.contains(name), "dev-dependency {name} remains");
    }
    for name in ["rayon", "rand_distr"] {
        assert!(!manifest.contains(name), "{name} remains in the solver manifest");
    }
}

/// The Rust files under the root `tests/` read against the targets the root manifest declares.
struct RootTestFiles<'a> {
    /// The root `tests/` directory.
    tests: &'a std::path::Path,
    /// The paths the root manifest's `path = "..."` lines declare.
    declared: &'a [PathBuf],
    /// The text of each declared `main.rs` target, by path: what says which files it makes modules.
    main_sources: &'a [(PathBuf, String)],
}

impl RootTestFiles<'_> {
    /// Whether `file` is a module of a declared `main.rs` target: under that target's directory, and
    /// its first path component (`suite.rs` or `suite/...`) named by a `mod <name>;` line of the
    /// `main.rs` (the shape `tests/integrated/main.rs` gives a target of many files). A file the
    /// `main.rs` forgets is never compiled, so it is not accounted for.
    fn is_module_of_declared_main(&self, file: &PathBuf) -> bool {
        self.main_sources.iter().any(|(main, source)| {
            let Some(relative) = main.parent().and_then(|dir| file.strip_prefix(dir).ok()) else {
                return false;
            };
            let first = relative.components().next().map(|part| part.as_os_str().to_string_lossy().to_string()).unwrap_or_default();
            let module = first.strip_suffix(".rs").unwrap_or(&first);
            source.lines().any(|line| {
                let line = line.trim();
                line.strip_prefix("mod ").or_else(|| line.strip_prefix("pub mod ")).is_some_and(|rest| rest.trim_end_matches(';').trim() == module && rest.ends_with(';'))
            })
        })
    }

    /// The `.rs` files of `files` that no root target accounts for: not a top-level `tests/<name>.rs`
    /// (cargo finds it), not a declared `[[test]]` path, not support code of the Book targets and not
    /// a module of a declared `main.rs` target.
    fn undeclared<'f>(&self, files: &'f [PathBuf]) -> Vec<&'f PathBuf> {
        files
            .iter()
            .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
            .filter(|path| path.parent() != Some(self.tests))
            .filter(|path| !self.declared.contains(path))
            .filter(|path| !path.starts_with(self.tests.join("book").join("support")))
            .filter(|path| !self.is_module_of_declared_main(path))
            .collect()
    }
}

/// The modules of a declared `main.rs` target are accounted for under its own directory only; an
/// undeclared `.rs` anywhere else under `tests/` is still refused.
#[test]
fn a_declared_main_target_accounts_for_its_own_modules_only() {
    let tests = PathBuf::from("/repo/tests");
    let declared = vec![tests.join("integrated/main.rs")];
    let main_sources = vec![(tests.join("integrated/main.rs"), "mod suite;\nmod deep;\n// mod commented;\n".to_string())];
    let files = vec![
        tests.join("integrated/main.rs"),
        tests.join("integrated/suite.rs"),
        tests.join("integrated/deep/more.rs"),
        tests.join("integrated/orphan.rs"),
        tests.join("integrated/commented.rs"),
        tests.join("other/stray.rs"),
        tests.join("top_level.rs"),
    ];
    let undeclared = RootTestFiles { tests: &tests, declared: &declared, main_sources: &main_sources }.undeclared(&files);
    assert_eq!(undeclared, vec![&tests.join("integrated/orphan.rs"), &tests.join("integrated/commented.rs"), &tests.join("other/stray.rs")]);
}

#[test]
fn root_tests_hold_no_end_to_end_target() {
    let tests = SourceTree::workspace_root().join("tests");
    let files = SourceTree::files_under(&tests);
    let ext = |p: &PathBuf, e: &str| p.extension().is_some_and(|x| x == e);
    let vams: Vec<_> = files.iter().filter(|p| ext(p, "vams")).collect();
    assert!(vams.is_empty(), ".vams files remain under tests/: {vams:?}");
    let cir = files
        .iter()
        .filter(|p| ext(p, "cir") && p.starts_with(tests.join("e2e")))
        .count();
    assert!(cir > 0, "tests/e2e/ lost its .cir fixtures");
    // Rust under the root `tests/` is a test target the root manifest declares: a top-level
    // `tests/<name>.rs` (cargo finds it), a `[[test]]` path, or support code of the Book targets
    // under `tests/book/support/`. Anything else is an end-to-end target hiding in a directory.
    let manifest = std::fs::read_to_string(SourceTree::workspace_root().join("Cargo.toml")).expect("read the root manifest");
    let declared: Vec<PathBuf> = manifest
        .lines()
        .filter_map(|line| line.trim().strip_prefix("path = \""))
        .filter_map(|rest| rest.strip_suffix('"'))
        .map(|path| SourceTree::workspace_root().join(path))
        .collect();
    let main_sources: Vec<(PathBuf, String)> = declared
        .iter()
        .filter(|path| path.file_name().is_some_and(|name| name == "main.rs"))
        .map(|path| (path.clone(), std::fs::read_to_string(path).expect("read a declared main.rs")))
        .collect();
    let rs = RootTestFiles { tests: &tests, declared: &declared, main_sources: &main_sources }.undeclared(&files);
    assert!(rs.is_empty(), ".rs files under the root tests/ that no root target declares: {rs:?}");
}

#[test]
fn examples_hold_only_package_sources() {
    let files = SourceTree::files_under(&SourceTree::workspace_root().join("examples"));
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
        assert!(SourceTree::workspace_root().join("examples").join(kept).exists(), "examples/{kept} was removed");
    }
}

#[test]
fn default_members_are_the_green_crates() {
    let root = std::fs::read_to_string(SourceTree::workspace_root().join("Cargo.toml")).expect("read the root manifest");
    let workspace = SourceTree::manifest_section(&root, "workspace");
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

/// The crates the workspace keeps.
const KEPT_CRATES: &[&str] = &[
    "piperine-abi",
    "piperine-lang",
    "piperine-interp",
    "piperine-heap-fixtures",
    "piperine-ir",
    "piperine-ir-fixtures",
    "piperine-lang-fixtures",
    "piperine-numbers",
    "piperine-numbers-fixtures",
    "piperine-project",
    "piperine-solver",
    "piperine-analog-solver",
    "piperine-digital-solver",
    "piperine-device-fixtures",
    "piperine-api",
    "piperine-cli",
    "piperine-parity",
    "piperine-plan",
    "piperine-repository-guards",
    "piperine-transpile",
];

/// Reads the instruction files and the sources for the guards below.
struct InstructionReading;

impl InstructionReading {
    /// The crate names in the rows of the table under `## Crates` of `CLAUDE.md` (the first
    /// backticked cell of each row).
    pub fn claude_table_crates() -> Vec<String> {
        let text = std::fs::read_to_string(SourceTree::workspace_root().join("CLAUDE.md")).expect("read CLAUDE.md");
        let body = text.split("## Crates").nth(1).expect("CLAUDE.md has a `## Crates` section");
        let section = body.split("\n## ").next().unwrap_or(body);
        section
            .lines()
            .filter(|l| l.starts_with("| `"))
            .filter_map(|l| l.split('`').nth(1).map(str::to_string))
            .collect()
    }
}

#[test]
fn instructions_list_the_kept_crates() {
    let mut table = InstructionReading::claude_table_crates();
    table.sort();
    let mut kept: Vec<String> = KEPT_CRATES.iter().map(|s| s.to_string()).collect();
    kept.sort();
    assert_eq!(table, kept, "CLAUDE.md crate table");
}

#[test]
fn state_records_the_solver_structure_decision() {
    let text = std::fs::read_to_string(SourceTree::workspace_root().join(".specs/STATE.md")).expect("read STATE.md");
    let heading = "### MD-60: the solver is three crates; events are the only boundary";
    let body = text.split(heading).nth(1).expect("STATE.md records the decision heading");
    let section = body.split("\n### ").next().unwrap_or(body);
    for name in ["piperine-solver", "piperine-analog-solver", "piperine-digital-solver"] {
        assert!(section.contains(name), "the decision does not name {name}");
    }
}

#[test]
fn the_root_package_is_a_re_export_and_nothing_else() {
    // CLI-01 AC 3 (msl-cli T4): the root `src/lib.rs` holds `pub use piperine_api::*;` and no
    // other item; comments and blank lines are not items
    let root = SourceTree::workspace_root().join("src");
    let files = SourceTree::files_under(&root);
    assert_eq!(files, vec![root.join("lib.rs")], "the root package has one source file");
    let text = std::fs::read_to_string(root.join("lib.rs")).expect("read src/lib.rs");
    let items: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("//")).collect();
    assert_eq!(items, ["pub use piperine_api::*;"], "src/lib.rs");
}
