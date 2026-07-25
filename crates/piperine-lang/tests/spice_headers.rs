//! spice-stdlib SPICE-01/SPICE-02: the builtin `spice` namespace resolves
//! through `headers/spice/`, and every migrated model file parses and
//! elaborates cleanly.

use piperine_lang::SourceMap;

/// SPICE-01: `use spice::diode;` resolves through the builtin header path
/// (no `Piperine.toml`, no package registration — just the source map).
#[test]
fn use_spice_diode_resolves_via_builtin_namespace() {
    let src = "
        use piperine::disciplines;
        use spice::sources;
        use spice::passives;
        use spice::diode;
        mod Top() {
            wire gnd: Electrical; wire vin: Electrical; wire out: Electrical;
            v1: vsrc (.p=vin,.n=gnd) { .dc = 5.0 };
            r1: res  (.p=vin,.n=out) { .r = 1.0e3 };
            d1: dio  (.p=out,.n=gnd) { };
        }
    ";
    let design = piperine_lang::parse_and_elaborate(src, &SourceMap::dummy())
        .expect("use spice::diode; must elaborate through the builtin namespace");
    assert!(design.module("Top").is_some(), "Top module elaborated");
}

/// SPICE-02: every file in `headers/spice/` parses and elaborates cleanly.
#[test]
fn every_spice_header_elaborates() {
    let dir = std::path::Path::new("headers/spice");
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .expect("headers/spice/ must exist")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("phdl"))
        .collect();
    files.sort();
    assert_eq!(files.len(), 12, "expected the 12 migrated model files, got {files:?}");

    let mut failures = Vec::new();
    for path in &files {
        let src = std::fs::read_to_string(path).unwrap();
        if let Err(e) = piperine_lang::parse_and_elaborate(&src, &SourceMap::dummy()) {
            failures.push(format!("{}: {e:?}", path.display()));
        }
    }
    assert!(failures.is_empty(), "spice header(s) failed to elaborate:\n{}", failures.join("\n"));
}

// ── Qualified full-path instance types (no `use` required) ──────────────────

/// A labeled instance may name its module by its full `::`-qualified path
/// (`spice::passives::res`) with NO `use spice::passives;` statement — the
/// resolver auto-loads the prefix file as an implicit `use`.
#[test]
fn full_path_labeled_instance_elaborates_without_use() {
    let src = "
        use piperine::disciplines;
        mod Board() {
            wire a: Electrical; wire gnd: Electrical;
            r1: spice::passives::res(.p = a, .n = gnd) { .r = 1e3 };
        }
    ";
    let design = piperine_lang::parse_and_elaborate(src, &SourceMap::dummy())
        .expect("qualified full-path instance must elaborate without a `use`");
    let board = design.module("Board").expect("Board elaborated");
    let inst = board.instances.iter().find(|i| i.label.as_deref() == Some("r1")).expect("r1 present");
    assert_eq!(inst.module, "res", "the instance's module resolves to the final path segment `res`");
}

/// An unlabeled instance may also use a full path.
#[test]
fn full_path_unlabeled_instance_elaborates_without_use() {
    let src = "
        use piperine::disciplines;
        mod Board() {
            wire a: Electrical; wire gnd: Electrical;
            spice::passives::res(.p = a, .n = gnd) { .r = 1e3 };
        }
    ";
    let design = piperine_lang::parse_and_elaborate(src, &SourceMap::dummy())
        .expect("unlabeled qualified full-path instance must elaborate without a `use`");
    assert!(design.module("Board").is_some());
}

/// The full-path form and an explicit `use` of the same file coexist (the
/// resolver dedups the file load) — mixing the two styles is not an error.
#[test]
fn full_path_and_explicit_use_coexist() {
    let src = "
        use piperine::disciplines;
        use spice::passives;
        mod Board() {
            wire a: Electrical; wire b: Electrical; wire gnd: Electrical;
            r1: res(.p = a, .n = gnd) { .r = 1e3 };
            r2: spice::passives::res(.p = b, .n = gnd) { .r = 2e3 };
        }
    ";
    let design = piperine_lang::parse_and_elaborate(src, &SourceMap::dummy())
        .expect("full-path + explicit use of the same file must coexist");
    assert!(design.module("Board").is_some());
}

/// A full path whose prefix file does not exist fails loud (no silent
/// fallthrough to a bare-name lookup that would mis-resolve).
#[test]
fn full_path_to_missing_file_fails_loud() {
    let src = "
        use piperine::disciplines;
        mod Board() {
            wire a: Electrical; wire gnd: Electrical;
            r1: spice::nonexistent::foo(.p = a, .n = gnd) { };
        }
    ";
    let err = piperine_lang::parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("a full path to a missing file must fail loud");
    let msg = format!("{err:?}");
    assert!(msg.contains("nonexistent"), "error must name the missing file, got: {msg}");
}
