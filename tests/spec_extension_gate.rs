//! Extension gate (T48, `lrm-conformance-wave3`, PSW3-07, B9): every
//! sanctioned extension `docs/spec/part_iii_conformance.md` §6 lists names a
//! live test — spec.md §0 rule 3 ("An extension is not exempt from proof: it
//! needs the same thing an LRM clause needs — a live test, and a line in the
//! spec"). Same registry+exhaustiveness shape as `tests/lrm_conformance_matrix.rs`
//! (T9): a hardcoded [`REGISTRY`] is cross-checked against the *live* §6
//! table text (`include_str!`, so a docs edit that adds/removes/renames a row
//! is caught, never silently drifting), and every registry entry's named test
//! must actually exist as a live (non-`#[ignore]`d) `#[test]`.
//!
//! Reclaims the four wave-2 targets whose coverage claims were dropped
//! because they prove an extension, not an LRM clause (T7's note in
//! `.specs/features/lrm-conformance-wave2/coverage_exclusions.toml`):
//! `vams_constraint_block.rs`, `vams_param_distributions.rs`,
//! `vams_doc_comments.rs`, `vams_device_port_attrs.rs`.

use std::fs;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

const CONFORMANCE: &str = include_str!("../docs/spec/part_iii_conformance.md");

/// One row of §6's extensions table.
struct Entry {
    /// The Extension column's cell text, verbatim — the cross-check key
    /// against the live table (each row's first `|`-delimited cell).
    extension: &'static str,
    file: &'static str,
    test_fn: &'static str,
}

/// The full registry — one entry per row in §6's table, in the table's own
/// order.
const REGISTRY: &[Entry] = &[
    Entry {
        extension: "`constraint begin … end` with `require`/`var`/`target`/`cover`/`cover cross`, analysis scoping (`@ dc`/`@ tran`/`@ ac`) and `after`/`dur` windows",
        file: "crates/piperine-lang/tests/vams_constraint_block.rs",
        test_fn: "a_scope_group_parses_and_carries_analysis_and_window",
    },
    Entry {
        extension: "`tol gauss/uniform(...) [global]` on parameters (the random-variable catalogue)",
        file: "crates/piperine-lang/tests/vams_param_distributions.rs",
        test_fn: "a_gauss_clause_with_sigma_and_global_parses",
    },
    Entry {
        extension: "`const <name> = <expr>;` at file scope",
        file: "crates/piperine-api/tests/model_design.rs",
        test_fn: "const_reads_a_global_and_reports_none_for_an_unknown_name",
    },
    Entry {
        extension: "`(* rfport, num, z0 *)` attribute schema for S-parameter ports",
        file: "crates/piperine-python/tests/sp_parity.rs",
        test_fn: "python_sp_matches_rust_sp",
    },
    Entry {
        extension: "`(* device = … *)` / `(* port = … *)` attribute schemas",
        file: "crates/piperine-lang/tests/vams_device_port_attrs.rs",
        test_fn: "device_attribute_lowers_into_the_device_schema_when_seeded",
    },
    Entry {
        extension: "`///` doc comments attached to declarations",
        file: "crates/piperine-lang/tests/vams_doc_comments.rs",
        test_fn: "doc_runs_reach_module_param_and_port",
    },
    Entry {
        extension: "`@(initial_step) V(a,b) <+ value;` as a UIC seed (SPICE `.ic`)",
        file: "crates/piperine-codegen/tests/margin_tran_points.rs",
        test_fn: "uic_skips_t0_while_a_computed_op_is_evaluated",
    },
    Entry {
        extension: "`$unconnected` sentinel nets for an unconnected port",
        file: "crates/piperine-lang/tests/vams_unconnected_ports.rs",
        test_fn: "distinct_unconnected_ports_get_distinct_sentinel_nets",
    },
    Entry {
        extension: "`paramset name (module);` (parenthesized form) alongside the LRM's `paramset name module;`",
        file: "crates/piperine-lang/tests/vams_paramset.rs",
        test_fn: "both_paramset_forms_parse_to_the_same_pom",
    },
];

/// Every Extension-column cell in §6's live table, parsed straight out of
/// `part_iii_conformance.md`: scoped to the lines between the `## §6` heading
/// and the next `## ` heading, filtered to `|`-prefixed rows, skipping the
/// header row (`| Extension | Where | Test |`) and the `|---|---|---|`
/// separator.
fn table_extensions() -> Vec<String> {
    let mut in_section = false;
    let mut rows = Vec::new();
    for line in CONFORMANCE.lines() {
        if line.starts_with("## §6") {
            in_section = true;
            continue;
        }
        if in_section && line.starts_with("## ") {
            break;
        }
        if !in_section {
            continue;
        }
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            continue;
        }
        if trimmed.starts_with("| Extension") || trimmed.starts_with("|---") {
            continue;
        }
        let cell = trimmed.trim_start_matches('|').split('|').next().unwrap_or("").trim();
        if !cell.is_empty() {
            rows.push(cell.to_string());
        }
    }
    rows
}

#[test]
fn registry_has_no_duplicate_extensions() {
    let mut seen = std::collections::HashSet::new();
    for e in REGISTRY {
        assert!(seen.insert(e.extension), "duplicate extension in REGISTRY: {}", e.extension);
    }
}

#[test]
fn registry_exactly_matches_the_table() {
    let table = table_extensions();
    assert!(
        table.len() >= 5,
        "parsed only {} §6 table rows — the table's shape changed and this guard went blind",
        table.len()
    );

    let table_set: std::collections::HashSet<&str> = table.iter().map(String::as_str).collect();
    assert_eq!(
        table.len(),
        table_set.len(),
        "part_iii_conformance.md's §6 table has a duplicate Extension row"
    );

    let registry_set: std::collections::HashSet<&str> = REGISTRY.iter().map(|e| e.extension).collect();

    let missing_from_registry: Vec<&str> =
        table_set.iter().filter(|c| !registry_set.contains(*c)).copied().collect();
    assert!(
        missing_from_registry.is_empty(),
        "§6 table rows with no REGISTRY entry (a row was added without a test):\n  {}",
        missing_from_registry.join("\n  ")
    );

    let orphaned_in_registry: Vec<&str> =
        registry_set.iter().filter(|c| !table_set.contains(*c)).copied().collect();
    assert!(
        orphaned_in_registry.is_empty(),
        "REGISTRY entries with no matching §6 table row (stale after a docs edit):\n  {}",
        orphaned_in_registry.join("\n  ")
    );
}

/// True if `contents` has a live (non-`#[ignore]`d) `#[test]`-attributed
/// `fn <name>` — same shape as `lrm_conformance_matrix.rs::test_function_exists`.
fn test_function_exists(contents: &str, name: &str) -> bool {
    let needle_owned = format!("fn {name}(");
    let needle = needle_owned.as_str();
    let lines: Vec<&str> = contents.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if !line.contains(needle) {
            continue;
        }
        if line.contains("#[test]") {
            return true;
        }
        let mut saw_test = false;
        let mut saw_ignore = false;
        let mut j = i;
        while j > 0 {
            j -= 1;
            let prev = lines[j].trim();
            if prev.is_empty() {
                continue;
            }
            if prev.starts_with("#[test]") {
                saw_test = true;
            }
            if prev.starts_with("#[ignore") {
                saw_ignore = true;
            }
            if prev.starts_with('#') || prev.starts_with("//") {
                continue;
            }
            break;
        }
        if saw_test && !saw_ignore {
            return true;
        }
    }
    false
}

/// Every row's named test must actually exist and be live — a stale
/// reference to a renamed/deleted/ignored test fails this gate, not just a
/// doc claim.
#[test]
fn every_row_names_a_real_test() {
    let root = workspace_root();
    let mut failures = Vec::new();
    for e in REGISTRY {
        let path = root.join(e.file);
        let Ok(contents) = fs::read_to_string(&path) else {
            failures.push(format!("{}: file `{}` does not exist", e.extension, e.file));
            continue;
        };
        if !test_function_exists(&contents, e.test_fn) {
            failures.push(format!(
                "{}: no live `#[test] fn {}` found in `{}`",
                e.extension, e.test_fn, e.file
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "extension rows whose named test doesn't resolve:\n  {}",
        failures.join("\n  ")
    );
}
