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
    "piperine-ir",
    "piperine-python",
    "piperine-plugin",
    "piperine-plugin-macros",
];

/// The workspace's `default-members`: the crates that build and test green.
/// Later tasks extend this list as the sub-solver crates appear.
const DEFAULT_MEMBERS: &[&str] = &[
    "crates/piperine-abi",
    "crates/piperine-analog-solver",
    "crates/piperine-digital-solver",
    "crates/piperine-lang",
    "crates/piperine-parity",
    "crates/piperine-solver",
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
    let rs: Vec<_> = files.iter().filter(|p| ext(p, "rs")).collect();
    assert!(rs.is_empty(), ".rs files remain under the root tests/: {rs:?}");
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

/// The ten crates the workspace keeps.
const KEPT_CRATES: &[&str] = &[
    "piperine-abi",
    "piperine-lang",
    "piperine-project",
    "piperine-solver",
    "piperine-analog-solver",
    "piperine-digital-solver",
    "piperine-api",
    "piperine-cli",
    "piperine-lang-server",
    "piperine-parity",
];

/// The crate names in the rows of the table under `## Crate responsibilities`
/// of `CLAUDE.md` (the first backticked cell of each row).
fn claude_table_crates() -> Vec<String> {
    let text = std::fs::read_to_string(repo_root().join("CLAUDE.md")).expect("read CLAUDE.md");
    let body = text
        .split("## Crate responsibilities")
        .nth(1)
        .expect("CLAUDE.md has a `## Crate responsibilities` section");
    let section = body.split("\n## ").next().unwrap_or(body);
    section
        .lines()
        .filter(|l| l.starts_with("| `"))
        .filter_map(|l| l.split('`').nth(1).map(str::to_string))
        .collect()
}

/// The crate directory names in the fenced block under `## Workspace map` of
/// `AGENTS.md` (lines of the tree that name a `piperine-*` directory).
fn agents_map_crates() -> Vec<String> {
    let text = std::fs::read_to_string(repo_root().join("AGENTS.md")).expect("read AGENTS.md");
    let body = text
        .split("## Workspace map")
        .nth(1)
        .expect("AGENTS.md has a `## Workspace map` section");
    let section = body.split("\n## ").next().unwrap_or(body);
    let block = section.split("```").nth(1).expect("a fenced block under the map");
    block
        .lines()
        .filter_map(|l| {
            let at = l.find("piperine-")?;
            let name: String = l[at..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
                .collect();
            Some(name)
        })
        .collect()
}

#[test]
fn instructions_list_the_kept_crates() {
    let mut table = claude_table_crates();
    table.sort();
    let mut map = agents_map_crates();
    map.sort();
    let mut kept: Vec<String> = KEPT_CRATES.iter().map(|s| s.to_string()).collect();
    kept.sort();
    assert_eq!(table, kept, "CLAUDE.md crate table");
    assert_eq!(map, kept, "AGENTS.md workspace map");
    instruction_text_places_the_moved_code_and_states_the_boundary_as_target();
}

/// The row of the `CLAUDE.md` crate table for `name`.
fn claude_row(name: &str) -> String {
    let text = std::fs::read_to_string(repo_root().join("CLAUDE.md")).expect("read CLAUDE.md");
    let prefix = format!("| `{name}` |");
    text.lines()
        .find(|l| l.starts_with(&prefix))
        .unwrap_or_else(|| panic!("CLAUDE.md has no row for {name}"))
        .to_string()
}

/// The role text of both instruction files; run by `instructions_list_the_kept_crates`.
fn instruction_text_places_the_moved_code_and_states_the_boundary_as_target() {
    let claude = std::fs::read_to_string(repo_root().join("CLAUDE.md")).expect("read CLAUDE.md");
    let agents = std::fs::read_to_string(repo_root().join("AGENTS.md")).expect("read AGENTS.md");
    let owner = claude_row("piperine-solver");
    let analog = claude_row("piperine-analog-solver");
    for moved in ["math/", "ConvergencePlan", "TrBdf2"] {
        assert!(!owner.contains(moved), "the owner row still claims {moved}");
        assert!(analog.contains(moved), "the analog row does not hold {moved}");
    }
    assert!(!claude.contains("analyses/convergence.rs"), "CLAUDE.md cites the old convergence path");
    assert!(!claude.contains("`math/integration.rs`") || claude.contains("analog crate, `math/integration.rs`"));
    assert!(owner.contains("`piperine-abi`"), "the owner row omits its `piperine-abi` dependency");
    for (name, text) in [("CLAUDE.md", &claude), ("AGENTS.md", &agents)] {
        let flat = text.replace('\n', " ");
        let at = flat.find("owner `piperine-solver` depends on").unwrap_or_else(|| panic!("{name} states no owner dependency"));
        let sentence = flat[at..].split_once(". ").map_or(&flat[at..], |(s, _)| s);
        assert!(sentence.contains("piperine-abi"), "{name}: the owner's dependency sentence omits `piperine-abi`: {sentence}");
    }
    let agents_owner = agents.lines().find(|l| l.contains("piperine-solver/")).expect("map row");
    assert!(!agents_owner.contains("math/"), "AGENTS.md owner map row still lists math/");
    for (name, text) in [("CLAUDE.md", &claude), ("AGENTS.md", &agents)] {
        assert!(!text.contains("meet through events only"), "{name} states the events-only boundary as current");
        assert!(!text.contains("through events only."), "{name} states the events-only boundary as current");
        assert!(!text.contains("Events are the only boundary"), "{name} states the events-only boundary as current");
        assert!(text.contains("MD-62"), "{name} does not cite MD-62, the device ABI as the only device contract");
        assert!(text.contains("owner protocol"), "{name} does not name the owner protocol as the boundary");
        assert!(text.contains("root `piperine`"), "{name} omits the root shell from the non-building list");
        for gone in ["crates/piperine-plugin", "PluginHost", "codegen's own"] {
            assert!(!text.contains(gone), "{name} still references {gone}");
        }
    }
}

#[test]
fn instructions_have_no_entry_for_a_retired_crate() {
    let both: Vec<String> = claude_table_crates().into_iter().chain(agents_map_crates()).collect();
    for retired in RETIRED_CRATES.iter().chain(["piperine-signal", "piperine-vams"].iter()) {
        assert!(
            !both.iter().any(|c| c == retired),
            "an instruction file still lists retired crate {retired}"
        );
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
    for file in ["CLAUDE.md", "AGENTS.md"] {
        let text = std::fs::read_to_string(repo_root().join(file)).expect("read an instruction file");
        assert!(text.contains("piperine-abi"), "{file} does not name piperine-abi");
    }
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
