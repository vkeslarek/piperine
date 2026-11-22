//! Suite hygiene: the invariants that keep the test suite honest, enforced by
//! the suite itself (P6/CLN-05, CLN-08).
//!
//! P6 found 38 tests in two files that had been switched off with
//! `#![cfg(any())]` — one of them named in `CLAUDE.md` as a test of record —
//! looking like coverage while never compiling. A policy that lives only in a
//! document cannot catch that; these tests can.
//!
//! Each walks the repository's own sources (never a hardcoded list of test
//! names) so it cannot pass stale, and every failure names `file:line`.
//!
//! ## The root test-naming rule (MD-28 · CLA-13)
//!
//! The root package is a thin re-export shell (MD-20), so root `tests/` is for
//! the shell's own proofs — nothing else. Two shapes, and only two:
//!
//! - **`host_*.rs` — a host-surface proof.** It exercises the published host
//!   API, so it reaches the shell (`piperine::…`) or `piperine-api` directly.
//! - **`<feature>.rs` — a shell or cross-crate proof.** It either reaches the
//!   shell too, or it proves something no single crate owns by driving two or
//!   more workspace crates together.
//!
//! A target that exercises exactly one non-root crate is neither, and belongs
//! in `crates/<that-crate>/tests/`. `root_targets_match_the_naming_rule`
//! enforces it.

use std::fs;
use std::path::{Path, PathBuf};

// T3 (lrm-conformance-wave2): repo_root/rust_sources/collect/
// integration_targets/root_targets moved to `support::sources` so the LRM
// section coverage gate (T4) does not copy-paste this walker. Included by
// direct path (not `mod support;`) so this target does not also pull in
// `support::ngspice`'s own inline `#[cfg(test)]` module — that would change
// this target's test count, breaking the "behavior byte-identical" rule.
#[path = "support/sources.rs"]
mod sources;
use sources::{integration_targets, repo_root, root_targets, rust_sources, RootManifest};

/// Every library source: each crate's `src` tree plus the root package's.
/// Integration targets are excluded — a lint suppression in a test file hides
/// nothing that ships.
fn library_sources() -> Vec<(String, String)> {
    let sources: Vec<(String, String)> = rust_sources()
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

/// The library-source walk reaches the three solver crates and `piperine-lang`
/// (feature `solver-restructure`, SR-07): a guard that cannot see a crate
/// enforces nothing over it.
#[test]
fn the_walk_covers_the_three_solver_crates() {
    let sources = library_sources();
    for dir in [
        "crates/piperine-solver/src",
        "crates/piperine-analog-solver/src",
        "crates/piperine-digital-solver/src",
        "crates/piperine-lang/src",
    ] {
        assert!(
            sources.iter().any(|(path, _)| path.starts_with(dir)),
            "the library-source walk reaches no file under {dir}"
        );
    }
}

// ─── 1. No switched-off test code ─────────────────────────────────────────────

/// Whether one source line switches test code off. A file or module compiled
/// out of existence is worse than a missing test: it reads as coverage.
/// `#![cfg(any())]`, `#[cfg(FALSE)]`, and a commented-out `#[test]` are all the
/// same lie.
fn is_disabled_marker(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("#![cfg(any())]")
        || trimmed.starts_with("#[cfg(any())]")
        || trimmed.contains("cfg(FALSE)")
        || trimmed.contains("cfg(false)")
        || trimmed.starts_with("// #[test]")
        || trimmed.starts_with("//#[test]")
}

/// Whether one source line ignores a test.
fn is_ignore_attribute(line: &str) -> bool {
    line.trim_start().starts_with("#[ignore")
}

/// The detectors are tested against fixtures, not only against a clean tree:
/// a guard whose tree has no violations passes even when its detector is
/// broken, so the detector needs its own positive cases (found by P6's
/// discrimination sensor — mutants M3/M4 survived without this).
#[test]
fn the_detectors_recognise_what_they_forbid() {
    for offender in [
        "#![cfg(any())]",
        "    #[cfg(any())]",
        "#[cfg(FALSE)]",
        "#[cfg(false)]",
        "// #[test]",
        "//#[test]",
    ] {
        assert!(is_disabled_marker(offender), "must be flagged as disabled code: {offender:?}");
    }
    for innocent in ["#[test]", "#[cfg(test)]", "let x = 1; // #[test] in a sentence", ""] {
        assert!(!is_disabled_marker(innocent), "must not be flagged: {innocent:?}");
    }

    for offender in ["#[ignore]", "  #[ignore = \"flaky\"]"] {
        assert!(is_ignore_attribute(offender), "must be flagged as ignored: {offender:?}");
    }
    for innocent in ["#[test]", "/// #[ignore] in a doc comment", "let ignore = 1;"] {
        assert!(!is_ignore_attribute(innocent), "must not be flagged: {innocent:?}");
    }

    for offender in [
        "//! LRM-COVERS: §4.5.3",
        "//! LRM-COVERS: 4.5.3-4.5.5",
        "//! LRM-COVERS:",
        "//! LRM-COVERS: ",
        "//! LRM-COVERS: 4.5.3,",
        "//! LRM-COVERS: 4.5.x",
        "//! LRM-COVERS: Z.1",
    ] {
        assert!(is_malformed_lrm_covers(offender), "must be flagged as malformed: {offender:?}");
    }
    for innocent in [
        "//! LRM-COVERS: 4.5.3",
        "//! LRM-COVERS: 4.5.3, 7.3.5",
        "//! LRM-COVERS: A.1.2",
        "//! LRM-COVERS: 1",
        "// LRM-COVERS: §4.5.3 in a regular comment",
        "",
    ] {
        assert!(!is_malformed_lrm_covers(innocent), "must not be flagged: {innocent:?}");
    }

    for offender in [
        "#![allow(dead_code)]",
        "  #![allow(unused_imports)]",
        "#![allow(clippy::all)]",
    ] {
        assert!(is_file_scope_allow(offender), "must be flagged as file-scope: {offender:?}");
    }
    for innocent in ["#[allow(dead_code)]", "    #[allow(dead_code)]", "// #![allow(dead_code)]"] {
        assert!(!is_file_scope_allow(innocent), "must not be flagged: {innocent:?}");
    }

    for (offender, expected) in [
        ("//! walks an [`crate::resolve::IrProgram`]'s top module", "IrProgram"),
        ("/// no `IrModule` structural twin", "IrModule"),
        ("//! dispatches on POM `Expr`, not `IrExpr`", "IrExpr"),
        ("/// every module's `IrInstance.connections`", "IrInstance"),
    ] {
        assert_eq!(
            dead_architecture_identifiers_in(offender),
            vec![expected],
            "must be flagged as dead architecture: {offender:?}"
        );
    }
    for innocent in [
        "//! the POM `Design`/`Module`/`Instance`",
        "use piperine_lang::pom;",
        "//! `piperine-ir` is the compiler's IR crate (MD-40)",
        "use piperine_ir::core::Module;",
        "",
    ] {
        assert!(
            dead_architecture_identifiers_in(innocent).is_empty(),
            "must not be flagged: {innocent:?}"
        );
    }

    for offender in ["// hygiene-exempt: the frozen parse tree", "  // hygiene-exempt: x"] {
        assert!(is_hygiene_exemption(offender), "must be read as an exemption: {offender:?}");
    }
    for innocent in ["//! hygiene-exempt: in a doc header", "// hygiene exempt", "// exempt:", ""] {
        assert!(!is_hygiene_exemption(innocent), "must not be read as an exemption: {innocent:?}");
    }

    for reaching in ["use piperine::prelude::*;", "let s = piperine_api::Session::compile(x);"] {
        assert!(proves_host_surface(reaching), "must count as host surface: {reaching:?}");
    }
    for not_reaching in ["use piperine_solver::prelude::*;", "// piperine is the shell", ""] {
        assert!(!proves_host_surface(not_reaching), "must not count as host surface: {not_reaching:?}");
    }

    // `piperine_lang_server` contains `piperine_lang`: matching on `<ident>::`
    // is what keeps one import from counting as two crates.
    assert_eq!(
        workspace_crates_used("use piperine_lang_server::state::DocumentState;"),
        vec!["piperine_lang_server"]
    );
    assert_eq!(
        workspace_crates_used("use piperine_lang;\nuse piperine_solver::prelude::*;"),
        vec!["piperine_lang", "piperine_solver"]
    );
    assert!(workspace_crates_used("use std::fs;").is_empty());

    // The rule itself: a `host_*` target must reach the host surface; any other
    // target must reach the shell or two crates; one crate alone is an offence
    // that names the crate to move it to.
    assert!(root_naming_offence("host_probe.rs", "use piperine::prelude::*;").is_none());
    assert!(root_naming_offence("host_probe.rs", "use piperine_solver::prelude::*;").is_some());
    assert!(root_naming_offence("session.rs", "use piperine::prelude::*;").is_none());
    assert!(
        root_naming_offence(
            "plugin_parity.rs",
            "use piperine_plugin::host::PluginHost;\nuse piperine_solver::prelude::*;"
        )
        .is_none()
    );
    let single = root_naming_offence("sens.rs", "use piperine_solver::prelude::Context;")
        .expect("a single-crate root target is an offence");
    assert!(
        single.contains("crates/piperine-solver/tests/"),
        "the failure must name where the target belongs: {single}"
    );
}

#[test]
fn no_disabled_test_code() {
    let mut offences = Vec::new();
    for (path, text) in rust_sources() {
        for (index, line) in text.lines().enumerate() {
            if is_disabled_marker(line) {
                offences.push(format!("{path}:{}: {}", index + 1, line.trim()));
            }
        }
    }
    assert!(
        offences.is_empty(),
        "switched-off test code (delete it or make it compile):\n  {}",
        offences.join("\n  ")
    );
}

// ─── 2. No ignored tests ──────────────────────────────────────────────────────

/// An `#[ignore]`d test is work hidden from the gate, reason string or not.
/// The tree holds zero; this keeps it that way.
#[test]
fn no_ignored_tests() {
    let mut offences = Vec::new();
    for (path, text) in rust_sources() {
        for (index, line) in text.lines().enumerate() {
            if is_ignore_attribute(line) {
                offences.push(format!("{path}:{}", index + 1));
            }
        }
    }
    assert!(
        offences.is_empty(),
        "ignored tests (fix them or delete them — a skipped test proves nothing):\n  {}",
        offences.join("\n  ")
    );
}

// ─── 3. Every ignored doc example is accounted for ────────────────────────────

/// A ```` ```ignore ```` doc fence cannot carry a reason inline, so the reason
/// lives here: the registry below is the complete set of illustrative examples
/// that are deliberately not compiled. A new one fails this test until someone
/// records why — the same "registry + exhaustiveness" shape
/// `capabilities_contract.rs` uses for capability flags.
#[test]
fn every_ignored_doc_example_is_registered() {
    /// Why each file's `ignore` fence is not a runnable example.
    fn reason(path: &str) -> Option<&'static str> {
        Some(match path {
                "crates/piperine-solver/src/analyses/ac.rs" => {
                "illustrative sweep snippet: needs a built circuit the doc does not construct"
            }
            "crates/piperine-solver/src/core/builder.rs" => {
                "illustrative builder sketch over an Element the doc does not define"
            }
            "crates/piperine-solver/src/prelude.rs" => {
                "shows the import shape only — no runnable body"
            }
            _ => return None,
        })
    }

    let mut unregistered = Vec::new();
    let mut seen = Vec::new();
    for (path, text) in rust_sources() {
        if !text.contains("```ignore") {
            continue;
        }
        match reason(&path) {
            Some(_) => seen.push(path),
            None => unregistered.push(path),
        }
    }
    assert!(
        unregistered.is_empty(),
        "unregistered ```ignore doc examples (add the reason to this registry, \
         or make the example runnable):\n  {}",
        unregistered.join("\n  ")
    );
    assert_eq!(seen.len(), 3, "the registered set changed — update the registry: {seen:?}");
}

// ─── 4. Every integration target states its scope ─────────────────────────────

/// MD-28 rule 2: integration tests are grouped **by functionality**. A target
/// that cannot say in one `//!` line what it covers is not grouped — it is a
/// pile. This is the mechanical half of that rule (the semantic half is the
/// P6 allocation audit).
#[test]
fn every_integration_target_declares_its_scope() {
    let mut headerless = Vec::new();
    for (path, text) in integration_targets() {
        let has_header = text
            .lines()
            .take(5)
            .any(|line| line.trim_start().starts_with("//!"));
        if !has_header {
            headerless.push(path);
        }
    }
    assert!(
        headerless.is_empty(),
        "integration targets with no `//!` scope header (say what the file covers):\n  {}",
        headerless.join("\n  ")
    );
}

/// The root manifest's `[[test]]` paths are what makes a `tests/<dir>/<file>.rs` a target (the Book
/// targets of `feature root-e2e`): the reading finds each declared path and ignores a `path` of any
/// other table.
#[test]
fn the_root_manifest_names_its_test_targets() {
    let manifest = "[package]\nname = \"piperine\"\n\n[dependencies]\npiperine-api = { path = \"crates/piperine-api\" }\n\n\
                    [[test]]\nname = \"book_welcome\"\npath = \"tests/book/welcome.rs\"\n\n\
                    [[bin]]\nname = \"tool\"\npath = \"src/tool.rs\"\n\n\
                    [[test]]\nname = \"book_values\"\npath = \"tests/book/values.rs\"\n";
    assert_eq!(RootManifest::test_paths(manifest), vec!["tests/book/welcome.rs", "tests/book/values.rs"]);
    assert!(RootManifest::test_paths("[dependencies]\nx = { path = \"a/b\" }\n").is_empty());
}

// ─── Targets excluded from the default run (`test = false`, MD-77) ───────────

/// A manifest read for its on-demand targets: the `[[test]]` tables that carry `test = false`.
struct OnDemandManifest;

impl OnDemandManifest {
    /// The `name` of every `[[test]]` table of manifest `text` that sets `test = false`, in order. A
    /// `test = false` under any other table (`[lib]`, `[[bin]]`) is not a test target.
    fn targets(text: &str) -> Vec<String> {
        let mut targets = Vec::new();
        let mut in_test_table = false;
        let mut name = String::new();
        let mut flagged = false;
        for line in text.lines().map(Self::without_comment) {
            if line.starts_with('[') {
                if in_test_table && flagged {
                    targets.push(name.clone());
                }
                in_test_table = line == "[[test]]";
                name.clear();
                flagged = false;
            } else if in_test_table {
                let compact: String = line.split_whitespace().collect();
                if let Some(value) = compact.strip_prefix("name=") {
                    name = value.trim_matches('"').to_string();
                } else if compact == "test=false" {
                    flagged = true;
                }
            }
        }
        if in_test_table && flagged {
            targets.push(name);
        }
        targets
    }

    /// The `line` with its TOML `#` comment cut off and the ends trimmed (`test = false # slow` and
    /// `[[test]] # x` are legal TOML: cargo reads them as the bare key and header).
    fn without_comment(line: &str) -> &str {
        line.split('#').next().unwrap_or("").trim()
    }

    /// Every on-demand target of the repository's manifests as `(crate directory, target)`; the root
    /// package's directory is `.`.
    fn declared() -> Vec<(String, String)> {
        let root = repo_root();
        let mut manifests = vec![(".".to_string(), root.join("Cargo.toml"))];
        let mut crate_dirs: Vec<_> = fs::read_dir(root.join("crates"))
            .expect("the crates directory is readable")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .collect();
        crate_dirs.sort();
        for dir in crate_dirs {
            manifests.push((format!("crates/{dir}"), root.join("crates").join(&dir).join("Cargo.toml")));
        }
        let mut declared = Vec::new();
        for (dir, manifest) in manifests {
            if let Ok(text) = fs::read_to_string(manifest) {
                declared.extend(Self::targets(&text).into_iter().map(|target| (dir.clone(), target)));
            }
        }
        declared
    }
}

/// The targets a manifest excludes from `cargo test`, each with the reason it still exists: crate
/// directory, target name, why. `test = false` is a way to say "by name only", never a way to
/// switch a test off, so every flagged target is named here and in `examples/buck/README.md`'s
/// "On demand" block (`on_demand_targets_are_registered`).
const ON_DEMAND_TARGETS: [(&str, &str, &str); 12] = [
    ("crates/piperine-parity", "buck_whole", "simulates the `examples/buck` blocks or the whole converter; run by name (MD-77)"),
    ("crates/piperine-parity", "buck_dpwm", "simulates the `examples/buck` blocks or the whole converter; run by name (MD-77)"),
    ("crates/piperine-parity", "buck_control", "simulates the `examples/buck` blocks or the whole converter; run by name (MD-77)"),
    ("crates/piperine-parity", "buck_boundary", "simulates the `examples/buck` blocks or the whole converter; run by name (MD-77)"),
    ("crates/piperine-parity", "buck_power", "simulates the `examples/buck` blocks or the whole converter; run by name (MD-77)"),
    ("crates/piperine-parity", "buck_supervisor", "simulates the `examples/buck` blocks or the whole converter; run by name (MD-77)"),
    ("crates/piperine-parity", "buck_closed_loop", "simulates the `examples/buck` blocks or the whole converter; run by name (MD-77)"),
    ("crates/piperine-api", "buck_long", "the 600 us start of `examples/buck` through the host library; run by name (MD-77)"),
    ("crates/piperine-cli", "buck", "the binary on `examples/buck`'s `idle` and `startup` runs; run by name (MD-77)"),
    (".", "buck_scenarios", "SPEC S2 to S10 of `examples/buck` under a Rust host; run by name (MD-77)"),
    (".", "book_buck", "the 2 us run of the Book's `cli#44` buck bench, which compiles the whole converter; run by name (MD-77)"),
    ("crates/piperine-cli", "book_buck", "the Book's buck listing held to `piperine run startup`, the 600 us start; run by name (MD-77)"),
];

/// The registry and the README's "On demand" block, read against a set of declared targets.
struct OnDemandRegistry<'a> {
    /// The registry's entries: crate directory, target, why.
    entries: &'a [(&'a str, &'a str, &'a str)],
}

impl OnDemandRegistry<'_> {
    /// Every declared flagged target that no entry registers, named by crate and target.
    fn unregistered(&self, declared: &[(String, String)]) -> Vec<String> {
        declared
            .iter()
            .filter(|(dir, target)| !self.entries.iter().any(|(d, t, _)| d == dir && t == target))
            .map(|(dir, target)| format!("{dir}: target `{target}` has `test = false` but is not in `ON_DEMAND_TARGETS`"))
            .collect()
    }

    /// Every entry whose target is missing from the declared set (gone, or not flagged).
    fn stale(&self, declared: &[(String, String)]) -> Vec<String> {
        self.entries
            .iter()
            .filter(|(dir, target, _)| !declared.iter().any(|(d, t)| d == dir && t == target))
            .map(|(dir, target, _)| format!("{dir}: registered target `{target}` is missing or has no `test = false`"))
            .collect()
    }

    /// Every entry whose `--test <name>` the block of `readme` does not hold on a line that also
    /// carries the entry's `-p <package>` (both as whole words: `--test buck` is not `--test
    /// buck_whole`, and one crate's `book_buck` line is not the other's).
    fn undocumented(&self, readme: &str) -> Vec<String> {
        let block = Self::on_demand_block(readme);
        self.entries
            .iter()
            .filter(|(dir, target, _)| !block.lines().any(|line| Self::names_target(line, Self::package_of(dir), target)))
            .map(|(dir, target, _)| {
                format!("{dir}: target `{target}` is not named as `-p {} --test {target}` on one line of the README's \"On demand\" block", Self::package_of(dir))
            })
            .collect()
    }

    /// The package name of a crate directory: `crates/<name>` is `<name>`, the root `.` is `piperine`.
    fn package_of(dir: &str) -> &str {
        dir.strip_prefix("crates/").unwrap_or("piperine")
    }

    /// Whether `line` holds `-p <package>` and `--test <target>` as whole words.
    fn names_target(line: &str, package: &str, target: &str) -> bool {
        let words: Vec<&str> = line.split_whitespace().collect();
        let pair = |flag: &str, value: &str| words.windows(2).any(|pair| pair[0] == flag && pair[1] == value);
        pair("-p", package) && pair("--test", target)
    }

    /// The README text from its `## On demand` heading to the next `## ` heading (empty if absent).
    fn on_demand_block(readme: &str) -> String {
        let mut block = String::new();
        let mut inside = false;
        for line in readme.lines() {
            if line.starts_with("## ") {
                inside = line.trim() == "## On demand";
            }
            if inside {
                block.push_str(line);
                block.push('\n');
            }
        }
        block
    }
}

/// The reader finds exactly the `[[test]]` tables flagged `test = false`, whatever the key order,
/// and ignores a `test = false` of `[lib]`.
#[test]
fn the_on_demand_reader_finds_test_false() {
    let manifest = "[lib]\ntest = false\n\n[[test]]\nname = \"fast\"\npath = \"tests/fast.rs\"\n\n\
                    [[test]]\ntest = false\nname = \"slow\"\npath = \"tests/slow.rs\"\n";
    assert_eq!(OnDemandManifest::targets(manifest), vec!["slow".to_string()]);
    let last = "[[test]]\nname = \"tail\"\ntest = false\n";
    assert_eq!(OnDemandManifest::targets(last), vec!["tail".to_string()]);
    assert!(OnDemandManifest::targets("[lib]\ntest = false\n").is_empty());
    let commented = "[[test]] # on demand\nname = \"slow\" # the long one\ntest = false # slow\n";
    assert_eq!(OnDemandManifest::targets(commented), vec!["slow".to_string()]);
}

/// Every flagged target of every manifest is registered; a planted flagged target missing from a
/// planted registry is reported by crate and target.
#[test]
fn on_demand_targets_are_registered() {
    let planted = [("crates/x", "slow", "why")];
    let declared = vec![("crates/x".to_string(), "slow".to_string()), ("crates/y".to_string(), "hidden".to_string())];
    let offences = OnDemandRegistry { entries: &planted }.unregistered(&declared);
    assert_eq!(offences.len(), 1, "{offences:?}");
    assert!(offences[0].contains("crates/y") && offences[0].contains("hidden"), "{offences:?}");

    let offences = OnDemandRegistry { entries: &ON_DEMAND_TARGETS }.unregistered(&OnDemandManifest::declared());
    assert!(offences.is_empty(), "targets switched off by `test = false` without a registry entry:\n  {}", offences.join("\n  "));
}

/// Every registry entry names a target that exists and is flagged; a planted stale entry is reported.
#[test]
fn the_on_demand_registry_names_only_on_demand_targets() {
    let planted = [("crates/x", "gone", "why")];
    let declared = vec![("crates/x".to_string(), "slow".to_string())];
    let offences = OnDemandRegistry { entries: &planted }.stale(&declared);
    assert_eq!(offences.len(), 1, "{offences:?}");
    assert!(offences[0].contains("crates/x") && offences[0].contains("gone"), "{offences:?}");

    let offences = OnDemandRegistry { entries: &ON_DEMAND_TARGETS }.stale(&OnDemandManifest::declared());
    assert!(offences.is_empty(), "registry entries with no flagged target:\n  {}", offences.join("\n  "));
}

/// The registry holds exactly the twelve targets of `integrated-tests` D3: the ones that simulate
/// `examples/buck`. A thirteenth entry is a decision, so it moves this set with it.
#[test]
fn the_on_demand_targets_are_the_buck_set() {
    let mut registered: Vec<String> = ON_DEMAND_TARGETS.iter().map(|(dir, target, _)| format!("{dir}:{target}")).collect();
    registered.sort();
    let mut expected: Vec<String> = [
        "crates/piperine-parity:buck_whole",
        "crates/piperine-parity:buck_dpwm",
        "crates/piperine-parity:buck_control",
        "crates/piperine-parity:buck_boundary",
        "crates/piperine-parity:buck_power",
        "crates/piperine-parity:buck_supervisor",
        "crates/piperine-parity:buck_closed_loop",
        "crates/piperine-api:buck_long",
        "crates/piperine-cli:buck",
        "crates/piperine-cli:book_buck",
        ".:buck_scenarios",
        ".:book_buck",
    ]
    .iter()
    .map(|entry| (*entry).to_string())
    .collect();
    expected.sort();
    assert_eq!(registered, expected, "the on-demand set is the buck set of D3 (MD-77)");
}

/// The buck README's "On demand" block names each registered target as `--test <name>`; a planted
/// block that omits one is reported by target.
#[test]
fn the_buck_readme_names_every_on_demand_target() {
    let planted = [("crates/x", "slow", "why")];
    let offences = OnDemandRegistry { entries: &planted }.undocumented("# t\n\n## On demand\n\ncargo test --test fast\n\n## Next\n--test slow\n");
    assert_eq!(offences.len(), 1, "{offences:?}");
    assert!(offences[0].contains("slow"), "{offences:?}");

    let prefix = [("crates/x", "buck", "why")];
    let offences = OnDemandRegistry { entries: &prefix }.undocumented("## On demand\n\ncargo test -p x --test buck_whole\n");
    assert_eq!(offences.len(), 1, "a longer target name is not the prefix target: {offences:?}");
    let whole = [("crates/x", "buck_whole", "why")];
    let offences = OnDemandRegistry { entries: &whole }.undocumented("## On demand\n\ncargo test -p x --test buck --test buck_whole\n");
    assert!(offences.is_empty(), "{offences:?}");
    let twins = [("crates/a", "book_buck", "why"), ("crates/b", "book_buck", "why")];
    let offences = OnDemandRegistry { entries: &twins }.undocumented("## On demand\n\ncargo test -p a --test book_buck\n");
    assert_eq!(offences.len(), 1, "the other package's line is missing: {offences:?}");
    assert!(offences[0].contains("-p b"), "{offences:?}");

    let readme = fs::read_to_string(repo_root().join("examples/buck/README.md")).expect("the buck README is readable");
    let offences = OnDemandRegistry { entries: &ON_DEMAND_TARGETS }.undocumented(&readme);
    assert!(offences.is_empty(), "on-demand targets the buck README does not name:\n  {}", offences.join("\n  "));
}

// ─── 4a. LRM-COVERS declarations are well-formed (T6, lrm-conformance-wave2) ──

/// Whether one `//! LRM-COVERS:` line is malformed: not the canonical
/// comma-separated list of bare LRM-2-4 section ids
/// (`tests/lrm_section_coverage.rs::lrm_sections`'s own id shape — digit
/// groups, or a single `A`..`H` letter followed by digit groups, joined by
/// `.`). Declaring at all is optional; a line that does declare must be
/// parseable by the coverage gate, so `§4.5.3` (a section symbol, not the
/// gate's own id spelling), `4.5.3-4.5.5` (a range — the gate has no
/// concept of one, every covered id must be named), and an empty list all
/// fail here rather than silently scraping as zero declared ids.
fn is_malformed_lrm_covers(line: &str) -> bool {
    let Some(rest) = line.trim_start().strip_prefix("//! LRM-COVERS:") else {
        return false;
    };
    let rest = rest.trim();
    if rest.is_empty() {
        return true;
    }
    let ids: Vec<&str> = rest.split(',').map(str::trim).collect();
    if ids.iter().any(|id| id.is_empty()) {
        return true;
    }
    for id in &ids {
        if id.contains('§') || id.contains('-') || id.contains(char::is_whitespace) {
            return true;
        }
        let segments: Vec<&str> = id.split('.').collect();
        let Some(first) = segments.first() else { return true };
        let digit_group = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
        let numbered_head = digit_group(first);
        let annex_head = first.len() == 1 && matches!(first.chars().next(), Some('A'..='H'));
        if !(numbered_head || annex_head) {
            return true;
        }
        if segments[1..].iter().any(|seg| !digit_group(seg)) {
            return true;
        }
    }
    false
}

/// Declaring `LRM-COVERS` is optional (T6's own rule); a declaration that
/// exists must be well-formed, because `lrm_section_coverage.rs`'s gate
/// derives the whole matrix from parsing exactly this shape.
#[test]
fn lrm_covers_declarations_are_well_formed() {
    let mut offences = Vec::new();
    for (path, text) in rust_sources() {
        for (index, line) in text.lines().enumerate() {
            if is_malformed_lrm_covers(line) {
                offences.push(format!("{path}:{}: {}", index + 1, line.trim()));
            }
        }
    }
    assert!(
        offences.is_empty(),
        "malformed LRM-COVERS declaration (must be a bare comma-separated id list — \
         no `§`, no ranges, not empty):\n  {}",
        offences.join("\n  ")
    );
}

// ─── 5. No file-scope lint suppression (MD-33) ────────────────────────────────

/// Whether one source line switches a lint off for a whole file. A file-scope
/// `#![allow(…)]` is invisible from the item it excuses: P6 found twelve of
/// them hiding 22 dead items across `piperine-solver` and `piperine-codegen`,
/// four of which were traits describing an analysis contract the solver never
/// built. An item that truly has no consumer yet says so *at the item*, with a
/// one-line reason a reader can check.
fn is_file_scope_allow(line: &str) -> bool {
    line.trim_start().starts_with("#![allow(")
}

#[test]
fn no_file_scope_lint_suppression() {
    let mut offences = Vec::new();
    for (path, text) in library_sources() {
        for (index, line) in text.lines().enumerate() {
            if is_file_scope_allow(line) {
                offences.push(format!("{path}:{}: {}", index + 1, line.trim()));
            }
        }
    }
    assert!(
        offences.is_empty(),
        "file-scope lint suppression (MD-33 — move the exemption onto the item \
         it excuses and give it a one-line reason, or delete the item):\n  {}",
        offences.join("\n  ")
    );
}

// ─── 6. No dead-architecture identifiers (MD-35) ──────────────────────────────

/// The names of the deleted IR layer. There is no `IrProgram`/`IrModule`/
/// `IrExpr`/`IrInstance` type: codegen resolves a POM `Design`/`Module`/
/// `Instance` directly. P6 found sixteen comments defining the code by that
/// dead architecture — two of them broken intra-doc links. A comment that
/// can only be checked against a deleted type cannot be checked at all.
///
/// Note: `piperine-ir` and `piperine_ir` were on this list under MD-35
/// before MD-40 renamed `piperine-signal` to `piperine-ir`; they are
/// LIVE identifiers again (the `msl-ir` feature's object-model crate, a
/// leaf with no in-tree dependency) and are no longer banned.
const DEAD_ARCHITECTURE_IDENTIFIERS: [&str; 4] =
    ["IrProgram", "IrModule", "IrExpr", "IrInstance"];

/// Which dead identifiers one source line mentions.
fn dead_architecture_identifiers_in(line: &str) -> Vec<&'static str> {
    DEAD_ARCHITECTURE_IDENTIFIERS.into_iter().filter(|id| line.contains(id)).collect()
}

/// The registry of deliberate historical notes: file → (identifier, why it
/// survives). Empty today: the rename in MD-40 made every former
/// `piperine-ir`-as-dead-architecture mention either live again or stale,
/// and the stale ones were reworded. Registry + exhaustiveness (the
/// `capabilities_contract.rs` shape): a new note fails until it is recorded,
/// and a note that disappears fails just as loudly.
fn registered_historical_note(_path: &str) -> Option<(&'static str, &'static str)> {
    None
}

#[test]
fn no_dead_architecture_identifiers() {
    let mut offences = Vec::new();
    let mut notes_seen = Vec::new();
    for (path, text) in library_sources() {
        for (index, line) in text.lines().enumerate() {
            for id in dead_architecture_identifiers_in(line) {
                match registered_historical_note(&path) {
                    Some((allowed, _)) if allowed == id => notes_seen.push(path.clone()),
                    _ => offences.push(format!("{path}:{}: `{id}`", index + 1)),
                }
            }
        }
    }
    assert!(
        offences.is_empty(),
        "dead-architecture identifiers (MD-35 — say what the code does now; the \
         `Ir*` types no longer exist, codegen resolves the POM \
         `Design`/`Module`/`Instance` directly):\n  {}",
        offences.join("\n  ")
    );
    assert!(
        notes_seen.is_empty(),
        "the registered-historical-note set is no longer empty — a new entry \
         appeared; re-add it to `registered_historical_note` if deliberate: \
         {notes_seen:?}"
    );
}

// ─── 7. `mod.rs` declares, never implements (MD-34) ───────────────────────────

/// MD-13 rule 4's golden rule: a file is named after what it holds. `mod.rs` is
/// named after nothing — it is the module's table of contents, so it may hold
/// declarations, re-exports and the layer's `//!` contract, and nothing else.
/// P6 found four in `piperine-codegen` holding 1237/864/861/555 lines of
/// implementation: `AnalogInstance`, `AnalogKernel`, `CompiledModule` +
/// `PiperineDevice`, and `LoweredBody` all lived in a file whose name a reader
/// cannot grep for.
///
/// The ceiling is a line count because that is what a scan can check; sixty
/// lines is enough for a declaration list and a real contract, and too few for
/// a type with an implementation.
const MOD_RS_MAX_LINES: usize = 60;

/// The exemption marker (design D8). It sits **inside** the `mod.rs` it
/// excuses, carrying the reason where the debt is, so a reader who opens the
/// oversized file learns why before they wonder. The scan counts exemptions and
/// the registry below names them: debt stays visible instead of dissolving.
fn is_hygiene_exemption(line: &str) -> bool {
    line.trim_start().starts_with("// hygiene-exempt:")
}

/// Every `mod.rs` under a crate's `src` tree, as `(path, line count)`.
fn mod_rs_files() -> Vec<(String, usize, String)> {
    let files: Vec<(String, usize, String)> = library_sources()
        .into_iter()
        .filter(|(path, _)| path.ends_with("/mod.rs"))
        .map(|(path, text)| {
            let lines = text.lines().count();
            (path, lines, text)
        })
        .collect();
    // msl-cli T4 deleted the old `piperine-api` (five `mod.rs`), so the floor is 10; the walk
    // must also reach the three registered oversized files, which a blind walk would miss.
    assert!(
        files.len() > 10,
        "the walk found only {} `mod.rs` files — the layout moved and this guard is blind",
        files.len()
    );
    for known in ["crates/piperine-lang/src/pom/resolve/mod.rs", "crates/piperine-lang/src/pom/reflect/mod.rs", "crates/piperine-lang/src/pom/lower/mod.rs"] {
        assert!(files.iter().any(|(path, _, _)| path == known), "the walk does not reach {known}: this guard is blind");
    }
    files
}

/// The registry of `mod.rs` files allowed to exceed the ceiling: path → why the
/// split is not this feature's work. Registry + exhaustiveness (the
/// `capabilities_contract.rs` shape used by the other guards here): a new
/// exemption fails until it is recorded, and an exemption that becomes
/// unnecessary fails just as loudly, so the list cannot rot into a blanket.
///
/// Two remain — the parse-tree entries left with the PHDL parser (the
/// gate was dark from T21 until T37 and could not prune them), and the
/// VAMS-era splits (catalog, ast) shrank their `mod.rs` back under the
/// ceiling. The in-file `// hygiene-exempt:` line carries the per-file
/// reason for the ones that remain.
fn registered_oversized_mod_rs(path: &str) -> Option<&'static str> {
    Some(match path {
        // Follow-up (ROADMAP.md, `code-clarity`): refactor these three into named modules.
        "crates/piperine-lang/src/pom/resolve/mod.rs" => "name-resolution stages; split pending",
        "crates/piperine-lang/src/pom/reflect/mod.rs" => "POM reflection surface; split pending",
        "crates/piperine-lang/src/pom/lower/mod.rs" => "lowering driver; split pending",
        _ => return None,
    })
}

#[test]
fn mod_rs_declares_only() {
    let mut offences = Vec::new();
    let mut exempted = Vec::new();
    for (path, lines, text) in mod_rs_files() {
        if lines <= MOD_RS_MAX_LINES {
            assert!(
                registered_oversized_mod_rs(&path).is_none(),
                "{path} is {lines} lines — within the {MOD_RS_MAX_LINES}-line ceiling, so its \
                 exemption is stale. Remove the `// hygiene-exempt:` line and its registry entry."
            );
            continue;
        }
        if text.lines().any(is_hygiene_exemption) && registered_oversized_mod_rs(&path).is_some() {
            exempted.push(format!("{path} ({lines} lines)"));
        } else {
            offences.push(format!("{path}: {lines} lines (ceiling {MOD_RS_MAX_LINES})"));
        }
    }
    assert!(
        offences.is_empty(),
        "`mod.rs` files holding implementation (MD-34 — a `mod.rs` declares its \
         modules, re-exports their surface and states the layer's contract; move \
         the types into files named after them, or add a `// hygiene-exempt: \
         <reason>` line and register the file in this test):\n  {}",
        offences.join("\n  ")
    );
    assert_eq!(
        exempted.len(),
        3,
        "the exempted set changed — update the registry and the count: {exempted:?}"
    );
}

// ─── 8. The root test-naming rule (MD-28 · CLA-13) ────────────────────────────

/// A root target claiming to prove the host surface says so in its name.
const HOST_TARGET_PREFIX: &str = "host_";

/// Every non-root workspace crate, spelled as Rust source spells it. Order does
/// not matter — [`workspace_crates_used`] matches on `<ident>::`, so
/// `piperine_lang_server::` never counts as `piperine_lang`.
const WORKSPACE_CRATE_IDENTS: [&str; 10] = [
    "piperine_api",
    "piperine_cli",
    "piperine_codegen",
    "piperine_lang",
    "piperine_lang_server",
    "piperine_plugin",
    "piperine_plugin_macros",
    "piperine_project",
    "piperine_python",
    "piperine_solver",
];

/// Whether a target reaches the host surface: the root shell's re-export face
/// (`piperine::…`, MD-20) or `piperine-api` underneath it.
fn proves_host_surface(text: &str) -> bool {
    text.contains("piperine::") || text.contains("piperine_api::")
}

/// Which workspace crates a target names. A crate counts when the target
/// addresses it by path (`piperine_solver::…`) or imports it whole
/// (`use piperine_solver;`) — the two ways a target can depend on one.
fn workspace_crates_used(text: &str) -> Vec<&'static str> {
    WORKSPACE_CRATE_IDENTS
        .into_iter()
        .filter(|id| text.contains(&format!("{id}::")) || text.contains(&format!("use {id};")))
        .collect()
}

/// The registry of root targets that prove neither the host surface nor a
/// cross-crate seam: file → why it still belongs at the root. Registry +
/// exhaustiveness (the shape the guards above use): an unregistered offender
/// fails, and a registered entry that vanishes or stops needing its exemption
/// fails just as loudly.
///
/// Registered now: `book/claims_guard.rs`, the Book claim guard (feature `root-e2e`, RE-03), the same text-only
/// policy proof over the Book's sentences; `book/inventory_guard.rs`, which is the Book inventory guard (feature `root-e2e`, RE-01):
/// the Book's blocks against `tests/book/inventory.tsv`, a text-only policy proof. Registered before it: `ngspice_parity_matrix.rs` is the ngspice model × domain
/// coverage registry (NGP-15/MD-31): a repo-policy proof in the
/// `capabilities_contract.rs` shape — a data table plus a filesystem walk of
/// `tests/e2e/twins/` and the golden targets — so it links no workspace crate,
/// the same standing this file has (skipped as [`SELF`]).
/// `lrm_conformance_matrix.rs` is the lrm-conformance-closure LRMC-ID
/// registry (MD-31, T9): a data table plus `include_str!`-parsing of
/// `.specs/features/lrm-conformance-closure/spec.md` and a filesystem read
/// of each closing test's file — same repo-policy-proof standing, no
/// workspace crate linked. `lrm_section_coverage.rs` (T4/T5,
/// lrm-conformance-wave2) is the same shape one wave over: a `VAMS-LRM-2-4.md`
/// heading parse + `coverage_exclusions.toml` read + `//! LRM-COVERS:`
/// source scrape, no workspace crate linked either. An entry added here must move the expected
/// set in [`root_targets_match_the_naming_rule`] with it, so the exemption
/// cannot be granted quietly.
const ROOT_TARGET_EXEMPTIONS: [(&str, &str); 2] = [
    (
        "book/inventory_guard.rs",
        "the Book inventory guard (root-e2e T12): a repo-policy proof over `docs/book/content.html` and \
         `tests/book/inventory.tsv`, so it links no workspace crate",
    ),
    (
        "book/claims_guard.rs",
        "the Book claim guard (root-e2e T14): a repo-policy proof over the sentences of \
         `docs/book/content.html` and `tests/book/inventory.tsv`, so it links no workspace crate",
    ),
];

fn registered_root_exemption(file: &str) -> Option<&'static str> {
    ROOT_TARGET_EXEMPTIONS.iter().find(|(name, _)| *name == file).map(|(_, why)| *why)
}

/// Why the offender fails the rule, in the words a reader needs to fix it.
fn root_naming_offence(file: &str, text: &str) -> Option<String> {
    let crates = workspace_crates_used(text);
    if file.starts_with(HOST_TARGET_PREFIX) {
        if proves_host_surface(text) {
            return None;
        }
        return Some(format!(
            "tests/{file}: named `{HOST_TARGET_PREFIX}*` but never reaches the host \
             surface (no `piperine::` or `piperine_api::`) — prove the host API here, \
             or rename it to `<feature>.rs`"
        ));
    }
    if proves_host_surface(text) || crates.len() >= 2 {
        return None;
    }
    Some(match crates.as_slice() {
        [only] => format!(
            "tests/{file}: exercises `{only}` alone — move it to \
             `crates/{}/tests/` (MD-28: a target lives in the crate it exercises)",
            only.replace('_', "-")
        ),
        _ => format!(
            "tests/{file}: reaches no workspace crate and no host surface — root \
             `tests/` is for the shell's proofs; say what it proves or move it"
        ),
    })
}

#[test]
fn root_targets_match_the_naming_rule() {
    let mut offences = Vec::new();
    let mut exempted = Vec::new();
    for (file, text) in root_targets() {
        let offence = root_naming_offence(&file, &text);
        match registered_root_exemption(&file) {
            Some(_) => {
                assert!(
                    offence.is_some(),
                    "tests/{file} now satisfies the root naming rule, so its exemption \
                     is stale — remove its entry from `registered_root_exemption`"
                );
                exempted.push(file);
            }
            None => offences.extend(offence),
        }
    }
    assert!(
        offences.is_empty(),
        "root targets breaking the naming rule (MD-28 — `host_*.rs` proves the host \
         surface, `<feature>.rs` proves the shell or a cross-crate seam; anything \
         exercising one crate alone lives in that crate):\n  {}",
        offences.join("\n  ")
    );
    let expected: Vec<String> =
        ROOT_TARGET_EXEMPTIONS.iter().map(|(name, _)| (*name).to_string()).collect();
    assert_eq!(
        exempted, expected,
        "the exempted root-target set changed — record the exemption's reason in \
         `ROOT_TARGET_EXEMPTIONS` and move this assertion's expected set with it"
    );
}

// ─── `.phdl` monotonic-shrink gate (VAMS-11, MD-31, T39) ──────────────────────

/// The committed high-water mark for straggling PHDL/PPR source files during
/// the Verilog-AMS migration (`.specs/features/verilog-ams-support/design.md`
/// §7: "a dual-frontend period allowed S2-S4, bounded by a monotonic-shrink
/// gate" — MD-31, the invariant lives in a test, not a promise). Set at T39
/// (2026-08-13) to the count measured in the tree the day this gate landed —
/// `piperine-lang`'s own PHDL *parser* is already gone (T21/T38); this counts
/// the source *corpus* Phase 8 (T40-T46) still has to migrate one file at a
/// time, deleting each `.phdl`/`.ppr` original in the same commit that adds
/// its `.vams`/`.va` twin (VAMS-11's own requirement — "no `.phdl` file
/// survives past its own migration task").
///
/// T46 brought the corpus to exactly zero — the mark is now `0` and the
/// assertion below is exact-equality (VAMS-11's own closing requirement).
/// No `.phdl`/`.ppr` file may ever exist in this repository again.
const PHDL_HIGH_WATER_MARK: usize = 0;

/// Every `.phdl`/`.ppr` file anywhere in the repository (recursive, skipping
/// `target`/`.git`) — a plain filesystem walk over every file extension, not
/// a `.rs`-source scan like [`rust_sources`]: the files this gate counts are
/// PHDL source, not Rust.
fn phdl_files() -> Vec<PathBuf> {
    let root = repo_root();
    let mut out = Vec::new();
    collect_phdl(&root, &mut out);
    out
}

fn collect_phdl(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if path.is_dir() {
            if name != "target" && name != ".git" {
                collect_phdl(&path, out);
            }
        } else if path.extension().is_some_and(|e| e == "phdl" || e == "ppr") {
            out.push(path);
        }
    }
}

#[test]
fn phdl_corpus_does_not_grow() {
    let root = repo_root();
    let files = phdl_files();
    let count = files.len();
    assert_eq!(
        count, PHDL_HIGH_WATER_MARK,
        "`.phdl`/`.ppr` file count is {count}, expected exactly {PHDL_HIGH_WATER_MARK} \
         (VAMS-11) — PHDL is fully retired (MD-44); no `.phdl`/`.ppr` file may exist in this \
         repository. Files found:\n  {}",
        files
            .iter()
            .map(|p| p.strip_prefix(&root).unwrap_or(p).display().to_string())
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

// ---- value semantics: no source of the tree writes a removed form (VS-09) -------------------

/// The forms the amended language removed (Appendix G.71), found in the text of an MSL source: `let mut`, a
/// `&` or `*` that starts an operand, `Clone` or `Copy` in a derive, and a `'static` lifetime.
struct RemovedForms;

impl RemovedForms {
    /// The characters after which an operator is a prefix operator: the start of an operand.
    const OPERAND_START: [char; 6] = ['(', '[', ',', '=', '{', ':'];

    /// The line without its `//` comment and without the text of its string literals.
    fn code_of(line: &str) -> String {
        let mut code = String::new();
        let mut in_string = false;
        let mut previous = ' ';
        for c in line.chars() {
            if !in_string && c == '/' && previous == '/' {
                code.pop();
                break;
            }
            if c == '"' && previous != '\\' {
                in_string = !in_string;
            } else if !in_string {
                code.push(c);
            }
            previous = c;
        }
        code
    }

    /// Whether `code` has a `&` or a `*` where an operand starts (a borrow or a dereference, not the binary operator).
    fn has_prefix_operator(code: &str) -> bool {
        let chars: Vec<char> = code.chars().collect();
        (0..chars.len()).any(|at| {
            let c = chars[at];
            if c != '&' && c != '*' {
                return false;
            }
            let after = chars.get(at + 1).copied();
            let glued = after == Some('=') || (c == '&' && (after == Some('&') || (at > 0 && chars[at - 1] == '&')));
            let before: String = chars[..at].iter().collect::<String>().trim_end().to_string();
            let starts = before.is_empty() || before.ends_with("->") || before.chars().last().is_some_and(|b| Self::OPERAND_START.contains(&b));
            let operand = after.is_some_and(|a| a.is_alphanumeric() || matches!(a, '_' | '(' | '[' | '\''));
            !glued && starts && operand
        })
    }

    /// The removed forms a line of MSL writes, by name.
    fn in_line(line: &str) -> Vec<&'static str> {
        let code = Self::code_of(line);
        let mut found = Vec::new();
        if code.contains("let mut ") {
            found.push("let mut");
        }
        if code.contains("derive(") && code.split(|c: char| !c.is_alphanumeric() && c != '_').any(|word| word == "Clone" || word == "Copy") {
            found.push("Clone or Copy in a derive");
        }
        if code.contains("'static") {
            found.push("'static");
        }
        if Self::has_prefix_operator(&code) {
            found.push("a prefix `&` or `*`");
        }
        found
    }

    /// Every `.msl` file of `std/`, `examples/` and the `tests/` trees of the crates, recursively.
    fn msl_files() -> Vec<PathBuf> {
        let root = repo_root();
        let mut files = Vec::new();
        Self::collect(&root.join("examples"), &mut files);
        if let Ok(crates) = fs::read_dir(root.join("crates")) {
            for krate in crates.flatten() {
                Self::collect(&krate.path().join("std"), &mut files);
                Self::collect(&krate.path().join("tests"), &mut files);
            }
        }
        files
    }

    /// Appends the `.msl` files under `dir` to `out`.
    fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                Self::collect(&path, out);
            } else if path.extension().is_some_and(|e| e == "msl") {
                out.push(path);
            }
        }
    }
}

#[test]
fn no_msl_file_uses_a_removed_form() {
    // VS-09 AC 1: the tree is written in the amended language, so the parser can refuse the earlier one.
    let files = RemovedForms::msl_files();
    assert!(files.len() > 40, "the walk found only {} `.msl` files: the layout moved and the guard is blind", files.len());
    let mut offences = Vec::new();
    for file in &files {
        let text = fs::read_to_string(file).expect("an `.msl` file");
        for (number, line) in text.lines().enumerate() {
            for form in RemovedForms::in_line(line) {
                offences.push(format!("{}:{}: {form}: {}", file.display(), number + 1, line.trim()));
            }
        }
    }
    assert!(offences.is_empty(), "removed forms in the tree:\n  {}", offences.join("\n  "));
}

#[test]
fn the_removed_form_walk_fails_on_a_planted_line() {
    // The detector finds each removed form and passes the neighbouring forms that stay (binary `&`, `&&`, `*`).
    for (line, form) in [
        ("fn f() { let mut x = 1.0; }", "let mut"),
        ("#[derive(Repr, Clone, Copy)]", "Clone or Copy in a derive"),
        ("fn f(s: &'static str) {}", "'static"),
        ("let y = f(&x);", "a prefix `&` or `*`"),
        ("let y = *x + 1;", "a prefix `&` or `*`"),
        ("fn f(xs: &[f64]) -> f64 { 0.0 }", "a prefix `&` or `*`"),
    ] {
        assert!(RemovedForms::in_line(line).contains(&form), "`{line}` is not found as {form}");
    }
    for line in ["let y = a & -b;", "let y = a && !b;", "let y = a * (b);", "n &= 7; m *= 2;", "// let mut x = &y;", "let s = \"let mut &x\";", "#[derive(Repr, PartialEq)]"] {
        assert!(RemovedForms::in_line(line).is_empty(), "`{line}` writes no removed form: {:?}", RemovedForms::in_line(line));
    }
}

/// The explicit model of unknowns (explicit-unknowns, EU-20): which `.msl` sources may still spell a removed name,
/// and which test replaced each test that pinned an unknown behavior.
struct RemovedUnknownForms;

impl RemovedUnknownForms {
    /// The spellings of the earlier flag: `Unknown`, `known_or`, `is_known`, `.known()`, `.high()`, `.level(` and `merge(`.
    const NAMES: [&'static str; 7] = ["Unknown", "known_or", "is_known", ".known()", ".high()", ".level(", "merge("];

    /// The `.msl` files of std, the examples and every test tree, as `(repo-relative path, text)`.
    fn sources() -> Vec<(String, String)> {
        let root = repo_root();
        let mut found = Vec::new();
        for base in ["crates/piperine-lang/std", "examples", "tests", "crates"] {
            Self::collect(&root.join(base), &root, &mut found);
        }
        found
    }

    /// Every `.msl` under `dir`, skipping build output.
    fn collect(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.file_name().is_some_and(|name| name == "target") {
                continue;
            }
            if path.is_dir() {
                Self::collect(&path, root, out);
            } else if path.extension().is_some_and(|ext| ext == "msl") {
                let relative = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
                if relative.contains("/fixtures/preformat/") || (relative.starts_with("crates/") && !relative.contains("/tests/") && !relative.contains("/std/")) {
                    continue;
                }
                if let Ok(text) = fs::read_to_string(&path) {
                    out.push((relative, text));
                }
            }
        }
    }

    /// The removed names a source spells, outside its `//` comments.
    fn spelled(text: &str) -> Vec<&'static str> {
        let code: String = text.lines().map(|line| line.split("//").next().unwrap_or("")).collect::<Vec<_>>().join("\n");
        Self::NAMES.iter().copied().filter(|name| code.contains(name)).collect()
    }

    /// Each test of the explicit-unknowns range that pinned an unknown, a flag, a merge or a known byte, the test that supersedes it, and the task that writes it when it does
    /// not exist yet (`None`: it exists).
    const SUCCESSORS: [(&'static str, &'static str, Option<&'static str>); 124] = [
        ("a_function_and_a_definition_merge_an_unknown_if", "control_on_a_carrier_runs_one_path", None),
        ("an_unknown_if_runs_both_arms_without_a_fault", "control_on_a_carrier_runs_one_path", None),
        ("both_arms_committing_commit_their_merge", "control_on_a_carrier_runs_one_path", None),
        ("equal_known_commits_stay_known", "control_on_a_carrier_runs_one_path", None),
        ("one_arm_committing_merges_with_the_value_then", "control_on_a_carrier_runs_one_path", None),
        ("an_if_expression_and_an_if_statement_commit_the_same", "control_on_a_carrier_runs_one_path", None),
        ("an_unknown_match_merges_every_arm", "control_on_a_carrier_runs_one_path", None),
        ("a_nested_one_armed_commit_merges_with_the_path_before_it", "control_on_a_carrier_runs_one_path", None),
        ("a_merge_commit_meets_another_reactions_commit_of_the_same_microstep", "control_on_a_carrier_runs_one_path", None),
        ("a_panic_under_unknown_control_makes_the_merge_unknown", "control_on_a_carrier_runs_one_path", None),
        ("definitions_lets_and_functions_merge_alike", "control_on_a_carrier_runs_one_path", None),
        ("an_unknown_delay_under_unknown_control_commits_unknown_next", "control_on_a_carrier_runs_one_path", None),
        ("neither_arm_committing_commits_nothing", "control_on_a_carrier_runs_one_path", None),
        ("delays_one_tick_apart_are_one_key", "control_on_a_carrier_runs_one_path", None),
        ("the_pfd3_reaction_keeps_state_unknown_until_a_known_commit", "control_on_a_carrier_runs_one_path", None),
        ("an_if_whose_arms_commit_is_the_value_of_a_commit_and_of_an_emit_field", "control_on_a_carrier_runs_one_path", None),
        ("a_maybe_finish_does_not_end_the_run", "a_finish_ends_the_run_where_its_guard_holds", None),
        ("a_record_under_unknown_control_is_maybe", "an_emit_under_a_guard_is_a_plain_record", None),
        ("a_record_field_keeps_its_flag", "an_emit_under_a_guard_is_a_plain_record", None),
        ("a_maybe_check_is_judged_as_unknown", "an_emit_under_a_guard_is_a_plain_record", None),
        ("an_unknown_guard_runs_the_reaction_as_a_maybe", "a_false_guard_runs_nothing", None),
        ("two_commits_under_an_unknown_guard_merge_exactly", "a_false_guard_runs_nothing", None),
        ("a_known_false_guard_does_not_run", "a_false_guard_runs_nothing", None),
        ("an_unknown_if_in_a_cross_argument_merges", "an_if_in_a_cross_argument_picks_its_arm", None),
        ("an_unknown_initial_value_starts_every_level_unknown", "a_carrier_without_a_value_starts_at_its_default", None),
        ("an_unknown_initial_value_fires_no_change_at_zero", "the_first_commit_is_a_change", None),
        ("a_const_holding_unknown_reads_unknown_everywhere", "a_binary_carrier_is_read_as_its_value", None),
        ("committing_unknown_makes_every_level_unknown", "a_written_initial_value_is_the_value_at_the_start", None),
        ("short_circuit_operands_follow_the_merge", "short_circuit_operands_decide_as_bools", None),
        ("a_math_function_of_an_unknown_value_is_unknown", "a_math_function_of_a_value_is_a_value", None),
        ("known_or_with_an_unknown_default_gives_that_default", "a_math_function_of_a_value_is_a_value", None),
        ("a_function_that_emits_runs_in_a_value_position_of_a_reaction", "an_emitting_function_runs_in_a_value_position_of_a_reaction", None),
        ("level_reads_each_level_of_a_known_value", "at_gives_the_state_of_one_level", Some("T46")),
        ("level_with_a_known_index_is_known", "at_gives_the_state_of_one_level", Some("T46")),
        ("level_out_of_range_is_a_panic_naming_k", "at_gives_the_state_of_one_level", Some("T46")),
        ("level_with_an_unknown_index_merges_every_level", "at_gives_the_state_of_one_level", Some("T46")),
        ("merge_of_equal_known_values_is_that_value", "and_follows_the_four_state_table", Some("T46")),
        ("merge_of_different_values_is_unknown_and_commutative", "or_follows_the_four_state_table", Some("T46")),
        ("merge_of_a_value_with_itself_is_that_value", "same_compares_the_four_states", Some("T46")),
        ("same_compares_flags_and_values", "same_compares_the_four_states", Some("T46")),
        ("cast_keeps_each_level_and_its_flag", "a_binary_value_converts_to_logic", Some("T46")),
        ("cast_to_a_tag_with_no_variant_is_unknown", "a_logic_converts_to_an_option_of_bits", Some("T46")),
        ("logic_builds_known_and_unknown_values", "logic_reads_a_four_state_pattern", Some("T45")),
        ("matches_bits_follows_its_table", "logic_reads_a_four_state_pattern", Some("T45")),
        ("the_library_items_that_took_borrows_give_the_same_results", "logic_and_matches_bits_give_the_table_values", Some("T45")),
        ("a_bad_pattern_is_e0702", "logic_reads_a_four_state_pattern", Some("T45")),
        ("a_flag_total_function_may_take_a_carrier_in_analog", "a_function_that_reads_its_parameter_takes_a_carrier_in_analog", None),
        ("a_function_that_reads_its_parameter_bare_is_not_flag_total", "a_function_that_reads_its_parameter_takes_a_carrier_in_analog", None),
        ("level_is_typed_on_every_repr_type", "at_gives_the_state_of_one_level", Some("T46")),
        ("level_on_a_value_without_repr_is_refused", "the_flag_methods_do_not_resolve", None),
        ("unknown_with_no_inferable_type_is_e0906", "unknown_is_a_name_that_does_not_resolve", None),
        ("a_carrier_read_is_flagged_and_the_flag_reads_are_not", "no_value_type_is_flagged", None),
        ("a_function_gets_one_variant_per_flag_pattern", "control_on_a_carrier_runs_one_path", None),
        ("a_partial_right_operand_under_a_flagged_short_circuit_is_an_if", "short_circuit_operands_decide_as_bools", None),
        ("a_short_circuit_over_a_flagged_operand_becomes_bitwise", "short_circuit_operands_decide_as_bools", None),
        ("a_commit_with_flags_reaches_the_owner", "a_commit_reaches_the_owner", None),
        ("a_host_without_commit_with_is_a_named_fault", "the_host_has_one_commit", None),
        ("a_digital_if_on_an_unknown_value_runs_both_arms_and_merges_what_they_commit", "a_digital_if_runs_the_arm_its_condition_selects_and_only_that_arm_commits", None),
        ("a_function_value_called_with_an_unknown_is_the_flagged_variant_or_p0001", "function_values_and_recursion_compile_and_run", None),
        ("a_known_or_over_an_unwritten_input_gives_the_default", "a_carrier_without_a_value_starts_at_its_default", None),
        ("an_unknown_condition_merges_both_arms", "an_if_commits_through_the_arm_it_takes", None),
        ("an_unknown_delay_commits_unknown_at_the_next_microstep", "a_commit_is_made_after_the_delay_it_names", None),
        ("level_merge_same_and_recast_compute_their_tables", "same_cast_and_math_compute_their_tables", None),
        ("a_maybe_commit_converts_the_pending_it_would_remove", "a_plain_commit_removes_the_pending_it_transports", None),
        ("a_maybe_replace_converts_every_pending_commit_of_the_statement", "a_replace_commit_removes_every_pending_commit_of_its_statement", None),
        ("a_rewind_undoes_a_conversion", "a_rewind_restores_a_removal", None),
        ("a_merge_commit_equal_to_the_value_fires_no_change", "a_commit_equal_to_the_value_fires_no_change", None),
        ("a_merge_commit_merges_with_the_value_then", "the_last_plain_commit_of_a_microstep_is_the_value", None),
        ("a_merge_commit_the_layout_cannot_merge_is_a_run_error_naming_the_net", "an_instant_that_never_goes_quiet_is_e0501_naming_the_nets", None),
        ("a_moving_field_without_a_known_byte_fails", "a_moving_loop_is_e0511_not_a_value_made_up", None),
        ("a_moving_known_field_is_zeroed_and_observed", "a_moving_loop_is_e0511_not_a_value_made_up", None),
        ("a_non_canonical_write_is_a_fault_through_a_function_a_commit_and_the_host", "a_host_write_outside_its_net_fails_loud_and_changes_nothing", None),
        ("a_past_or_unknown_time_does_not_fire", "a_past_time_does_not_fire", None),
        ("a_reaction_loop_committing_after_an_unknown_decision_commits_maybe", "a_reaction_loop_commits_on_every_iteration_it_runs", None),
        ("unknown_decisions_merge_written_locals_and_loop_exits", "known_control_prints_plain_rust_and_runs", None),
        ("a_maybe_record_prints_maybe_and_a_known_one_does_not", "a_guarded_record_prints_when_its_guard_holds_and_no_record_carries_a_maybe", None),
        ("a_reaction_on_a_changed_unknown_emits_the_inputs", "a_reaction_on_a_changed_carrier_emits_the_inputs", None),
        ("e0150_an_unknown_carrier_is_read_with_high", "a_bit_in_an_if_is_read_as_its_value", None),
        ("e0150_an_unknown_switch_control_is_read_with_high", "a_bit_control_in_a_division_is_read_as_its_value", None),
        ("e0150_is_an_analog_read_of_a_digital_carrier", "an_analog_read_of_a_digital_carrier_is_no_error", None),
        ("e0150_is_found_on_the_read_of_the_carrier_in_the_worked_example", "the_read_of_the_carrier_in_the_worked_example_is_no_error", None),
        ("e0187_is_not_raised_for_a_signal_with_no_first_value_yet", "e0187_a_signal_with_no_first_value_and_no_default_is_refused", None),
        ("e0187_is_not_raised_yet", "e0187_a_signal_with_no_first_value_and_no_default_is_refused", None),
        ("without_an_initial_value_the_output_sits_at_zero_volts", "e0187_a_signal_with_no_first_value_and_no_default_is_refused", None),
        ("e0702_is_raised_for_a_logic_pattern_with_z", "logic_reads_a_four_state_pattern", Some("T45")),
        ("the_monitor_file_runs_its_window_monitor_and_refuses_its_option_and_logic", "the_monitor_file_runs_its_window_monitor_and_its_adder_and_refuses_logic", None),
        ("an_absent_drive_is_left_out_and_an_unknown_one_is_passed", "an_absent_drive_is_left_out_and_a_present_one_is_passed_whatever_its_value", None),
        ("an_unsettled_step_is_one_observation_naming_every_relax_net", "an_unsettled_loop_is_e0511_naming_its_drives", None),
        ("an_unsettled_loop_reaches_the_owner_once_per_step", "an_unsettled_loop_is_e0511_naming_its_drives", None),
        ("a_reaction_on_a_flagged_guard_reads_the_guard_and_fired", "a_guard_or_a_decision_adds_no_read_and_no_resolution_pin", None),
        ("a_reaction_with_an_unknown_decision_reads_the_resolution", "a_guard_or_a_decision_adds_no_read_and_no_resolution_pin", None),
        ("carriers_are_discrete_pins_with_a_known_byte", "carriers_are_discrete_pins_that_hold_the_value_only", None),
        ("every_record_layout_ends_with_maybe", "every_record_layout_holds_its_declared_fields_only", None),
        ("a_real_that_is_nan_merges_with_itself", "known_values_fold_by_the_level_rules", None),
        ("unknown_folds_by_the_level_rules", "known_values_fold_by_the_level_rules", None),
        ("analog_reading_a_maybe_unknown_value_is_e0150", "analog_has_no_refusal_of_a_flag", None),
        ("no_analog_refusal_of_an_unknown_is_p0001", "analog_has_no_refusal_of_a_flag", None),
        ("unknown_deciding_structure_is_e0151", "unknown_decides_no_structure_because_it_is_no_value", None),
        ("a_row_over_an_unknown_value_is_unknown", "f64_rows_equal_rusts_bit_for_bit_in_analog_and_in_a_reaction", None),
        ("a_tag_naming_no_variant_reads_unknown", "a_tag_the_cast_names_is_a_variant", None),
        ("an_unknown_composite_is_refused_by_name", "composite_carriers_initialize_define_commit_and_change", None),
        ("an_unknown_index_reads_and_writes_by_merge", "an_index_reads_and_writes_the_element_it_names", None),
        ("an_unknown_previous_composite_is_refused_by_the_carrier", "composite_carriers_initialize_define_commit_and_change", None),
        ("a_top_with_discrete_in_ports_runs_with_them_unknown", "a_top_with_discrete_in_ports_starts_them_at_their_initial_values", None),
        ("an_undriven_top_input_is_unknown", "an_undriven_top_input_keeps_its_initial_value", None),
        ("an_unknown_field_is_unknown", "a_record_holds_its_declared_fields_only", None),
        ("maybe_is_decoded_beside_the_fields", "a_record_holds_its_declared_fields_only", None),
        ("a_valueless_field_with_value_bytes_is_not_canonical", "the_shipped_layouts_hold_the_value_only", None),
        ("merge_keeps_equal_known_fields_and_clears_the_rest", "the_shipped_layouts_hold_the_value_only", None),
        ("merge_refuses_a_field_without_a_known_byte", "the_shipped_layouts_hold_the_value_only", None),
        ("merge_works_element_by_element_over_an_array_field", "the_shipped_layouts_hold_the_value_only", None),
        ("shipped_layouts_have_their_stated_fields", "the_shipped_layouts_hold_the_value_only", None),
        ("an_analog_read_of_a_carrier_outside_the_flag_reads_is_e0150", "an_analog_read_of_a_carrier_is_the_value", None),
        ("an_e0150_carries_a_help_line_naming_the_flag_reads", "an_analog_read_of_a_carrier_is_the_value", None),
        ("gaps_closes_s6_and_g6_by_the_redesign", "gaps_closes_s6_and_tracks_the_flag", None),
        ("known_resolves_into_std_values", "bit_rise_resolves_into_std_values", None),
        ("the_flag_reads_are_typed", "the_flag_methods_do_not_resolve", None),
        ("the_stdlib_d2a_reads_its_match_scrutinee_at_each_test", "the_stdlib_d2a_gives_transition_the_level_of_its_input", None),
        ("unknown_as_value_ok_as_name_e0908", "unknown_is_an_ordinary_name", None),
        ("the_unknown_nodes_are_refused_by_name", "the_level_nodes_are_refused_by_name", None),
        ("analog_reads_a_carrier_through_level_and_same_only", "no_value_type_is_flagged", None),
        ("the_unknown_nodes_type_print_and_check", "same_and_cast_type_print_and_check", None),
        ("unknown_values_and_their_reads_print", "no_value_type_is_flagged", None),
        ("rise_and_fall_fire_on_edges_to_and_from_unknown", "rise_and_fall_fire_on_transitions", None),
    ];
}

#[test]
fn no_source_uses_a_removed_unknown_form() {
    // EU-20 AC 1 (C86): no `.msl` under std, examples, tests or crates/*/tests spells `Unknown`, `known_or`, `is_known`,
    // `.known()`, `.high()`, `.level(` or `merge(` in code
    let mut offences = Vec::new();
    for (path, text) in RemovedUnknownForms::sources() {
        let spelled = RemovedUnknownForms::spelled(&text);
        if !spelled.is_empty() {
            offences.push(format!("{path} spells {spelled:?}"));
        }
    }
    assert!(offences.is_empty(), "sources that use a removed unknown form:\n  {}", offences.join("\n  "));
}

#[test]
fn every_unknown_test_has_a_successor() {
    // EU-20 AC 2 (C87): every test of the range that pinned an unknown, a flag, a merge or a known byte is deleted (`source_unknowns.rs` and the others the table lists), and each is named in the table with the test that replaces
    // it; the old name is spelled by no test, the new one is a test of the tree, unless a later task writes it
    let sources = rust_sources();
    let defines = |name: &str| sources.iter().any(|(_, text)| text.contains(&format!("fn {name}(")));
    let mut offences = Vec::new();
    for (old, new, pending) in RemovedUnknownForms::SUCCESSORS {
        if defines(old) {
            offences.push(format!("{old} is still a test: replace it by {new}"));
        }
        match pending {
            None if !defines(new) => offences.push(format!("{old}: the successor {new} is no test of the tree")),
            Some(task) if defines(new) => offences.push(format!("{old}: {new} exists now: drop `Some(\"{task}\")` from its row")),
            _ => {}
        }
    }
    assert!(offences.is_empty(), "{}", offences.join("\n"));
}

#[test]
fn retired_unknown_codes_are_raised_nowhere() {
    // EU-02 AC 4 (C7): `E0150`, `E0151`, `E0906` and `E0908` are retired with the model of
    // unknowns and never reused (§0.2), so no library source of any crate spells one as a code it
    // raises: the string literal of a code is how every raise names it.
    let retired = ["E0150", "E0151", "E0906", "E0908"];
    let mut offences = Vec::new();
    for (path, text) in library_sources() {
        for (n, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            for code in retired {
                if line.contains(&format!("\"{code}\"")) {
                    offences.push(format!("{path}:{}: raises the retired {code}", n + 1));
                }
            }
        }
    }
    assert!(offences.is_empty(), "{}", offences.join("\n"));
}
