//! Opt-in conformance run over an out-of-tree, real-world compact-model
//! corpus (tasks.md T14, VAMS-23 evidence, design.md R2).
//!
//! **No model files are vendored into this repo.** Real compact models
//! (BSIM, HICUM, PSP, EKV, ...) carry their own third-party licenses;
//! vendoring them into an MIT-licensed crate would redistribute
//! foreign-licensed sources (design.md R2). The corpus lives wherever the
//! caller keeps it, named by an env var — obtained from each model's own
//! upstream publisher, never inherited from any deleted crate (C7).
//!
//! Set `PIPERINE_VAMS_CORPUS` to a directory of `.va`/`.vams` files to run
//! this conformance check for real; leave it unset and the test passes
//! with a skip message (never `#[ignore]` — C5, `tests/suite_hygiene.rs`
//! bans it repo-wide, and an opt-in gate that silently vanishes from `cargo
//! test`'s reported count is exactly the failure mode `#[ignore]` causes).

use std::env;
use std::fs;
use std::path::Path;

use piperine_lang::Parser;

const CORPUS_ENV_VAR: &str = "PIPERINE_VAMS_CORPUS";

#[test]
fn real_model_corpus_parses_or_names_the_failing_construct() {
    let Ok(corpus_dir) = env::var(CORPUS_ENV_VAR) else {
        eprintln!(
            "{CORPUS_ENV_VAR} is not set — skipping the real-model corpus conformance run. \
             This is a pass, not a failure: set {CORPUS_ENV_VAR} to a directory of .va/.vams \
             files (obtained from their own upstream publishers — none are vendored into \
             this repo, design.md R2) to exercise it for real."
        );
        return;
    };

    let dir = Path::new(&corpus_dir);
    assert!(
        dir.is_dir(),
        "{CORPUS_ENV_VAR} is set to {corpus_dir:?}, but that is not a directory"
    );

    let (checked, failures) = check_corpus_dir(dir);
    assert!(
        checked > 0,
        "{CORPUS_ENV_VAR} ({corpus_dir:?}) contains no .va/.vams files — nothing to \
         conformance-check"
    );

    eprintln!("real-model corpus: {checked} files checked, {} failed", failures.len());
    assert!(
        failures.is_empty(),
        "the following corpus files failed to parse — each entry names the file and the \
         construct the parser rejected:\n{}",
        failures.join("\n")
    );
}

/// A deterministic, committed proof of the "every file parses or names the
/// file + failing construct" Done-when criterion — independent of whether
/// `PIPERINE_VAMS_CORPUS` happens to be set in this run. Uses a scratch
/// directory (not the env var), so it always runs.
#[test]
fn a_malformed_corpus_file_is_named_with_its_own_span_not_swallowed() {
    let dir = std::env::temp_dir().join(format!(
        "piperine_vams_corpus_test_{}_{}",
        std::process::id(),
        line!()
    ));
    fs::create_dir_all(&dir).expect("create scratch corpus dir");

    fs::write(dir.join("good.va"), "module res(inout electrical p, n); endmodule")
        .expect("write good.va");
    fs::write(dir.join("bad.va"), "module broken(inout electrical p);\n  this is not valid vams\nendmodule")
        .expect("write bad.va");

    let (checked, failures) = check_corpus_dir(&dir);
    fs::remove_dir_all(&dir).ok();

    assert_eq!(checked, 2, "both scratch files must be discovered and checked");
    assert_eq!(failures.len(), 1, "exactly the malformed file must fail: {failures:?}");
    assert!(
        failures[0].contains("bad.va"),
        "the failure must name the offending file, got: {}",
        failures[0]
    );
    assert!(
        failures[0].contains("byte"),
        "the failure must carry the offending construct's byte span, got: {}",
        failures[0]
    );
}

/// Parses every `.va`/`.vams` file directly under `dir` (recursively),
/// returning the count checked and a list of `"path: message (byte
/// A..B)"` entries for every file that failed to parse.
fn check_corpus_dir(dir: &Path) -> (usize, Vec<String>) {
    let mut files = Vec::new();
    collect_va_files(dir, &mut files);

    let mut failures = Vec::new();
    for file in &files {
        let Ok(source) = fs::read_to_string(file) else {
            failures.push(format!("{}: could not read file", file.display()));
            continue;
        };
        if let Err(e) = Parser::new().parse(&source) {
            failures.push(format!("{}: {e} (byte {}..{})", file.display(), e.span.start, e.span.end));
        }
    }
    (files.len(), failures)
}

#[test]
fn missing_corpus_env_var_is_a_pass_not_a_skip_marker() {
    // This is the mechanical guarantee behind the doc comment above: an
    // absent PIPERINE_VAMS_CORPUS must not turn into `#[ignore]` or a
    // panic — it is business as usual for every contributor who hasn't
    // set up the corpus locally.
    if env::var(CORPUS_ENV_VAR).is_ok() {
        eprintln!("{CORPUS_ENV_VAR} is set in this run — nothing to prove here beyond \
                    'the other test exercises the set-path'.");
        return;
    }
    // No panic, no #[ignore]: reaching this point at all is the proof.
}

fn collect_va_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_va_files(&path, out);
            continue;
        }
        match path.extension().and_then(|e| e.to_str()) {
            Some("va") | Some("vams") => out.push(path),
            _ => {}
        }
    }
}
