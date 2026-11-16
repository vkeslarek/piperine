//! Scope: the entry surface of `piperine-project` (feature `msl-packages`): the project root is
//! the nearest `Piperine.toml` (PKG-14 AC 1), and the crate holds no release-fetch, plugin or
//! `SourceMap` surface (PKG-15 AC 1); `load_project` builds the design of a package graph, a
//! package error stops it and a language error does not (PKG-14 AC 2 to 4); and the crate reports
//! exactly the codes of the specification's `package` phase (PKG-15 AC 2).

use std::fs;
use std::path::{Path, PathBuf};

#[path = "support/mod.rs"]
#[allow(dead_code)]
mod support;

use piperine_lang::pom::Reflect;
use piperine_project::{find_project_root, load_project};

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

#[test]
fn the_project_root_is_the_nearest_manifest() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().canonicalize().unwrap();
    let app = base.join("ws/app");
    write(&app.join("Piperine.toml"), "");
    write(&app.join("src/deep/x.msl"), "");
    // The directory itself, and any directory below it.
    assert_eq!(find_project_root(&app), Some(app.clone()));
    assert_eq!(find_project_root(&app.join("src/deep")), Some(app.clone()));
    // The nearest of two manifests wins.
    let inner = app.join("vendor/inner");
    write(&inner.join("Piperine.toml"), "");
    write(&inner.join("src/x.msl"), "");
    assert_eq!(find_project_root(&inner.join("src")), Some(inner.clone()));
    assert_eq!(find_project_root(&app.join("vendor")), Some(app.clone()));
    // No manifest on the way up: none. (The temporary directory is searched up to the filesystem
    // root, so a manifest above it would make this fail; there is none.)
    let bare = base.join("bare/deeper");
    fs::create_dir_all(&bare).unwrap();
    assert_eq!(find_project_root(&bare), None);
}

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn the_crate_holds_no_retired_surface() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = fs::read_to_string(root.join("Cargo.toml")).unwrap();
    for name in ["ureq", "sha2", "serde_json", "thiserror", "serde"] {
        assert!(
            !manifest.lines().any(|l| l.trim_start().starts_with(&format!("{name} ")) || l.trim_start().starts_with(&format!("{name}="))),
            "Cargo.toml still depends on `{name}`"
        );
    }
    for gone in ["src/release.rs", "src/source_map.rs", "tests/release.rs"] {
        assert!(!root.join(gone).exists(), "{gone} still exists");
    }
    let mut files = Vec::new();
    sources(&root.join("src"), &mut files);
    assert!(files.len() >= 2, "{files:?}");
    // The crate's sources: the tests spell a retired table (`[plugins]`) to prove it is refused.
    for file in files {
        let text = fs::read_to_string(&file).unwrap().to_lowercase();
        for word in ["plugin", "source_map", "sourcemap", "release", "get_current_project_root"] {
            assert!(!text.contains(word), "{} mentions `{word}`", file.display());
        }
    }
}

#[test]
fn load_project_builds_the_design_of_a_package_graph() {
    let (_tmp, base) = support::workspace();
    let app = support::package(&base.join("app"), "app", "1.0.0", &["util = { path = \"../util\" }"], "use util::Cell;\nmodule Top() { let c = Cell(); }\n");
    support::package(&base.join("util"), "util", "1.0.0", &[], "pub module Cell() {}\n");
    let design = load_project(&app).expect("a clean project");
    let names: Vec<&str> = design.crates().iter().map(|c| c.name()).collect();
    assert_eq!(names, ["app", "util", "std"]);
    assert_eq!(design.root_crate().name(), "app");
    let cell = design.module("Top").unwrap().instance("c").unwrap().module().unwrap();
    assert_eq!(cell.krate().name(), "util");
    let codes: Vec<&str> = design.diagnostics().iter().map(|d| d.diagnostic.code.as_ref()).collect();
    assert!(codes.is_empty(), "{codes:?}");
}

#[test]
fn a_package_error_stops_load_project() {
    let (_tmp, base) = support::workspace();
    let app = base.join("app");
    support::write(&app.join("Piperine.toml"), "[package]\nname = \"app\"\nversion = \"1.0.0\"\nflavour = \"x\"\n");
    support::write(&app.join("src/lib.msl"), "");
    let errors = load_project(&app).err().expect("no design for a package error");
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].code(), "E0171");
}

#[test]
fn a_model_error_is_a_diagnostic_not_a_package_error() {
    let (_tmp, base) = support::workspace();
    let app = support::package(&base.join("app"), "app", "1.0.0", &[], "use nowhere::X;\n");
    let design = load_project(&app).expect("a language error is not a package error");
    let codes: Vec<&str> = design.diagnostics().iter().map(|d| d.diagnostic.code.as_ref()).collect();
    assert_eq!(codes, ["E0161"]);
}

#[test]
fn the_crate_reports_exactly_the_package_phase_codes() {
    use std::collections::BTreeSet;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    sources(&root.join("src"), &mut files);
    let mut reported = BTreeSet::new();
    for file in files {
        let text = fs::read_to_string(file).unwrap();
        let bytes = text.as_bytes();
        // A string literal of `E` and four digits: a code, however it reaches `Diagnostic::new`.
        for (at, _) in text.match_indices("\"E") {
            let code = &text[at + 1..(at + 6).min(text.len())];
            if code.len() == 5 && code[1..].bytes().all(|b| b.is_ascii_digit()) && bytes.get(at + 6) == Some(&b'"') {
                reported.insert(code.to_string());
            }
        }
    }
    // The rows of phase `package` in the specification: the code is the first cell, the phase the last.
    let mut documented = BTreeSet::new();
    for entry in fs::read_dir(root.join("../../docs/spec")).unwrap() {
        let text = fs::read_to_string(entry.unwrap().path()).unwrap();
        for line in text.lines().filter(|l| l.starts_with("| `E")) {
            let cells: Vec<&str> = line.split('|').map(str::trim).collect();
            if cells.len() >= 4 && cells[cells.len() - 2] == "package" {
                documented.insert(cells[1].trim_matches('`').to_string());
            }
        }
    }
    assert!(documented.len() >= 7, "the scrape found only {documented:?}");
    assert_eq!(reported, documented);
}

#[test]
fn the_rc_example_resolves_with_no_diagnostic() {
    // CLI-19 AC 2 (msl-cli T6): `examples/rc` resolves as the package `rc` and its crate has no
    // diagnostic; its library holds the three parts the bench places
    let rc = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/rc");
    let design = load_project(&rc).unwrap_or_else(|e| panic!("examples/rc is no package: {e:?}"));
    let codes: Vec<String> = design.diagnostics().iter().map(|d| format!("{}: {}", d.diagnostic.code, d.diagnostic.message)).collect();
    assert!(codes.is_empty(), "{codes:?}");
    assert_eq!(design.root_crate().name(), "rc");
    for part in ["Resistor", "Capacitor", "Vsource"] {
        assert!(design.module(part).is_some(), "the library holds `{part}`");
    }
    assert!(rc.join("benches/step.msl").is_file(), "the bench of the runs `op` and `step`");
}
