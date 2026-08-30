//! Spec heading parser + `SPEC-COVERS` scraper — axis B (T45,
//! `lrm-conformance-wave3`, PSW3-10).
//!
//! Mirrors `tests/lrm_section_coverage.rs` one-for-one, but reads
//! `docs/spec/*.md` instead of `VAMS-LRM-2-4.md`: axis A asks "does Piperine
//! implement what the LRM requires"; axis B asks "does Piperine implement
//! what *its own* specification claims" (spec.md §0). `spec_sections()`
//! parses every numbered `##`/`###` heading in `docs/spec/*.md` into an id
//! `<file-prefix>#<heading-id>` (`part_iv#9`, `appendix_c#3`,
//! `part_viii#15.2`); `declared_coverage()` scrapes `//! SPEC-COVERS: <ids>`
//! the same way `lrm_section_coverage.rs` scrapes `LRM-COVERS`.
//!
//! This file is itself infrastructure, not a claim of its own, so it carries
//! no `SPEC-COVERS` declaration (mirroring T6's "declaring is optional"
//! rule for `LRM-COVERS`).

use std::collections::BTreeSet;

// Same shared walker `lrm_section_coverage.rs` uses; `integration_targets`/
// `root_targets` stay unused here for the same MD-33 reason that file
// documents (the shared module's shape is fixed by suite_hygiene.rs's own
// needs).
#[allow(dead_code)]
#[path = "support/sources.rs"]
mod sources;
use sources::{repo_root, rust_sources};

/// Every `docs/spec/*.md` file this gate reads, in a fixed order (not a
/// directory-iteration order, so a filesystem quirk cannot reorder the
/// pinned-count test's failure grouping).
const SPEC_FILES: &[&str] = &[
    "docs/spec/index.md",
    "docs/spec/part_i_language.md",
    "docs/spec/part_ii_elaboration.md",
    "docs/spec/part_iii_conformance.md",
    "docs/spec/part_iv_reflection_selector.md",
    "docs/spec/part_v_builtins.md",
    "docs/spec/part_vi_plugins.md",
    "docs/spec/part_vii_solver.md",
    "docs/spec/part_viii_host_api.md",
    "docs/spec/part_ix_signal_semantics.md",
    "docs/spec/part_x_codegen_pipeline.md",
    "docs/spec/appendix_a_worked_examples.md",
    "docs/spec/appendix_b_grammar.md",
    "docs/spec/appendix_c_host_surface.md",
];

/// The id prefix for one `docs/spec/*.md` file: `part_<roman>` /
/// `appendix_<letter>` (the first two `_`-separated stem components), or the
/// bare stem for anything else (`index.md` -> `index`) — mirrors how
/// `part_viii_host_api.md` collapses to `part_viii` in the design's own
/// example ids (`part_viii#15.2`).
fn file_prefix(stem: &str) -> String {
    let parts: Vec<&str> = stem.split('_').collect();
    if parts.len() >= 2 && (parts[0] == "part" || parts[0] == "appendix") {
        format!("{}_{}", parts[0], parts[1])
    } else {
        stem.to_string()
    }
}

/// Extract a heading's numeric/lettered id token from one heading line, or
/// `None` if the heading carries no id (a prose heading like `## Contents` or
/// `### Entry + reflection` — those have nothing to prove and are simply
/// absent from the id set, the same way the LRM parser skips front matter).
/// Handles `§9`, `§9.1`, `9.` (trailing period before the title), `9.1`
/// (bare), and letter-prefixed `A.1`/`B.1.1` (already spelled that way in
/// the appendix headings themselves).
fn parse_heading_id(line: &str) -> Option<String> {
    let after_hashes = line.trim_start_matches('#').trim_start();
    let token = after_hashes.split_whitespace().next()?;
    let token = token.strip_prefix('§').unwrap_or(token);
    let token = token.trim_end_matches('.');
    if token.is_empty() {
        return None;
    }
    let segments: Vec<&str> = token.split('.').collect();
    let first = segments[0];
    let is_digit_group = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    let numbered_head = is_digit_group(first);
    let letter_head = first.len() == 1 && first.chars().next().is_some_and(|c| c.is_ascii_uppercase());
    if !(numbered_head || letter_head) {
        return None;
    }
    if segments[1..].iter().any(|seg| !is_digit_group(seg)) {
        return None;
    }
    Some(token.to_string())
}

/// Every Piperine-spec section id this workspace's axis-B gate can check
/// coverage against: `<file-prefix>#<heading-id>`, parsed straight out of
/// `docs/spec/*.md`'s own Markdown headings — never a hand-maintained list,
/// so it cannot go stale silently (the same guarantee `lrm_sections()` gives
/// axis A).
pub fn spec_sections() -> BTreeSet<String> {
    let root = repo_root();
    let mut ids = BTreeSet::new();
    for rel in SPEC_FILES {
        let path = root.join(rel);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
        let stem = std::path::Path::new(rel).file_stem().unwrap().to_string_lossy().to_string();
        let prefix = file_prefix(&stem);
        for line in text.lines() {
            let hashes = line.chars().take_while(|&c| c == '#').count();
            if !(2..=6).contains(&hashes) {
                continue;
            }
            if let Some(id) = parse_heading_id(line) {
                ids.insert(format!("{prefix}#{id}"));
            }
        }
    }
    ids
}

/// One `//! SPEC-COVERS: <ids>` declaration: which file declared it and the
/// ids it claims.
pub struct Declaration {
    pub file: String,
    pub ids: Vec<String>,
}

/// Every `//! SPEC-COVERS: <ids>` declaration in the workspace, scraped the
/// same way `lrm_section_coverage.rs::declared_coverage` scrapes
/// `LRM-COVERS` — a test target's file-level header or an inline
/// `#[cfg(test)]` module header, both `//!` doc comments.
pub fn declared_coverage() -> Vec<Declaration> {
    let mut decls = Vec::new();
    for (path, text) in rust_sources() {
        for line in text.lines() {
            let Some(rest) = line.trim_start().strip_prefix("//! SPEC-COVERS:") else {
                continue;
            };
            let ids: Vec<String> =
                rest.split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect();
            if !ids.is_empty() {
                decls.push(Declaration { file: path.clone(), ids });
            }
        }
    }
    decls
}

/// Whether a source file has at least one live (non-`#[ignore]`d) `#[test]`
/// — identical in shape to `lrm_section_coverage.rs::has_live_test`: a
/// declaration in a file with none is unfalsifiable.
fn has_live_test(text: &str) -> bool {
    let lines: Vec<&str> = text.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if line.trim_start() != "#[test]" {
            continue;
        }
        let mut ignored = false;
        let mut j = i;
        while j > 0 {
            j -= 1;
            let prev = lines[j].trim();
            if prev.is_empty() {
                continue;
            }
            if prev.starts_with("#[ignore") {
                ignored = true;
                break;
            }
            if prev.starts_with('#') || prev.starts_with("//") {
                continue;
            }
            break;
        }
        if !ignored {
            return true;
        }
    }
    false
}

#[test]
fn spec_sections_matches_the_document() {
    let sections = spec_sections();
    assert_eq!(
        sections.len(),
        284,
        "docs/spec/*.md numbered-heading count drifted from 284 — either the docs changed \
         (update this figure) or the heading parser regressed. Got: {:#?}",
        sections
    );
    for known in ["part_iv#9", "appendix_c#3", "part_iii#1"] {
        assert!(
            sections.contains(known),
            "expected {known} in the parsed docs/spec section set, got {} ids total",
            sections.len()
        );
    }
}

#[test]
fn declared_ids_exist_in_the_spec() {
    let sections = spec_sections();
    let mut typos = Vec::new();
    for decl in declared_coverage() {
        for id in &decl.ids {
            if !sections.contains(id) {
                typos.push(format!(
                    "{}: declares SPEC-COVERS id `{id}` — not a real docs/spec section id",
                    decl.file
                ));
            }
        }
    }
    assert!(
        typos.is_empty(),
        "SPEC-COVERS declarations name ids that do not exist in docs/spec/*.md (typo?):\n  {}",
        typos.join("\n  ")
    );
}

#[test]
fn declaring_files_have_a_live_test() {
    let sources: std::collections::HashMap<String, String> = rust_sources().into_iter().collect();
    let mut unfalsifiable = Vec::new();
    for decl in declared_coverage() {
        let text = sources.get(&decl.file).map(String::as_str).unwrap_or_default();
        if !has_live_test(text) {
            unfalsifiable.push(decl.file);
        }
    }
    assert!(
        unfalsifiable.is_empty(),
        "these files declare SPEC-COVERS with no live (non-#[ignore]d) #[test] to back the \
         claim — a coverage claim with no test proves nothing:\n  {}",
        unfalsifiable.join("\n  ")
    );
}

