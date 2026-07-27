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

/// The repository root — this test target lives in the root package.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// This file: it *spells* the forbidden patterns, so it is the one source the
/// walk must skip — the rule book is not a subject of its own rules.
const SELF: &str = "tests/suite_hygiene.rs";

/// Every `.rs` file under `crates/`, `src/`, and `tests/`, skipping build
/// output and this file. Returned as `(repo-relative path, contents)`.
fn rust_sources() -> Vec<(String, String)> {
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

/// Every root `tests/*.rs` target, as `(file name, contents)` — top level only,
/// so a `tests/<dir>/` helper module is not mistaken for a target.
fn root_targets() -> Vec<(String, String)> {
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

/// Every integration-test target: `crates/*/tests/*.rs` and root `tests/*.rs`
/// (top level only — a `tests/<dir>/` module file is a helper, not a target).
fn integration_targets() -> Vec<(String, String)> {
    rust_sources()
        .into_iter()
        .filter(|(path, _)| {
            let parts: Vec<&str> = path.split('/').collect();
            matches!(parts.as_slice(),
                ["tests", _file] | ["crates", _, "tests", _file]
            )
        })
        .collect()
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
        ("//! formerly the standalone `piperine-ir` crate", "piperine-ir"),
        ("use piperine_ir::Program;", "piperine_ir"),
    ] {
        assert_eq!(
            dead_architecture_identifiers_in(offender),
            vec![expected],
            "must be flagged as dead architecture: {offender:?}"
        );
    }
    for innocent in ["//! the POM `Design`/`Module`/`Instance`", "use piperine_lang::pom;", ""] {
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
            "crates/piperine-plugin/src/lib.rs" => {
                "shows a plugin crate's own entry symbol — only compiles inside a cdylib plugin"
            }
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
    assert_eq!(seen.len(), 4, "the registered set changed — update the registry: {seen:?}");
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

/// The names of the deleted IR layer. There is no `piperine-ir` crate and no
/// `IrProgram`/`IrModule`/`IrExpr`/`IrInstance` type: codegen resolves a POM
/// `Design`/`Module`/`Instance` directly. P6 found sixteen comments defining the
/// code by that dead architecture — two of them broken intra-doc links. A
/// comment that can only be checked against a deleted crate cannot be checked
/// at all.
const DEAD_ARCHITECTURE_IDENTIFIERS: [&str; 6] =
    ["IrProgram", "IrModule", "IrExpr", "IrInstance", "piperine-ir", "piperine_ir"];

/// Which dead identifiers one source line mentions.
fn dead_architecture_identifiers_in(line: &str) -> Vec<&'static str> {
    DEAD_ARCHITECTURE_IDENTIFIERS.into_iter().filter(|id| line.contains(id)).collect()
}

/// The registry of deliberate historical notes: file → (identifier, why it
/// survives). Exactly one is allowed — the note in `piperine-codegen`'s `lib.rs`
/// where the pipeline is introduced, which is the one place a reader benefits
/// from knowing the resolved layer used to be its own crate. Everything else
/// describes the present. Registry + exhaustiveness (the
/// `capabilities_contract.rs` shape): a note that disappears fails the test just
/// as loudly as a new one appearing.
fn registered_historical_note(path: &str) -> Option<(&'static str, &'static str)> {
    match path {
        "crates/piperine-codegen/src/lib.rs" => Some((
            "piperine-ir",
            "the pipeline overview names the crate the `resolve` stage was split out of",
        )),
        _ => None,
    }
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
         IR crate and its `Ir*` types no longer exist, codegen resolves the POM \
         `Design`/`Module`/`Instance` directly):\n  {}",
        offences.join("\n  ")
    );
    assert_eq!(
        notes_seen,
        vec!["crates/piperine-codegen/src/lib.rs".to_string()],
        "the registered historical note moved or vanished — update the registry"
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
    assert!(
        files.len() > 15,
        "the walk found only {} `mod.rs` files — the layout moved and this guard is blind",
        files.len()
    );
    files
}

/// The registry of `mod.rs` files allowed to exceed the ceiling: path → why the
/// split is not this feature's work. Registry + exhaustiveness (the
/// `capabilities_contract.rs` shape used by the other guards here): a new
/// exemption fails until it is recorded, and an exemption that becomes
/// unnecessary fails just as loudly, so the list cannot rot into a blanket.
///
/// All seven are pre-existing and outside `p6-cleanup-architecture`'s task
/// list; the in-file `// hygiene-exempt:` line carries the per-file reason.
fn registered_oversized_mod_rs(path: &str) -> Option<&'static str> {
    Some(match path {
        "crates/piperine-lang/src/parse/parser/mod.rs" => "`Parser` + the `Parse` trait; frozen parse tree",
        "crates/piperine-lang/src/parse/format/mod.rs" => "formatter vocabulary + `FormatRule`; frozen parse tree",
        "crates/piperine-lang/src/parse/mod.rs" => "phase contract + the three parse entry points",
        "crates/piperine-lang/src/elab/lower/mod.rs" => "`SymbolTable` + the `Elaborator` god struct",
        "crates/piperine-lang/src/elab/mod.rs" => "long-form elaboration phase contract",
        "crates/piperine-lang/src/elab/registry/mod.rs" => "registry re-exports + `ElabContext`",
        "crates/piperine-codegen/src/resolve/mod.rs" => "the resolved form's own vocabulary",
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
        7,
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
/// **Empty, deliberately.** Every root target today is a shell proof or a
/// cross-crate proof. This file — the one root target that links no workspace
/// crate — needs no entry because the walk already skips it as [`SELF`]. An
/// entry added here must move the count in [`root_targets_match_the_naming_rule`]
/// with it, so the exemption cannot be granted quietly.
const ROOT_TARGET_EXEMPTIONS: [(&str, &str); 0] = [];

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
    assert!(
        exempted.is_empty(),
        "the exempted root-target set grew — record the new exemption's reason in \
         `ROOT_TARGET_EXEMPTIONS` and move this assertion's expected set with it: \
         {exempted:?}"
    );
}
