//! The lrm-conformance-closure conformance registry (MD-31 policy-invariant
//! gate, design.md's "Cross-cutting: `lrm_conformance_matrix.rs`"): makes
//! "N LRMC findings closed" mechanically checkable instead of a claim in a
//! doc. Same registry+exhaustiveness shape as
//! `crates/piperine-solver/tests/capabilities_contract.rs`/
//! `spec_failure_rules_guard.rs` (MD-31).
//!
//! **Two independent checks, both required:**
//! 1. This file's hardcoded [`REGISTRY`] — every LRMC-ID from
//!    `.specs/features/lrm-conformance-closure/spec.md`'s own Requirement
//!    Traceability table, each carrying its LRM clause, story, and a
//!    [`Status`] (`Pending`, `OutOfScope`, or `Closed` naming the real test
//!    that closes it) — is cross-checked against the *live* table text
//!    (`include_str!`, so a spec.md edit that adds/removes/renames an ID is
//!    caught, never silently drifting) for an exact ID-set match: no
//!    registry entry without a matching spec row, no spec row without a
//!    matching registry entry, no duplicate ID on either side.
//! 2. Every `Closed` entry's named test must actually exist — the file it
//!    names is read from disk and must contain a `#[test] fn <name>` (or
//!    `fn <name>` immediately preceded by `#[test]` on the line above) that
//!    is not `#[ignore]`d — a stale reference to a renamed/deleted test
//!    fails this gate, not just a doc claim.
//!
//! **Count note (disclosed, not silently reconciled):** spec.md's own
//! Coverage paragraph states "51 requirement IDs" / "51 mapped to tasks",
//! but its Requirement Traceability table, counted literally
//! (`grep -c '^| LRMC-'`), has 54 rows (52 tracked toward closure + 2
//! `Out of Scope`: LRMC-C10-15, LRMC-C10-17). This gate does not assert a
//! magic count — it asserts *set equality* against the table's actual
//! content, which is authoritative over the prose count per this task's own
//! instruction ("Source of the 51 IDs: spec.md's Requirement Traceability
//! table"). The discrepancy is spec.md's own prose vs. its own table, not
//! introduced by this gate.
//!
//! **Guard proven able to fail:** during T9's implementation, a registry
//! entry's `test_fn` was temporarily renamed to a nonexistent name and the
//! `every_closed_entry_names_a_real_test` check failed as expected, then
//! reverted — recorded in this task's commit body per the task's own
//! "Done when" requirement (not left as a comment here, since the mutation
//! itself is not part of the shipped gate).

use std::fs;
use std::path::PathBuf;

/// The status of one LRMC-ID's closure.
enum Status {
    /// Not yet closed by any task in this batch.
    Pending,
    /// Explicitly excluded from closure per spec.md's own Out of Scope
    /// table — never expected to gain a `Closed` status.
    OutOfScope,
    /// Closed by the named test — `file` is a path relative to the
    /// workspace root, `test_fn` is the `#[test] fn` name inside it.
    Closed { file: &'static str, test_fn: &'static str },
}

struct Entry {
    id: &'static str,
    story: &'static str,
    clause: &'static str,
    status: Status,
}

/// The full registry — one entry per LRMC-ID in spec.md's Requirement
/// Traceability table, in the table's own order.
const REGISTRY: &[Entry] = &[
    Entry { id: "LRMC-C1-01", story: "P1", clause: "§2.6.1", status: Status::Pending },
    Entry { id: "LRMC-C1-02", story: "P1", clause: "§2.6.1", status: Status::Closed { file: "crates/piperine-lang/tests/vams_four_state_literals.rs", test_fn: "sixteen_bit_hex_x_evaluates_to_a_four_state_logic_value_not_zero" } },
    Entry { id: "LRMC-C1-03", story: "P1", clause: "§2.6.1", status: Status::Pending },
    Entry { id: "LRMC-C1-04", story: "P1", clause: "§2.6.1/§2.6.2", status: Status::Pending },
    Entry { id: "LRMC-C1-05", story: "P1", clause: "§2.9/§2.9.1", status: Status::Pending },
    Entry { id: "LRMC-C1-06", story: "P1", clause: "§3.4.2/3.4.4/3.4.6/3.4.8/4.2.14", status: Status::Pending },
    Entry { id: "LRMC-C1-07", story: "P1", clause: "§3.4.7", status: Status::Pending },
    Entry { id: "LRMC-C1-08", story: "P1", clause: "§4.4/§3.6.1.2", status: Status::Pending },
    Entry { id: "LRMC-C1-09", story: "P1", clause: "§4.4/§5.4.2/§5.4.3", status: Status::Pending },
    Entry { id: "LRMC-C1-10", story: "P1", clause: "§4.4/§5.4.3", status: Status::Pending },
    Entry { id: "LRMC-C1-11", story: "P1", clause: "§4.2.1.1", status: Status::Pending },
    Entry { id: "LRMC-C1-12", story: "P1", clause: "§4.5.11.1-4", status: Status::Pending },
    Entry { id: "LRMC-C5-01", story: "P2", clause: "§5.6", status: Status::Pending },
    Entry { id: "LRMC-C5-02", story: "P2", clause: "§5.10.3.1", status: Status::Pending },
    Entry { id: "LRMC-C5-03", story: "P2", clause: "§5.10.3.4", status: Status::Pending },
    Entry { id: "LRMC-C5-04", story: "P2", clause: "§5.10.4", status: Status::Pending },
    Entry { id: "LRMC-C5-05", story: "P2", clause: "§5.10.5", status: Status::Pending },
    Entry { id: "LRMC-C5-06", story: "P2", clause: "§6.3.1", status: Status::Pending },
    Entry { id: "LRMC-C5-07", story: "P2", clause: "§6.4", status: Status::Pending },
    Entry { id: "LRMC-C5-08", story: "P2", clause: "§6.2.2", status: Status::Pending },
    Entry { id: "LRMC-C7-01", story: "P3", clause: "§7.4.4.1/§7.4.4.3/§7.5-7.8", status: Status::Pending },
    Entry { id: "LRMC-C7-02", story: "P3", clause: "§7.7.1", status: Status::Pending },
    Entry { id: "LRMC-C7-03", story: "P3", clause: "§7.7.3/§7.7.4", status: Status::Pending },
    Entry { id: "LRMC-C7-04", story: "P3", clause: "§7.4.4.3", status: Status::Pending },
    Entry { id: "LRMC-C7-05", story: "P3", clause: "§7.6/§9.22-9.23", status: Status::Pending },
    Entry { id: "LRMC-C7-06", story: "P3", clause: "§7.3.6", status: Status::Pending },
    Entry { id: "LRMC-C7-07", story: "P3", clause: "§9.2/Table 9-1", status: Status::Pending },
    Entry { id: "LRMC-C7-08", story: "P3", clause: "§9.2/§9.4.2/§9.4.4", status: Status::Pending },
    Entry { id: "LRMC-C7-09", story: "P3", clause: "§9.2-9.23", status: Status::Pending },
    Entry { id: "LRMC-C7-10", story: "P3", clause: "§9.2", status: Status::Pending },
    Entry { id: "LRMC-C7-11", story: "P3", clause: "§9.17.1", status: Status::Pending },
    Entry { id: "LRMC-C7-12", story: "P3", clause: "§8.3", status: Status::Pending },
    Entry { id: "LRMC-C7-13", story: "P3", clause: "§8.5.3.5", status: Status::Pending },
    Entry { id: "LRMC-C7-14", story: "P3", clause: "§8.4.6", status: Status::Pending },
    Entry { id: "LRMC-C10-01", story: "P4", clause: "§10.2", status: Status::Closed { file: "crates/piperine-lang/tests/vams_directives.rs", test_fn: "default_discipline_sets_the_named_discipline" } },
    Entry { id: "LRMC-C10-02", story: "P4", clause: "§10.3", status: Status::Closed { file: "crates/piperine-lang/tests/vams_directives.rs", test_fn: "default_transition_sets_rise_and_fall_to_the_same_value" } },
    Entry { id: "LRMC-C10-03", story: "P4", clause: "§10.5", status: Status::Pending },
    Entry { id: "LRMC-C10-04", story: "P4", clause: "§10.6", status: Status::Pending },
    Entry { id: "LRMC-C10-05", story: "P4", clause: "ch.10 table", status: Status::Pending },
    Entry { id: "LRMC-C10-06", story: "P4", clause: "ch.10 table", status: Status::Pending },
    Entry { id: "LRMC-C10-07", story: "P4", clause: "ch.10 table", status: Status::Pending },
    Entry { id: "LRMC-C10-08", story: "P4", clause: "ch.10 table", status: Status::Pending },
    Entry { id: "LRMC-C10-09", story: "P4", clause: "ch.10 table", status: Status::Pending },
    Entry { id: "LRMC-C10-10", story: "P4", clause: "ch.10 table", status: Status::Pending },
    Entry { id: "LRMC-C10-11", story: "P4→P5", clause: "§11.2.1/Table 11-1", status: Status::Pending },
    Entry { id: "LRMC-C10-12", story: "P4→P5", clause: "§11.4 Table 11-7", status: Status::Pending },
    Entry { id: "LRMC-C10-13", story: "P4→P5", clause: "§11.4 Table 11-8", status: Status::Pending },
    Entry { id: "LRMC-C10-14", story: "P4→P5", clause: "§11.4 Table 11-10", status: Status::Pending },
    Entry { id: "LRMC-C10-15", story: "Out of Scope", clause: "Annex C.4", status: Status::OutOfScope },
    Entry { id: "LRMC-C10-16", story: "P4", clause: "Annex D.3", status: Status::Pending },
    Entry { id: "LRMC-C10-17", story: "Out of Scope", clause: "Annex E.2.1/E.3.2", status: Status::OutOfScope },
    Entry { id: "LRMC-C10-18", story: "P4", clause: "Annex F §7.4.4.1", status: Status::Closed { file: "crates/piperine-lang/tests/vams_annex_f_domain.rs", test_fn: "continuous_wins_over_discrete_matching_figure_7_3" } },
    Entry { id: "LRMC-C10-19", story: "P4", clause: "Annex F §7.4.4.1 step 4.b", status: Status::Closed { file: "crates/piperine-lang/tests/vams_annex_f_domain.rs", test_fn: "unresolved_net_falls_back_to_default_discipline_directive" } },
    Entry { id: "LRMC-C10-20", story: "P4", clause: "Annex F §7.4.1", status: Status::Pending },
];

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

const SPEC: &str = include_str!("../.specs/features/lrm-conformance-closure/spec.md");

/// Every `LRMC-*` id appearing as the first column of a Requirement
/// Traceability table row in spec.md, parsed directly from the live file.
fn spec_table_ids() -> Vec<String> {
    SPEC.lines()
        .filter(|l| l.trim_start().starts_with("| LRMC-"))
        .map(|l| {
            let id = l.trim_start().trim_start_matches('|').split('|').next().unwrap().trim();
            id.to_string()
        })
        .collect()
}

#[test]
fn registry_has_no_duplicate_ids() {
    let mut seen = std::collections::HashSet::new();
    for e in REGISTRY {
        assert!(seen.insert(e.id), "duplicate LRMC-ID in REGISTRY: {}", e.id);
    }
}

#[test]
fn registry_exactly_matches_spec_table_id_set() {
    let spec_ids = spec_table_ids();
    assert!(spec_ids.len() >= 50, "parsed only {} spec.md table rows — the table's shape changed and this guard went blind", spec_ids.len());

    let mut spec_seen = std::collections::HashSet::new();
    for id in &spec_ids {
        assert!(spec_seen.insert(id.clone()), "spec.md's own table has a duplicate row: {id}");
    }

    let registry_ids: std::collections::HashSet<&str> = REGISTRY.iter().map(|e| e.id).collect();

    let missing_from_registry: Vec<&String> =
        spec_ids.iter().filter(|id| !registry_ids.contains(id.as_str())).collect();
    assert!(
        missing_from_registry.is_empty(),
        "spec.md table rows with no REGISTRY entry: {missing_from_registry:?}"
    );

    let orphaned_in_registry: Vec<&str> =
        registry_ids.iter().filter(|id| !spec_seen.contains(&id.to_string())).copied().collect();
    assert!(
        orphaned_in_registry.is_empty(),
        "REGISTRY entries with no matching spec.md table row: {orphaned_in_registry:?}"
    );
}

#[test]
fn out_of_scope_entries_match_spec_out_of_scope_findings() {
    // LRMC-C10-15/17 are the only two findings spec.md's own Out of Scope
    // framing excludes from closure (spec.md's Coverage note names them by
    // id). Any other entry marked OutOfScope, or either of these two NOT
    // marked OutOfScope, is a registry/spec drift.
    for e in REGISTRY {
        let should_be_out_of_scope = e.id == "LRMC-C10-15" || e.id == "LRMC-C10-17";
        let is_out_of_scope = matches!(e.status, Status::OutOfScope);
        assert_eq!(
            is_out_of_scope, should_be_out_of_scope,
            "{}: OutOfScope status mismatch (is={is_out_of_scope}, should_be={should_be_out_of_scope})",
            e.id
        );
    }
}

/// Every `Closed` entry's named test must actually exist — a stale
/// reference to a renamed/deleted test fails this gate, not just a doc
/// claim (T9's own "Done when": "test fails if an ID is missing,
/// duplicated, or names a non-existent test").
#[test]
fn every_closed_entry_names_a_real_test() {
    let root = workspace_root();
    let mut failures = Vec::new();
    for e in REGISTRY {
        let Status::Closed { file, test_fn } = &e.status else { continue };
        let path = root.join(file);
        let Ok(contents) = fs::read_to_string(&path) else {
            failures.push(format!("{}: file `{file}` does not exist", e.id));
            continue;
        };
        if !test_function_exists(&contents, test_fn) {
            failures.push(format!("{}: no `#[test] fn {test_fn}` found in `{file}`", e.id));
        }
    }
    assert!(failures.is_empty(), "closing-test references that don't resolve:\n  {}", failures.join("\n  "));
}

/// True if `contents` has a `#[test]`-attributed `fn <name>` — the
/// attribute may be on the line immediately above the `fn`, or (less
/// commonly in this codebase) share a line via `#[test] fn`. Does not
/// match an `#[ignore]`d test — an ignored test is not a real closing
/// proof (suite_hygiene's own rule against switched-off coverage applies
/// here too).
fn test_function_exists(contents: &str, name: &str) -> bool {
    let needle_owned = format!("fn {name}(");
    let needle = needle_owned.as_str();
    let lines: Vec<&str> = contents.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if !line.contains(needle) {
            continue;
        }
        // Same-line `#[test] fn name(...)`.
        if line.contains("#[test]") {
            return true;
        }
        // Look back over attribute lines immediately above for `#[test]`,
        // stopping at the first non-attribute line; reject if `#[ignore]`
        // appears in that same attribute block.
        let mut saw_test = false;
        let mut saw_ignore = false;
        let mut j = i;
        while j > 0 {
            j -= 1;
            let prev = lines[j].trim();
            if prev.starts_with("#[test]") {
                saw_test = true;
            } else if prev.starts_with("#[ignore") {
                saw_ignore = true;
            } else if prev.starts_with('#') || prev.starts_with("///") || prev.is_empty() {
                // keep scanning past other attributes/doc comments/blank lines
            } else {
                break;
            }
        }
        if saw_test && !saw_ignore {
            return true;
        }
    }
    false
}

#[test]
fn test_function_exists_helper_distinguishes_real_ignored_and_absent() {
    let src = "#[test]\nfn real_one() {}\n\n#[test]\n#[ignore]\nfn ignored_one() {}\n\nfn not_a_test() {}\n";
    assert!(test_function_exists(src, "real_one"));
    assert!(!test_function_exists(src, "ignored_one"), "an #[ignore]d test must not count as a real closing proof");
    assert!(!test_function_exists(src, "not_a_test"), "a fn with no #[test] attribute must not count");
    assert!(!test_function_exists(src, "totally_absent"));
}

#[test]
fn every_id_has_a_non_empty_clause_and_story() {
    for e in REGISTRY {
        assert!(!e.clause.is_empty(), "{}: empty LRM clause", e.id);
        assert!(!e.story.is_empty(), "{}: empty story", e.id);
    }
}
