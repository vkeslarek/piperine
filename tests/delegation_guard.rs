//! Guards the `codegen-simplification` delegation: for each of the five
//! families `piperine-ir` now owns (runtime operators, event detectors,
//! the junction limiter, `table`, digital edge detection), a second local
//! copy must never quietly reappear in `piperine-codegen` or
//! `piperine-api`. Source-walking style like `tests/suite_hygiene.rs` —
//! never a hardcoded test-name list, and a `const FAMILIES` registry so a
//! new family is exhaustively accounted for rather than silently unguarded.
//!
//! MD-31 obligation: each of the five witnesses below has been proven able
//! to fail — the forbidden marker was injected locally into the relevant
//! source file, `cargo test --test delegation_guard` was run and observed
//! to fail naming that family and `file:line`, then the injection was
//! reverted. Recorded per-family in this commit's message.

use std::fs;
use std::path::{Path, PathBuf};

/// The repository root — this target lives in the root package.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// One delegated family: its name, the exact strings that must never appear
/// again (a reintroduced local copy is caught by any one match), and the
/// crate `src` roots the family had a copy in before delegation.
struct Family {
    name: &'static str,
    forbidden: &'static [&'static str],
    roots: &'static [&'static str],
}

/// Five entries — one per family `codegen-simplification` delegated
/// (spec CGS-11, AC 22). The digital-edges entry is the only one that
/// walks two crates: it had copies in both `piperine-codegen` and
/// `piperine-api` (`piperine_codegen::device::digital` and
/// `piperine_api::sequencer::wait`) before this feature, and the two
/// copies had already drifted (spec §0 item 3) before being unified onto
/// `piperine_ir::logic`.
const FAMILIES: &[Family] = &[
    Family {
        name: "operators",
        // AC 1: codegen defines no `Operator` enum of its own.
        forbidden: &["enum Operator"],
        roots: &["crates/piperine-codegen/src"],
    },
    Family {
        name: "events",
        // AC 5: codegen defines no `EventDetector` of its own.
        forbidden: &["struct EventDetector"],
        roots: &["crates/piperine-codegen/src"],
    },
    Family {
        name: "limiter",
        // AC 10: emit/analog_expr.rs holds no emit_pnjlim/emit_fetlim/
        // emit_limvds (and the two small IR helpers only they used).
        forbidden: &[
            "fn emit_pnjlim",
            "fn emit_fetlim",
            "fn emit_limvds",
            "fn emit_min(",
            "fn emit_max(",
        ],
        roots: &["crates/piperine-codegen/src"],
    },
    Family {
        name: "table",
        // AC 16: the strictly-increasing check is signal::Table's alone —
        // this is its exact hand-rolled form, deleted from analog_ops.rs.
        forbidden: &["xs.windows(2).all(|w| w[0] < w[1])"],
        roots: &["crates/piperine-codegen/src"],
    },
    Family {
        name: "digital edges",
        // AC 30/31: no inline edge comparison in either
        // piperine_codegen::device::digital or piperine_api::sequencer::wait
        // — both crates' pre-delegation rule text, exactly as it read.
        forbidden: &[
            "prev != 1 && cur == 1",
            "prev != 0 && cur == 0",
            "before == LogicValue::Zero && after == LogicValue::One",
            "before == LogicValue::One && after == LogicValue::Zero",
        ],
        roots: &["crates/piperine-codegen/src", "crates/piperine-api/src"],
    },
];

/// Every `.rs` file under `root` (repo-relative), as `(repo-relative path,
/// contents)`.
fn rust_sources_under(root: &str) -> Vec<(String, String)> {
    let repo = repo_root();
    let mut out = Vec::new();
    collect(&repo.join(root), &repo, &mut out);
    out
}

fn collect(dir: &Path, repo: &Path, out: &mut Vec<(String, String)>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if path.is_dir() {
            if name != "target" && name != ".git" {
                collect(&path, repo, out);
            }
        } else if path.extension().is_some_and(|e| e == "rs") {
            let rel = path.strip_prefix(repo).unwrap_or(&path).to_string_lossy().to_string();
            if let Ok(text) = fs::read_to_string(&path) {
                out.push((rel, text));
            }
        }
    }
}

/// One family's check: every forbidden marker, over every file under every
/// one of its roots. Failure names the family and the offending
/// `file:line`.
fn offences_for(family: &Family) -> Vec<String> {
    let mut offences = Vec::new();
    for root in family.roots {
        for (path, text) in rust_sources_under(root) {
            for (index, line) in text.lines().enumerate() {
                for marker in family.forbidden {
                    if line.contains(marker) {
                        offences.push(format!(
                            "{path}:{}: a second copy of the {} family reappeared \
                             (`{marker}`) — {name} was delegated to piperine-ir, \
                             a local copy must not come back",
                            index + 1,
                            family.name,
                            name = family.name,
                        ));
                    }
                }
            }
        }
    }
    offences
}

#[test]
fn no_delegated_family_has_a_second_copy() {
    assert!(FAMILIES.len() == 5, "the registry must name all five delegated families");

    let mut all_offences = Vec::new();
    for family in FAMILIES {
        all_offences.extend(offences_for(family));
    }
    assert!(
        all_offences.is_empty(),
        "delegation_guard found a reintroduced local copy:\n  {}",
        all_offences.join("\n  ")
    );
}

/// A registry with a bad root (a typo, a moved directory) walks nothing and
/// passes for free — decoration, not a guard. Every root named above must
/// resolve to at least one `.rs` file today.
#[test]
fn every_family_root_resolves_to_real_sources() {
    let mut empty_roots = Vec::new();
    for family in FAMILIES {
        for root in family.roots {
            if rust_sources_under(root).is_empty() {
                empty_roots.push(format!("{}: {root}", family.name));
            }
        }
    }
    assert!(
        empty_roots.is_empty(),
        "these family roots found no `.rs` files — the guard is blind:\n  {}",
        empty_roots.join("\n  ")
    );
}
