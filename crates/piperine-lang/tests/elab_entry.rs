//! Scope: the entry point of `piperine_lang::elab` (ELAB-01): `Elaborator::run` returns the
//! front-end and type-check diagnostics unchanged and no `Ir`, refuses an unknown or ported top
//! with one diagnostic naming it, and returns a checked `Ir` for a clean top. The M1 corpus (the
//! six twins of `tests/ngspice/`) elaborates with no diagnostic to the IR pinned in
//! `tests/fixtures/elab/*.txt`; elaboration is deterministic, leaves the design unchanged, and
//! every diagnostic carries a code, a span inside its source and, about an instance, its path.

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

use std::path::PathBuf;

use piperine_lang::elab::Elaborator;
use piperine_lang::pom::Design;
use support::Elab;

/// The six twins of `tests/ngspice/` and the module each places.
const TWINS: &[(&str, &str)] = &[("divider", "Divider"), ("diode_iv", "DiodeIv"), ("diode_series", "DiodeSeries"), ("ac_rc", "AcRc"), ("tran_rc", "TranRc"), ("isrc_r", "IsrcR")];

/// Files read from disk by the tests of this target.
struct Disk;

impl Disk {
    fn twin(file: &str) -> Design {
        let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "tests", "ngspice", &format!("{file}.msl")].iter().collect();
        Design::builder().root_file(path).build()
    }

    fn read(parts: &[&str]) -> String {
        let path: PathBuf = std::iter::once(env!("CARGO_MANIFEST_DIR")).chain(parts.iter().copied()).collect();
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }
}

#[test]
fn front_end_diagnostics_are_returned_without_an_ir() {
    // The design itself holds a diagnostic (a malformed param): handed back as it is.
    let design = Elab::design("module Top() { x: f64 = ; }");
    assert!(!design.diagnostics().is_empty(), "the source must hold a diagnostic");
    let err = Elaborator::new(&design).run("Top").expect_err("no Ir for a design with diagnostics");
    assert_eq!(err, design.diagnostics().to_vec());

    // The design is clean, the type check is not: its diagnostic comes back, and no Ir.
    let design = Elab::design("module Top() { x: f64 = true; }");
    assert!(design.diagnostics().is_empty(), "{:?}", Elab::codes(design.diagnostics()));
    let err = Elaborator::new(&design).run("Top").expect_err("a bool default of an f64 param");
    assert_eq!(Elab::codes(&err), ["P0002"]);
}

#[test]
fn an_unknown_or_ported_top_is_one_diagnostic() {
    let design = Elab::design("module Top() { }");
    let err = Elaborator::new(&design).run("Missing").expect_err("no such module");
    assert_eq!(err.len(), 1);
    assert!(err[0].diagnostic.message.contains("Missing"), "{}", err[0].diagnostic.message);

    let src = "module Top(a: Electrical) { }";
    let design = Elab::design(src);
    assert!(design.diagnostics().is_empty(), "{:?}", Elab::codes(design.diagnostics()));
    let err = Elaborator::new(&design).run("Top").expect_err("a top with a port");
    assert_eq!(err.len(), 1);
    assert_eq!(err[0].diagnostic.code, "P0001");
    assert!(err[0].diagnostic.message.contains("Top"), "{}", err[0].diagnostic.message);
    let span = err[0].diagnostic.span();
    assert_eq!(&src[span.start..span.end], "Top", "the diagnostic sits on the module's name");
}

#[test]
fn an_empty_top_is_one_body_and_one_instance() {
    let ir = Elab::ir("module Top() { }", "Top");
    assert_eq!(ir.bodies().count(), 1);
    assert_eq!(ir.instances().count(), 1);
    assert_eq!(ir.top().path(), "Top");
}

#[test]
fn the_twins_elaborate_without_diagnostics() {
    for (file, top) in TWINS {
        let design = Disk::twin(file);
        assert!(design.diagnostics().is_empty(), "{file}: {:?}", Elab::codes(design.diagnostics()));
        let ir = Elaborator::new(&design).run(top).unwrap_or_else(|d| panic!("{file}: {d:?}"));
        assert_eq!(ir.top().path(), *top, "{file}");
    }
}

#[test]
fn a_twin_elaborates_to_a_checked_ir() {
    for (file, top) in TWINS {
        let design = Disk::twin(file);
        let ir = Elaborator::new(&design).run(top).unwrap_or_else(|d| panic!("{file}: {d:?}"));
        assert!(ir.check().is_empty(), "{file}: {:?}", ir.check());
        let golden = Disk::read(&["tests", "fixtures", "elab", &format!("{file}.txt")]);
        assert_eq!(ir.print_structural(), golden, "{file}: the printed IR differs from tests/fixtures/elab/{file}.txt");
    }
    // `r#out` is a held node (§16 H1): its initial potential is 0 and no other node has one.
    let design = Disk::twin("tran_rc");
    let ir = Elaborator::new(&design).run("TranRc").unwrap_or_else(|d| panic!("{d:?}"));
    let out = ir.node_at("TranRc.out").expect("the node `out`");
    assert_eq!(out.init().map(|r| r.get()), Some(0.0));
    assert_eq!(ir.node_at("TranRc.vin").expect("the node `vin`").init(), None);
}

#[test]
fn the_design_is_unchanged_by_elaboration() {
    for (file, top) in TWINS {
        let design = Disk::twin(file);
        let before = design.dump();
        let diagnostics = design.diagnostics().to_vec();
        Elaborator::new(&design).run(top).unwrap_or_else(|d| panic!("{file}: {d:?}"));
        assert_eq!(design.dump(), before, "{file}");
        assert_eq!(design.diagnostics(), diagnostics.as_slice(), "{file}");
    }
}

#[test]
fn elaborating_twice_prints_the_same_text() {
    for (file, top) in TWINS {
        let design = Disk::twin(file);
        let first = Elaborator::new(&design).run(top).unwrap_or_else(|d| panic!("{file}: {d:?}"));
        let second = Elaborator::new(&design).run(top).unwrap_or_else(|d| panic!("{file}: {d:?}"));
        assert_eq!(first.print_structural(), second.print_structural(), "{file}");
        assert_eq!(first.to_string(), second.to_string(), "{file}: the full text, origins included");
    }
}

#[test]
fn device_bodies_match_the_ir_goldens() {
    let devices = Disk::read(&["..", "..", "tests", "ngspice", "devices.msl"]);
    let top = "mod devices;\nuse devices::*;\nmodule Top() {\n    signal a: Electrical;\n    let r = Resistor { r: 1k }(a, gnd);\n    let c = Capacitor { c: 1e-12 }(a, gnd);\n    let v = Vsource { dc: 1.0 }(a, gnd);\n    let i = Isource { }(a, gnd);\n    let d = Diode { }(a, gnd);\n}\n";
    let design = Design::builder().source("top.msl", top).source("devices.msl", devices).build();
    assert!(design.diagnostics().is_empty(), "{:?}", Elab::codes(design.diagnostics()));
    let ir = Elaborator::new(&design).run("Top").unwrap_or_else(|d| panic!("{d:?}"));
    assert!(ir.check().is_empty(), "{:?}", ir.check());
    let text = ir.print_structural();
    let blocks = |text: &str| -> std::collections::BTreeMap<String, String> {
        let mut out = std::collections::BTreeMap::new();
        let mut head = String::new();
        for line in text.lines() {
            if line.starts_with("body ") || line.starts_with("fn ") || line.starts_with("nature ") {
                head = line.split('(').next().unwrap_or(line).to_string();
            } else if !line.starts_with("  ") {
                head.clear();
            }
            if !head.is_empty() {
                let entry: &mut String = out.entry(head.clone()).or_default();
                entry.push_str(line);
                entry.push('\n');
            }
        }
        out
    };
    let ours = blocks(&text);
    for file in ["resistor", "capacitor", "vsource", "isource", "diode"] {
        let golden = Disk::read(&["..", "piperine-ir", "tests", "golden", &format!("{file}.txt")]);
        let theirs = blocks(&golden);
        assert!(!theirs.is_empty(), "{file}");
        for (head, block) in &theirs {
            assert_eq!(ours.get(head), Some(block), "`{head}` of {file}.txt");
        }
    }
}

#[test]
fn every_diagnostic_has_code_span_and_path() {
    // One negative source per rule of ELAB-02 to ELAB-06 that names an instance or not: each
    // diagnostic has a code, a span inside the source and, about an instance, its path.
    let leaf = "module Leaf() { r: f64 where r > 0.0; }\n";
    let cases: [(&str, String, Option<&str>); 6] = [
        ("E0317", format!("{leaf}module Top() {{ let r1 = Leaf {{ r: -1.0 }}(); }}"), Some("Top.r1.r")),
        ("E0319", "module Leaf() { r: f64; }\nmodule Top() { let l = Leaf { }(); }".to_string(), Some("Top.l")),
        ("E0341", "module Loop() { let again = Loop { }(); }\nmodule Top() { let l = Loop { }(); }".to_string(), Some("Top.l.again")),
        ("P0001", "module M(p: Electrical, n: Electrical) { }\nanalog for M { if v(p) > 0.0 { i(p, n) +> 1.0; } }\nmodule Top() { signal s: Electrical; let m = M { }(s, gnd); }".to_string(), None),
        ("E0241", "module M(p: Electrical) { }\nanalog for M { i(p, p) +> 1.0; }\nmodule Top() { signal s: Electrical; let m = M { }(s); }".to_string(), None),
        ("E0415", "module M(p: Electrical, n: Electrical) { }\nanalog for M { i(p, n) +> 1.0; v(p, n) +> 1.0; }\nmodule Top() { signal s: Electrical; let m = M { }(s, gnd); }".to_string(), None),
    ];
    for (code, src, path) in &cases {
        let err = Elab::run(src, "Top").expect_err(code);
        assert!(err.iter().any(|d| d.diagnostic.code == *code), "{code}: {:?}", Elab::codes(&err));
        for d in &err {
            let span = d.diagnostic.span();
            assert!(span.start < span.end && span.end <= src.len(), "{code}: a span inside the source, got {span:?}");
            assert!(!d.diagnostic.code.is_empty() && !d.diagnostic.message.is_empty(), "{code}");
        }
        if let Some(path) = path {
            let d = err.iter().find(|d| d.diagnostic.code == *code).expect("the diagnostic");
            assert!(d.diagnostic.message.contains(path), "{code}: `{}` names no `{path}`", d.diagnostic.message);
        }
    }
}
