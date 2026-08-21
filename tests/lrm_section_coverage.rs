//! LRM section parser + coverage scraper (T4, `lrm-conformance-wave2`).
//!
//! `lrm_sections()` parses every `^#{2,6} **<id>` heading out of
//! `VAMS-LRM-2-4.md` (numbered `1`..`12.x`, annex `A`..`H.x`) into the flat
//! id set every later wave-2 conformance claim is checked against.
//! `declared_coverage()` scrapes `//! LRM-COVERS: <ids>` out of every test
//! target (`crates/*/tests/*.rs`, root `tests/*.rs`) and every inline
//! `#[cfg(test)]` module header in library source (the "pure helpers" row of
//! the Test Coverage Matrix — `piperine-signal`, `math.rs`, etc.), so a
//! coverage claim can live wherever the test that proves it lives.
//!
//! This file is itself infrastructure, not a clause proof, so it carries no
//! `LRM-COVERS` claim of its own (T6's "declaring is optional" rule).

use std::collections::BTreeSet;

// This target only needs repo_root/rust_sources — integration_targets and
// root_targets stay unused here (MD-33: the exemption sits on the item it
// excuses, but the shared module's shape is fixed by suite_hygiene.rs's own
// needs, so the allow sits on the import instead of splitting the module).
#[allow(dead_code)]
#[path = "support/sources.rs"]
mod sources;
use sources::{repo_root, rust_sources};

/// Every LRM-2-4 section id this workspace's conformance gate can check
/// coverage against: `1`..`12.x` (numbered) and `A`..`H.x` (annex),
/// parsed straight out of `VAMS-LRM-2-4.md`'s own Markdown headings —
/// never a hand-maintained list, so it cannot go stale silently.
pub fn lrm_sections() -> BTreeSet<String> {
    let root = repo_root();
    let path = root.join("VAMS-LRM-2-4.md");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    let mut ids = BTreeSet::new();
    for line in text.lines() {
        let hashes = line.chars().take_while(|&c| c == '#').count();
        if !(2..=6).contains(&hashes) {
            continue;
        }
        // The document's own §1.5 table of contents re-lists every heading
        // wrapped in `<u>…</u>` — those are navigation duplicates of the
        // real heading further down, not a second section.
        if line.contains("<u>") {
            continue;
        }
        if let Some(id) = parse_heading_id(line) {
            ids.insert(id);
        }
    }
    ids
}

/// Extract a section id from one heading line, or `None` if the line is not
/// an id'd section heading (front matter, a `Figure`/glossary-term heading,
/// etc.). A numbered id is one or more dot-separated digit groups (`4`,
/// `4.5`, `4.5.3`); an annex id is a single letter `A`..`H` followed by at
/// least one dot-separated digit group (`A.1`, `A.1.2`) — a bare letter with
/// no digit group is a glossary entry (Annex H's `**T**`, `**V**`, …), not a
/// section.
fn parse_heading_id(line: &str) -> Option<String> {
    let after_hashes = line.trim_start_matches('#').trim_start();
    let after_bold = after_hashes.strip_prefix("**")?;
    let token = after_bold.split_whitespace().next()?;
    // Most headings are `**<id> <title>**`, but a few (e.g. `**G.2.4**
    // **`default_function_type_analog**`) close the bold span immediately
    // after the id with no separating space, so the first whitespace token
    // can carry a trailing `**`/`.` that is punctuation, not part of the id.
    let token = token.trim_end_matches(|c: char| c == '.' || c == '*');
    if token.is_empty() {
        return None;
    }
    let segments: Vec<&str> = token.split('.').collect();
    let first = segments[0];
    let is_digit_group = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    let numbered_head = is_digit_group(first);
    let annex_head =
        first.len() == 1 && matches!(first.chars().next(), Some('A'..='H')) && segments.len() > 1;
    if !(numbered_head || annex_head) {
        return None;
    }
    if segments[1..].iter().any(|seg| !is_digit_group(seg)) {
        return None;
    }
    Some(token.to_string())
}

/// One `//! LRM-COVERS:` declaration: which file declared it, the ids it
/// claims, and whether that same file has a byte trail of the ids (used by
/// [`declared_coverage`]'s own callers, not stored here — a declaration is
/// just the raw claim).
pub struct Declaration {
    pub file: String,
    pub ids: Vec<String>,
}

/// Every `//! LRM-COVERS: <ids>` declaration in the workspace, scraped from
/// every `.rs` file under `crates/`, `src/`, and `tests/` — a test target's
/// file-level header and an inline `#[cfg(test)] mod tests { //! … }`
/// header both use the same `//!` doc-comment convention, so one scan finds
/// both. Multiple declarations in one file are unlikely per convention but
/// not disallowed — each `LRM-COVERS:` line becomes its own [`Declaration`].
pub fn declared_coverage() -> Vec<Declaration> {
    let mut decls = Vec::new();
    for (path, text) in rust_sources() {
        for line in text.lines() {
            let Some(rest) = line.trim_start().strip_prefix("//! LRM-COVERS:") else {
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
/// — a declaration in a file with none is unfalsifiable, so it is an error,
/// not just noise. (The project separately forbids `#[ignore]` everywhere
/// via `suite_hygiene::no_ignored_tests`; this check is independent so this
/// gate does not silently depend on that one staying wired up.)
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
fn lrm_sections_matches_the_document() {
    let sections = lrm_sections();
    let numbered =
        sections.iter().filter(|id| id.chars().next().is_some_and(|c| c.is_ascii_digit())).count();
    let annex = sections.len() - numbered;
    assert_eq!(
        numbered, 486,
        "numbered LRM-2-4 heading count drifted from 486 — either the document changed \
         (update this figure) or the heading parser regressed"
    );
    assert_eq!(
        annex, 126,
        "annex LRM-2-4 heading count drifted from 126 — either the document changed \
         (update this figure) or the heading parser regressed"
    );
    for known in ["4.5.3", "A.1.2", "F.2.1"] {
        assert!(
            sections.contains(known),
            "expected {known} in the parsed LRM-2-4 section set, got {} ids total",
            sections.len()
        );
    }
}

#[test]
fn declared_ids_exist_in_the_lrm() {
    let sections = lrm_sections();
    let mut typos = Vec::new();
    for decl in declared_coverage() {
        for id in &decl.ids {
            if !sections.contains(id) {
                typos.push(format!("{}: declares LRM-COVERS id `{id}` — not a real LRM-2-4 section id", decl.file));
            }
        }
    }
    assert!(
        typos.is_empty(),
        "LRM-COVERS declarations name ids that do not exist in VAMS-LRM-2-4.md (typo?):\n  {}",
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
        "these files declare LRM-COVERS with no live (non-#[ignore]d) #[test] to back the \
         claim — a coverage claim with no test proves nothing:\n  {}",
        unfalsifiable.join("\n  ")
    );
}

