//! Structural guards over the workspace manifests (feature `solver-restructure`).
//! Covers SR-01: the solver's own manifest lists no in-tree crate as a
//! dev-dependency and none of the dependencies the restructure removed.
//! Covers SR-02: retired crates have no directory and no manifest names them,
//! the root `tests/` holds no Verilog-AMS end-to-end target, and `examples/`
//! holds no Verilog-AMS or Python source, `default-members` lists the green
//! crates and the CI workflows name no retired crate. Covers SAC-12 AC 3
//! (`solver-abi-core`): the instruction files name `piperine-abi` and
//! `.specs/STATE.md` records MD-61. Covers SAI-16 (`solver-abi-integration`):
//! `.specs/STATE.md` records MD-62 and the instruction files name no removed item.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every crate this feature has removed so far; each task that removes
/// crates extends this list.
const RETIRED_CRATES: &[&str] = &[
    "piperine-codegen",
    "piperine-python",
    "piperine-plugin",
    "piperine-plugin-macros",
];

/// The workspace's `default-members`: the crates that build and test green.
/// Later tasks extend this list as the sub-solver crates appear.
const DEFAULT_MEMBERS: &[&str] = &[
    ".",
    "crates/piperine-abi",
    "crates/piperine-analog-solver",
    "crates/piperine-api",
    "crates/piperine-cli",
    "crates/piperine-digital-solver",
    "crates/piperine-interp",
    "crates/piperine-ir",
    "crates/piperine-lang",
    "crates/piperine-numbers",
    "crates/piperine-parity",
    "crates/piperine-plan",
    "crates/piperine-project",
    "crates/piperine-solver",
    "crates/piperine-transpile",
];

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
    let tests = repo_root().join("tests");
    let mut files = Vec::new();
    files_under(&tests, &mut files);
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
    let manifest = std::fs::read_to_string(repo_root().join("Cargo.toml")).expect("read the root manifest");
    let declared: Vec<PathBuf> = manifest
        .lines()
        .filter_map(|line| line.trim().strip_prefix("path = \""))
        .filter_map(|rest| rest.strip_suffix('"'))
        .map(|path| repo_root().join(path))
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

/// The crates the workspace keeps.
const KEPT_CRATES: &[&str] = &[
    "piperine-abi",
    "piperine-lang",
    "piperine-interp",
    "piperine-ir",
    "piperine-numbers",
    "piperine-project",
    "piperine-solver",
    "piperine-analog-solver",
    "piperine-digital-solver",
    "piperine-api",
    "piperine-cli",
    "piperine-lang-server",
    "piperine-parity",
    "piperine-plan",
    "piperine-transpile",
];

/// The crate names in the rows of the table under `## Crates` of `CLAUDE.md` (the first
/// backticked cell of each row).
fn claude_table_crates() -> Vec<String> {
    let text = std::fs::read_to_string(repo_root().join("CLAUDE.md")).expect("read CLAUDE.md");
    let body = text.split("## Crates").nth(1).expect("CLAUDE.md has a `## Crates` section");
    let section = body.split("\n## ").next().unwrap_or(body);
    section
        .lines()
        .filter(|l| l.starts_with("| `"))
        .filter_map(|l| l.split('`').nth(1).map(str::to_string))
        .collect()
}

#[test]
fn instructions_list_the_kept_crates() {
    let mut table = claude_table_crates();
    table.sort();
    let mut kept: Vec<String> = KEPT_CRATES.iter().map(|s| s.to_string()).collect();
    kept.sort();
    assert_eq!(table, kept, "CLAUDE.md crate table");
}

#[test]
fn instructions_have_no_entry_for_a_retired_crate() {
    let table = claude_table_crates();
    for retired in RETIRED_CRATES.iter().chain(["piperine-signal", "piperine-vams"].iter()) {
        assert!(!table.iter().any(|c| c == retired), "CLAUDE.md still lists retired crate {retired}");
    }
}

#[test]
fn state_records_the_solver_structure_decision() {
    let text = std::fs::read_to_string(repo_root().join(".specs/STATE.md")).expect("read STATE.md");
    let heading = "### MD-60: the solver is three crates; events are the only boundary";
    let body = text.split(heading).nth(1).expect("STATE.md records the decision heading");
    let section = body.split("\n### ").next().unwrap_or(body);
    for name in ["piperine-solver", "piperine-analog-solver", "piperine-digital-solver"] {
        assert!(section.contains(name), "the decision does not name {name}");
    }
}

#[test]
fn hygiene_guard_lives_in_piperine_lang() {
    let root = repo_root();
    assert!(root.join("crates/piperine-lang/tests/suite_hygiene.rs").is_file());
    assert!(root.join("crates/piperine-lang/tests/support/sources.rs").is_file());
    assert!(!root.join("tests/suite_hygiene.rs").exists());
    assert!(!root.join("tests/support").exists());
}

#[test]
fn instruction_files_and_state_name_the_abi_crate() {
    let claude = std::fs::read_to_string(repo_root().join("CLAUDE.md")).expect("read CLAUDE.md");
    assert!(claude.contains("piperine-abi"), "CLAUDE.md does not name piperine-abi");
    let state = std::fs::read_to_string(repo_root().join(".specs/STATE.md")).expect("read .specs/STATE.md");
    assert!(state.contains("### MD-61"), ".specs/STATE.md carries no MD-61");
    assert!(state.contains("### MD-62"), ".specs/STATE.md carries no MD-62");
    for file in ["CLAUDE.md", "AGENTS.md"] {
        let text = std::fs::read_to_string(repo_root().join(file)).expect("read an instruction file");
        for gone in ["Element", "ElementCapabilities", "AnalogDevice", "DigitalDevice", "LogicValue", "EventQueue"] {
            assert!(!text.contains(gone), "{file} still names the removed {gone}");
        }
    }
}

#[test]
fn the_root_package_is_a_re_export_and_nothing_else() {
    // CLI-01 AC 3 (msl-cli T4): the root `src/lib.rs` holds `pub use piperine_api::*;` and no
    // other item; comments and blank lines are not items
    let root = repo_root().join("src");
    let mut files = Vec::new();
    files_under(&root, &mut files);
    assert_eq!(files, vec![root.join("lib.rs")], "the root package has one source file");
    let text = std::fs::read_to_string(root.join("lib.rs")).expect("read src/lib.rs");
    let items: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("//")).collect();
    assert_eq!(items, ["pub use piperine_api::*;"], "src/lib.rs");
}

/// Whether `text` holds `word` as a whole identifier (no identifier character on either side).
fn names_identifier(text: &str, word: &str) -> bool {
    let ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    text.match_indices(word).any(|(at, _)| {
        let before = text[..at].chars().next_back().is_none_or(|c| !ident(c));
        let after = text[at + word.len()..].chars().next().is_none_or(|c| !ident(c));
        before && after
    })
}

#[test]
fn host_crates_name_no_deleted_surface() {
    // CLI-01 AC 4 (msl-cli T5): no source of `piperine-api` or `piperine-cli` names the deleted
    // Verilog-AMS front end or the deleted `Element` solver interface
    let mut files = Vec::new();
    for krate in ["piperine-api", "piperine-cli"] {
        let dir = repo_root().join("crates").join(krate);
        files.push(dir.join("Cargo.toml"));
        for sub in ["src", "tests"] {
            if dir.join(sub).is_dir() {
                files_under(&dir.join(sub), &mut files);
            }
        }
    }
    let mut offences = Vec::new();
    for path in files.iter().filter(|p| p.extension().is_some_and(|e| e == "rs" || e == "toml")) {
        let text = std::fs::read_to_string(path).expect("read a host source");
        for word in ["parse_and_elaborate", "Element", "piperine_codegen", "piperine_python", "piperine_plugin"] {
            if names_identifier(&text, word) {
                offences.push(format!("{}: `{word}`", path.display()));
            }
        }
        if text.contains(".vams") {
            offences.push(format!("{}: `.vams`", path.display()));
        }
    }
    assert!(files.len() > 2, "the walk found the two crates' sources: {files:?}");
    assert!(offences.is_empty(), "the host crates name the deleted surface:\n{}", offences.join("\n"));
}
