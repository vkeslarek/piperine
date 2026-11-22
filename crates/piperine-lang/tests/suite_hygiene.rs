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
