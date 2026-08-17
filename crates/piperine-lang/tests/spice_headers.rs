//! The builtin `spice` stdlib: `` `include "spice/<file>.va" `` resolves in
//! any project with no dependency declaration, and every shipped model file
//! parses and elaborates cleanly.
//!
//! **Retargeted from PHDL (this file's original form tested `use
//! spice::passives;` path imports, `::`-qualified full-path instance types,
//! and `Design::enum_()`).** VAMS has no module-path import mechanism at
//! all — `` `include`` is a textual directive, so there is no per-segment
//! path to resolve and no full-path instance type to write; and PHDL's
//! `enum` value layer went with the parser (MD-45). Those sub-scenarios are
//! genuinely deleted language, not gaps to route around. What survives —
//! and is what actually matters about this stdlib — is that the builtin
//! namespace resolves with zero project setup and that the shipped models
//! are all valid Verilog-AMS.

use piperine_lang::SourceMap;

const SPICE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/headers/spice");

/// The `spice` namespace is seeded by `SourceMap` itself — no
/// `Piperine.toml`, no dependency, no package registration.
#[test]
fn include_spice_header_resolves_via_the_builtin_namespace() {
    let src = "`include \"disciplines.vams\"\n\
               `include \"spice/passives.va\"\n\
               module Top();\n\
               \x20 electrical vin, gnd;\n\
               \x20 res #(.r(1.0e3)) r1 (.p(vin), .n(gnd));\n\
               endmodule\n";
    let design = piperine_lang::parse_and_elaborate(src, &SourceMap::dummy())
        .expect("`include \"spice/…\" must resolve through the builtin namespace");

    let top = design.module("Top").expect("Top elaborated");
    assert_eq!(
        top.instances.iter().find(|i| i.label.as_deref() == Some("r1")).map(|i| i.module.as_str()),
        Some("res"),
        "the instance must resolve to the stdlib `res` module the include brought in"
    );
}

/// Every shipped model file is valid Verilog-AMS. Each is elaborated on its
/// own (they `` `include`` whatever they need), so a syntax or semantic
/// regression in any one of them fails here naming that file — the guard
/// that keeps the stdlib from rotting silently.
#[test]
fn every_spice_header_elaborates() {
    let dir = std::path::Path::new(SPICE_DIR);
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .expect("headers/spice/ must exist")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("va"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "headers/spice/ must ship model files, found none");

    let failures: Vec<String> = files
        .iter()
        .filter_map(|path| {
            let src = std::fs::read_to_string(path).ok()?;
            piperine_lang::parse_and_elaborate(&src, &SourceMap::dummy())
                .err()
                .map(|e| format!("{}: {e:?}", path.display()))
        })
        .collect();

    assert!(
        failures.is_empty(),
        "{} of {} spice header(s) failed to elaborate:\n{}",
        failures.len(),
        files.len(),
        failures.join("\n")
    );
}

/// An `` `include`` naming a file the `spice` namespace does not hold fails
/// loud, naming the path — never a silent skip that would leave every
/// instance of a missing model mysteriously unresolved later.
#[test]
fn include_of_a_missing_spice_header_fails_loud() {
    let src = "`include \"spice/nonexistent.va\"\n\
               module Board();\n\
               endmodule\n";
    let err = piperine_lang::parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("a missing spice header must fail loud");
    let msg = format!("{err:?}");
    assert!(msg.contains("nonexistent"), "the error must name the missing file, got: {msg}");
}
