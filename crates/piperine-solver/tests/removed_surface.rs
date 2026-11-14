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

/// `file: name` for every removed name found in a solver crate's `src/`.
fn names_found(names: &[&str]) -> Vec<String> {
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
        for n in names_in(&text, names) {
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
fn the_scan_catches_a_planted_name() {
    let planted = "pub struct DistoSolver; // planted";
    assert_eq!(names_in(planted, &["DistoSolver", "HAS_DISTO2"]), vec!["DistoSolver"]);
    assert!(names_in("fn clean() {}", &["DistoSolver"]).is_empty());
}
