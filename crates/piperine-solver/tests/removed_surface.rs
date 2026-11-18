//! Structural guards for the surplus the `solver-restructure` feature removed
//! (SR-03): no source of a solver crate (`crates/piperine-*solver*/src`) names
//! a removed item. Each test scans the library sources for one removal-map row;
//! `the_scan_catches_a_planted_name` proves the scan itself can fail.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every `.rs` file under `dir`, recursively.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read dir") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The names of `names` that occur in `text`.
fn names_in(text: &str, names: &[&str]) -> Vec<String> {
    names
        .iter()
        .filter(|n| text.contains(**n))
        .map(|n| n.to_string())
        .collect()
}

/// The names of `words` that occur in `text` as whole identifiers, so a short
/// removed name (`Region`) does not trip on `RegionQueue`-free unrelated words.
fn words_in(text: &str, words: &[&str]) -> Vec<String> {
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    words
        .iter()
        .filter(|w| {
            text.match_indices(**w).any(|(i, m)| {
                let before = text[..i].chars().next_back();
                let after = text[i + m.len()..].chars().next();
                !before.is_some_and(ident) && !after.is_some_and(ident)
            })
        })
        .map(|w| w.to_string())
        .collect()
}

/// `file: name` for every removed name found in a solver crate's `src/`.
fn names_found(names: &[&str]) -> Vec<String> {
    scan(|text| names_in(text, names))
}

/// Like [`names_found`], matching whole identifiers only.
fn words_found(words: &[&str]) -> Vec<String> {
    scan(|text| words_in(text, words))
}

fn scan(matcher: impl Fn(&str) -> Vec<String>) -> Vec<String> {
    let crates = repo_root().join("crates");
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&crates).expect("read crates/") {
        let path = entry.expect("dir entry").path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if name.starts_with("piperine-") && name.contains("solver") && path.join("src").is_dir() {
            rust_files(&path.join("src"), &mut files);
        }
    }
    let mut found = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(&file).expect("read source");
        for n in matcher(&text) {
            found.push(format!("{}: {n}", file.display()));
        }
    }
    found
}

#[test]
fn disto_is_gone() {
    let found = names_found(&[
        "DistoSolver",
        "DistoOptions",
        "DistoResult",
        "Disto2",
        "Disto3",
        "load_disto2",
        "load_disto3",
        "HAS_DISTO2",
        "HAS_DISTO3",
        "NUMERIC_JACOBIAN",
        "SolverDomain::Disto",
        "analyses::disto",
        "fn disto(",
    ]);
    assert!(found.is_empty(), "removed distortion surface still named: {found:#?}");
}

#[test]
fn text_io_is_gone() {
    let found = names_found(&[
        "core::output",
        "SimOutput",
        "StdoutSink",
        "BufferSink",
        "FileTable",
        "McdTable",
        "MemRadix",
        "parse_memory_text",
        "pub io:",
        "io: std::rc::Rc",
    ]);
    assert!(found.is_empty(), "removed text/file output surface still named: {found:#?}");
    let found = words_found(&["Channel", "FileId", "FileError"]);
    assert!(found.is_empty(), "removed text/file output type still named: {found:#?}");
}

#[test]
fn verification_channels_are_gone() {
    let found = names_found(&[
        "core::validation",
        "core::margin",
        "core::coverage",
        "ValidationFinding",
        "ValidationReport",
        "ValidationSeverity",
        "MarginReport",
        "ReportedMargin",
        "MarginKind",
        "CoverageReport",
        "ReportedCoverPoint",
        "ReportedCoverCross",
        "CheckPosture",
        "CoverPosture",
        "RequireScope",
        "EMITS_VALIDATION",
        "validation_reports",
        "evaluate_constraints",
        "margin_report",
        "coverage_report",
        "reset_margin_tracking",
        "consume_validation_reports",
        "margin_reports",
        "coverage_reports",
        "pub validation:",
        "pub margins:",
    ]);
    assert!(found.is_empty(), "removed verification surface still named: {found:#?}");
}

#[test]
fn specify_timing_is_gone() {
    let found = names_found(&[
        "digital::delays",
        "DelayTable",
        "PathDelay",
        "TimingCheckKind",
        "TimingCheckLimit",
        "TimingCheckTable",
        "delay_table",
        "timing_checks",
        "last_change",
        "record_change",
        "check_timing_violations",
        "with_delay_table",
        "consume_timing_check_violations",
    ]);
    assert!(found.is_empty(), "removed specify timing surface still named: {found:#?}");
}

#[test]
fn regions_are_gone() {
    let found = names_found(&[
        "digital::regions",
        "RegionQueue",
        "RegionAction",
        "RegionEvent",
        "RegionOutcome",
        "Region::",
        "pub region:",
        "region: Region",
    ]);
    assert!(found.is_empty(), "removed region surface still named: {found:#?}");
    let found = words_found(&["Region"]);
    assert!(found.is_empty(), "removed Region type still named: {found:#?}");
}

/// HYG-15 AC 1: the two Newton hooks no system overrode, and the comment that named one.
#[test]
fn the_dead_newton_hooks_are_gone() {
    let found = names_found(&["apply_limit", "apply_limiting_reports"]);
    assert!(found.is_empty(), "removed Newton hook still named: {found:#?}");
}

/// HYG-15 AC 2: the `.sp` documentation names the host's `SpPort` values, not the retired PHDL
/// attribute.
#[test]
fn the_phdl_port_text_is_gone() {
    let found = names_found(&["PHDL", "@rfport"]);
    assert!(found.is_empty(), "retired PHDL text still in a solver source: {found:#?}");
}

#[test]
fn simprobe_is_gone() {
    let found = names_found(&["simprobe", "SimprobeError"]);
    assert!(found.is_empty(), "removed $simprobe helper still named: {found:#?}");
}

/// The `Element` contract of `solver-abi-integration`: the one device contract
/// is the `Device` trait (MD-62).
#[test]
fn the_element_contract_is_gone() {
    let found = words_found(&[
        "Element",
        "AnalogDevice",
        "Introspect",
        "ElementCapabilities",
        "ElementCheckpoint",
        "LimitingReport",
        "Invalidation",
        "next_breakpoints",
    ]);
    assert!(found.is_empty(), "removed Element contract still named: {found:#?}");
}

/// The two-phase digital engine of `solver-abi-integration`: the discrete side
/// is the byte-net engine of the device ABI (MD-62).
#[test]
fn the_old_digital_engine_is_gone() {
    let found = words_found(&[
        "Strength",
        "DriverTable",
        "DriverKind",
        "DriverRecord",
        "LogicValue",
        "seq_phase",
        "comb_phase",
        "DigitalState",
        "DigitalTopology",
        "DigitalDevice",
        "DigitalEvent",
        "EventQueue",
        "EventSink",
        "QueueSink",
        "EvalCtx",
        "DigitalPorts",
        "interpolate_a2d_crossing",
    ]);
    assert!(found.is_empty(), "removed old digital engine still named: {found:#?}");
}

/// The tests `solver-abi-integration` removed, by name and the item each was
/// about (`removed.txt`): every line is `name<TAB>item`, and no function of
/// that name is left in a crate of the solver, and every test of the base is
/// still defined or listed (the removal list is the audit trail of design
/// "Test disposition").
#[test]
fn removed_tests_are_listed_with_their_item() {
    let list = repo_root().join(".specs/features/solver-abi-integration/removed.txt");
    let text = std::fs::read_to_string(&list).expect("read removed.txt");
    let (names, malformed) = removed_names(&text);
    assert!(malformed.is_empty(), "removed.txt lines that are not `name<TAB>item`: {malformed:#?}");
    assert!(!names.is_empty(), "removed.txt lists nothing");

    let mut files = Vec::new();
    for name in ["piperine-abi", "piperine-analog-solver", "piperine-digital-solver", "piperine-solver"] {
        rust_files(&repo_root().join("crates").join(name), &mut files);
    }
    let mut left = Vec::new();
    let mut sources = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(&file).expect("read source");
        for name in &names {
            if defines_fn(&text, name) {
                left.push(format!("{}: {name}", file.display()));
            }
        }
        sources.push(text);
    }
    assert!(left.is_empty(), "removed tests still defined:\n{}", left.join("\n"));

    // And every test of the base (43876aec, `base_tests.txt`) is still defined
    // under its name or listed: a rename is a removal and is listed too.
    let base = std::fs::read_to_string(repo_root().join(".specs/features/solver-abi-integration/base_tests.txt"))
        .expect("read base_tests.txt");
    let base: Vec<&str> = base.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    assert!(base.len() > 500, "base_tests.txt lists the base's tests ({})", base.len());
    let unlisted: Vec<&str> = base
        .into_iter()
        .filter(|name| !names.iter().any(|n| n == name))
        .filter(|name| !sources.iter().any(|text| defines_fn(text, name)))
        .collect();
    assert!(unlisted.is_empty(), "base tests gone without a removed.txt line: {unlisted:#?}");
}

/// The names of a removal list, and its lines that are not `name<TAB>item`
/// (a blank line is skipped).
fn removed_names(list: &str) -> (Vec<String>, Vec<String>) {
    let (mut names, mut malformed) = (Vec::new(), Vec::new());
    for line in list.lines().filter(|l| !l.trim().is_empty()) {
        match line.split_once('\t') {
            Some((name, item)) if !name.trim().is_empty() && !item.trim().is_empty() && !item.contains('\t') => {
                names.push(name.trim().to_string());
            }
            _ => malformed.push(line.to_string()),
        }
    }
    (names, malformed)
}

/// Whether `text` defines a function called `name`.
fn defines_fn(text: &str, name: &str) -> bool {
    let needle = format!("fn {name}(");
    text.match_indices(&needle).any(|(i, _)| !text[..i].chars().next_back().is_some_and(|c| c.is_alphanumeric() || c == '_'))
}

#[test]
fn the_scan_catches_a_planted_name() {
    let planted = "pub struct DistoSolver; // planted";
    assert_eq!(names_in(planted, &["DistoSolver", "HAS_DISTO2"]), vec!["DistoSolver"]);
    assert!(names_in("fn clean() {}", &["DistoSolver"]).is_empty());
    assert_eq!(
        words_in("pub use removed::{Region, Channel, FileId, FileError};", &["Region", "Channel", "FileId", "FileError"]),
        vec!["Region", "Channel", "FileId", "FileError"]
    );
    assert!(words_in("struct RegionQueueless; fn channels() {}", &["Region", "Channel"]).is_empty());
    assert_eq!(words_in("impl Element for Resistor {}", &["Element", "Introspect"]), vec!["Element"]);
    assert!(words_in("struct ElementCount; fn elements() {}", &["Element"]).is_empty());
    // The removal list's parser and its scan.
    let (names, malformed) = removed_names("a_test\tthe item\n\nno_item\nb_test\t\n");
    assert_eq!(names, vec!["a_test"]);
    assert_eq!(malformed, vec!["no_item", "b_test\t"]);
    assert!(defines_fn("#[test]\nfn a_test() {}", "a_test"));
    assert!(!defines_fn("fn not_a_test() {}", "a_test"));
}
