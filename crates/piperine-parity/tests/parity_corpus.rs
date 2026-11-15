//! Scope: the guard of the parity corpus (requirements PAR-07 and PAR-05 AC 5): every twin under
//! `tests/ngspice/` has a `.cir` and a test function named after it in a `parity_*` target, and no
//! `parity_*` target compares an absolute difference with a numeric literal. Each check is a
//! method returning its failures: empty on the tree, non-empty on text planted by a test.

use std::path::{Path, PathBuf};

/// The two checks, over text handed in.
struct Guard;

impl Guard {
    /// The twin names (every `<name>.msl` but `devices.msl`) that have no `.cir` among `cirs` or no
    /// `fn <name>()` in a target of `targets` (`(file, text)`), each as one message.
    fn unmatched_twins(twins: &[String], cirs: &[String], targets: &[(String, String)]) -> Vec<String> {
        let mut failures = Vec::new();
        for twin in twins.iter().filter(|name| name.as_str() != "devices") {
            if !cirs.contains(twin) {
                failures.push(format!("tests/ngspice/{twin}.msl has no tests/ngspice/{twin}.cir"));
            }
            let signature = format!("fn {twin}()");
            if !targets.iter().any(|(_, text)| text.contains(&signature)) {
                failures.push(format!("tests/ngspice/{twin}.msl has no test function `{twin}` in a parity_* target"));
            }
        }
        failures
    }

    /// Each line of `text` that compares an absolute difference (`.abs()`) with a numeric
    /// literal, as `file:line: text`. A comparison is `<`, `<=`, `>` or `>=` followed by a number.
    fn literal_comparisons(file: &str, text: &str) -> Vec<String> {
        let mut failures = Vec::new();
        for (index, line) in text.lines().enumerate() {
            let Some(at) = line.find(".abs()") else { continue };
            let rest = line[at + ".abs()".len()..].trim_start();
            let after = rest.strip_prefix("<=").or_else(|| rest.strip_prefix(">=")).or_else(|| rest.strip_prefix('<')).or_else(|| rest.strip_prefix('>'));
            let Some(after) = after else { continue };
            if after.trim_start().starts_with(|c: char| c.is_ascii_digit()) {
                failures.push(format!("{file}:{}: {}", index + 1, line.trim()));
            }
        }
        failures
    }
}

/// The tree's own files.
struct Tree;

impl Tree {
    fn ngspice() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/ngspice")
    }

    fn stems(extension: &str) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(Self::ngspice())
            .expect("tests/ngspice exists")
            .map(|entry| entry.expect("a directory entry").path())
            .filter(|path| path.extension().is_some_and(|e| e == extension))
            .filter_map(|path| path.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .collect();
        names.sort();
        names
    }

    /// The `parity_*` targets that compare against ngspice, as `(file name, text)`: every
    /// `tests/parity_*.rs` but this guard.
    fn parity_targets() -> Vec<(String, String)> {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
        let mut targets: Vec<(String, String)> = std::fs::read_dir(directory)
            .expect("tests exists")
            .map(|entry| entry.expect("a directory entry").path())
            .filter_map(|path| {
                let name = path.file_name()?.to_string_lossy().into_owned();
                (name.starts_with("parity_") && name.ends_with(".rs") && name != "parity_corpus.rs").then(|| (name, std::fs::read_to_string(&path).expect("a readable target")))
            })
            .collect();
        targets.sort();
        targets
    }
}

#[test]
fn every_twin_has_a_cir_and_a_test() {
    let twins = Tree::stems("msl");
    assert!(twins.iter().filter(|t| *t != "devices").count() == 6, "the corpus holds six twins: {twins:?}");
    let failures = Guard::unmatched_twins(&twins, &Tree::stems("cir"), &Tree::parity_targets());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn no_parity_target_compares_against_a_literal() {
    let targets = Tree::parity_targets();
    assert!(targets.len() >= 4, "the parity targets are found: {:?}", targets.iter().map(|(n, _)| n).collect::<Vec<_>>());
    let failures: Vec<String> = targets.iter().flat_map(|(file, text)| Guard::literal_comparisons(file, text)).collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_literal_tolerance_in_a_parity_target_is_reported() {
    let planted = "fn x() {\n    assert!((a - b).abs() < 1e-3);\n    assert!((a - b).abs() <= 5.0);\n    assert!((a - b).abs() < tolerance);\n}\n";
    let failures = Guard::literal_comparisons("parity_planted.rs", planted);
    assert_eq!(failures.len(), 2, "{failures:?}");
    assert!(failures[0].starts_with("parity_planted.rs:2:"), "{failures:?}");
    assert!(failures[1].starts_with("parity_planted.rs:3:"), "{failures:?}");
}

#[test]
fn the_guards_report_planted_text() {
    let twins = vec!["alpha".to_string(), "beta".to_string(), "devices".to_string()];
    let cirs = vec!["alpha".to_string()];
    let targets = vec![("parity_op.rs".to_string(), "#[test]\nfn alpha() {}\n".to_string())];
    let failures = Guard::unmatched_twins(&twins, &cirs, &targets);
    assert_eq!(failures.len(), 2, "{failures:?}");
    assert!(failures.iter().any(|f| f.contains("beta.msl has no tests/ngspice/beta.cir")), "{failures:?}");
    assert!(failures.iter().any(|f| f.contains("beta.msl has no test function `beta`")), "{failures:?}");
    assert!(Guard::unmatched_twins(&twins[..1], &cirs, &targets).is_empty());
}
