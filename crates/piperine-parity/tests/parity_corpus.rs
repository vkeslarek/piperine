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
                failures.push(format!("tests/e2e/twins/{twin}.msl has no tests/e2e/twins/{twin}.cir"));
            }
            let signature = format!("fn {twin}()");
            if !targets.iter().any(|(_, text)| text.contains(&signature)) {
                failures.push(format!("tests/e2e/twins/{twin}.msl has no test function `{twin}` in a parity_* target"));
            }
        }
        failures
    }

    /// Each line of `text` that compares an absolute difference (`.abs()`) with a numeric
    /// literal, as `file:line: text`. Every `.abs()` of a line is read, in both operand orders:
    /// `x.abs() < 1e-3` (a comparison operator then a number after it) and `1e-3 > x.abs()` (a
    /// number then a comparison operator before it).
    fn literal_comparisons(file: &str, text: &str) -> Vec<String> {
        let mut failures = Vec::new();
        for (index, line) in text.lines().enumerate() {
            let hit = line.match_indices(".abs()").any(|(at, call)| Self::literal_after(&line[at + call.len()..]) || Self::literal_before(&line[..at]));
            if hit {
                failures.push(format!("{file}:{}: {}", index + 1, line.trim()));
            }
        }
        failures
    }

    /// Whether `rest` starts with a comparison operator followed by a number.
    fn literal_after(rest: &str) -> bool {
        let rest = rest.trim_start();
        let after = rest.strip_prefix("<=").or_else(|| rest.strip_prefix(">=")).or_else(|| rest.strip_prefix('<')).or_else(|| rest.strip_prefix('>'));
        after.is_some_and(|after| after.trim_start().starts_with(|c: char| c.is_ascii_digit()))
    }

    /// Whether `head` (the text before a `.abs()`) has a number as the operand of the last
    /// comparison operator in it: `1e-3 > (a - b)` ends so. An arrow (`->`, `=>`) is no comparison.
    fn literal_before(head: &str) -> bool {
        let bytes = head.as_bytes();
        let Some(at) = (0..bytes.len()).rev().find(|&i| matches!(bytes[i], b'<' | b'>') && !(i > 0 && matches!(bytes[i - 1], b'-' | b'='))) else { return false };
        let operand = head[..at].trim_end().trim_end_matches(['<', '>', '=']).trim_end();
        let token_start = operand.rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '.')).map_or(0, |i| i + 1);
        operand[token_start..].starts_with(|c: char| c.is_ascii_digit())
    }
}

/// The tree's own files.
struct Tree;

impl Tree {
    fn ngspice() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/e2e/twins")
    }

    fn stems(extension: &str) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(Self::ngspice())
            .expect("tests/e2e/twins exists")
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
    assert!(twins.iter().filter(|t| *t != "devices").count() == 16, "the corpus holds 16 twins: {twins:?}");
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
    let planted = concat!(
        "fn x() {\n",
        "    assert!((a - b).abs() < 1e-3);\n",
        "    assert!((a - b).abs() <= 5.0);\n",
        "    assert!((a - b).abs() < tolerance);\n",
        "    assert!(1e-3 > (a - b).abs());\n",
        "    assert!(0.5 >= (a - b).abs());\n",
        "    assert!(c.abs() < d && (a - b).abs() < 1e-6);\n",
        "    assert!(1e-3 > c.abs() || tolerance > d.abs());\n",
        "    assert!(tolerance > (a - b).abs());\n",
        "    let f = |x| -> f64 { x.abs() };\n",
        "    assert!(a1 > (a - b).abs());\n",
        "}\n"
    );
    let failures = Guard::literal_comparisons("parity_planted.rs", planted);
    let lines: Vec<usize> = failures.iter().map(|f| f.split(':').nth(1).expect("a line number").parse().expect("a number")).collect();
    assert_eq!(lines, vec![2, 3, 5, 6, 7, 8], "{failures:?}");
    assert!(failures[0].starts_with("parity_planted.rs:2:"), "{failures:?}");
}

#[test]
fn the_guards_report_planted_text() {
    let twins = vec!["alpha".to_string(), "beta".to_string(), "devices".to_string()];
    let cirs = vec!["alpha".to_string()];
    let targets = vec![("parity_op.rs".to_string(), "#[test]\nfn alpha() {}\n".to_string())];
    let failures = Guard::unmatched_twins(&twins, &cirs, &targets);
    assert_eq!(failures.len(), 2, "{failures:?}");
    assert!(failures.iter().any(|f| f.contains("beta.msl has no tests/e2e/twins/beta.cir")), "{failures:?}");
    assert!(failures.iter().any(|f| f.contains("beta.msl has no test function `beta`")), "{failures:?}");
    assert!(Guard::unmatched_twins(&twins[..1], &cirs, &targets).is_empty());
}
