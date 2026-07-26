//! Part VII §16 is a contract, not an intention (P6/CLN-19).
//!
//! The spec's failure-rule table carries an **Enforcement** column: each row
//! either names the test that trips it or is explicitly marked *not yet
//! enforced*. This guard reads that table out of the spec file itself and fails
//! when a row is neither — so a new normative rule cannot be added without
//! saying, in the row, whether anything checks it.
//!
//! It also verifies that every test a row names actually exists, which kills
//! the failure mode where a rule points at a renamed or deleted test and the
//! table keeps looking green.

use std::fs;
use std::path::PathBuf;

const SPEC: &str = include_str!("../../../docs/spec/part_vii_solver.md");

/// One row of the §16 table.
struct Rule {
    section: String,
    rule: String,
    enforcement: String,
}

/// The §16 table, parsed out of the spec.
fn rules() -> Vec<Rule> {
    let mut rows = Vec::new();
    let mut in_table = false;
    for line in SPEC.lines() {
        if line.starts_with("| Section | Rule | Failure") {
            in_table = true;
            continue;
        }
        if in_table {
            if !line.starts_with("| §") {
                if line.starts_with('#') || line.trim() == "---" {
                    break;
                }
                continue;
            }
            let cells: Vec<&str> = line.split('|').map(str::trim).collect();
            // "" | section | rule | failure | enforcement | ""
            assert_eq!(
                cells.len(),
                6,
                "§16 row must have four columns (section, rule, failure, enforcement): {line}"
            );
            rows.push(Rule {
                section: cells[1].to_string(),
                rule: cells[2].to_string(),
                enforcement: cells[4].to_string(),
            });
        }
    }
    assert!(
        rows.len() >= 16,
        "parsed only {} §16 rows — the table's shape changed and this guard went blind",
        rows.len()
    );
    rows
}

/// The workspace root (this crate is `crates/piperine-solver`).
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// Every `file.rs::test_name` mentioned in an enforcement cell.
fn named_tests(enforcement: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for token in enforcement.split(|c: char| c.is_whitespace() || c == ',' || c == '`') {
        let token = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '_' && c != '.' && c != ':' && c != '/');
        if let Some((file, test)) = token.split_once("::") {
            if file.ends_with(".rs") && !test.is_empty() {
                found.push((file.to_string(), test.to_string()));
            }
        }
    }
    found
}

#[test]
fn every_failure_rule_is_enforced_or_marked() {
    let unaccounted: Vec<String> = rules()
        .iter()
        .filter(|r| {
            let marked = r.enforcement.contains("not yet enforced");
            let bound = !named_tests(&r.enforcement).is_empty()
                // A row may point at a whole suite (`root: sens.rs`) when the
                // rule is covered by several of its cases.
                || r.enforcement.contains(".rs");
            !marked && !bound
        })
        .map(|r| format!("{} — {}", r.section, r.rule))
        .collect();

    assert!(
        unaccounted.is_empty(),
        "§16 rows that neither name an enforcing test nor say `not yet enforced`:\n  {}",
        unaccounted.join("\n  ")
    );
}

#[test]
fn every_named_enforcement_test_exists() {
    let root = workspace_root();
    // Where a bare file name can live: the solver's own suite, the codegen
    // suite, the plugin suite, or the root host suite.
    let search_dirs = [
        root.join("crates/piperine-solver/tests"),
        root.join("crates/piperine-codegen/tests"),
        root.join("crates/piperine-plugin/tests"),
        root.join("tests"),
    ];

    let mut missing = Vec::new();
    for rule in rules() {
        for (file, test) in named_tests(&rule.enforcement) {
            let base = file.rsplit('/').next().unwrap_or(&file);
            let found = search_dirs.iter().any(|dir| {
                fs::read_to_string(dir.join(base))
                    .map(|text| text.contains(&format!("fn {test}(")))
                    .unwrap_or(false)
            });
            if !found {
                missing.push(format!("{} names {base}::{test}, which does not exist", rule.section));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "§16 enforcement entries pointing at tests that are gone (rename or fix the row):\n  {}",
        missing.join("\n  ")
    );
}
