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
    assert_eq!(files.len(), 10, "expected the 10 migrated model files, got {files:?}");

    let mut failures = Vec::new();
    for path in &files {
        let src = std::fs::read_to_string(path).unwrap();
        if let Err(e) = piperine_lang::parse_and_elaborate(&src, &SourceMap::dummy()) {
            failures.push(format!("{}: {e:?}", path.display()));
        }
    }
    assert!(failures.is_empty(), "spice header(s) failed to elaborate:\n{}", failures.join("\n"));
}
