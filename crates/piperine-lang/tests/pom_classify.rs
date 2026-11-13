//! Scope: the classifications the resolver makes over names (POM-09 and the
//! behavior-block binding): which module a behavior block belongs to and the
//! errors `E0401`, `E0402` and `E0403` when it does not; what a declared type
//! makes of a port (`PortKind`) and of a struct (the bundle flag).

#[path = "support/pom.rs"]
mod support;

use piperine_lang::pom::{Design, DesignDiagnostic, PortKind, Reflect};
use support::build;

const PRELUDE: &str = include_str!("fixtures/pom/prelude.msl");

/// A design of `src` with the fixture prelude beside it.
fn with_prelude(src: &str) -> Design {
    Design::builder().prelude("prelude.msl", PRELUDE).source("top.msl", src).build()
}

fn with_code<'a>(design: &'a Design, code: &str) -> Vec<&'a DesignDiagnostic> {
    design.diagnostics().iter().filter(|d| d.diagnostic.code == code).collect()
}

/// The authored text of the primary span of a diagnostic.
fn primary_text<'a>(design: &'a Design, d: &DesignDiagnostic) -> &'a str {
    let file = &design.files()[d.file.index()];
    &file.text()[d.diagnostic.span().range()]
}

#[test]
fn behavior_without_module_is_e0401() {
    let src = "struct NotAModule {}\nanalog for Nope { }\nanalog for NotAModule { }\ndigital for b::C { }\n";
    let design = build(src);
    let errors = with_code(&design, "E0401");
    let at: Vec<_> = errors.iter().map(|d| primary_text(&design, d)).collect();
    assert_eq!(at, ["Nope", "NotAModule", "b::C"]);
    // The blocks stay in the tree with no module, and each is one error, not two.
    let blocks: Vec<_> = design.root().children().into_iter().filter_map(|c| c.as_behavior()).collect();
    assert_eq!(blocks.len(), 3);
    assert!(blocks.iter().all(|b| b.module().is_none()));
    assert_eq!(support::codes(&design).iter().filter(|c| c.as_str() == "E0161").count(), 0, "{:?}", design.diagnostics());
}

#[test]
fn behavior_for_extern_module_is_e0402() {
    let design = build("extern \"osdi\" module Bsim4(d: Electrical) { l: f64 = 1u; }\nanalog for Bsim4 { }\n");
    let errors = with_code(&design, "E0402");
    assert_eq!(errors.len(), 1, "{:?}", design.diagnostics());
    assert_eq!(primary_text(&design, errors[0]), "Bsim4");
    // The block is still linked: the module exists, it is just not allowed to have one.
    let block = design.root().children().into_iter().find_map(|c| c.as_behavior()).unwrap();
    assert_eq!(block.module(), design.module("Bsim4"));
    assert!(with_code(&design, "E0401").is_empty());
}

#[test]
fn behavior_names_mismatch_is_e0403() {
    let src = "\
module Short<N: u32>() {}
module Plain() {}
analog<N> for Short<N> { }
analog<A> for Short<N> { }
analog<N, M> for Short<N, M> { }
analog<N> for Short<A> { }
analog for Short { }
analog for Plain { }
analog<N> for Plain<N> { }
";
    let design = build(src);
    let errors = with_code(&design, "E0403");
    let heads: Vec<_> = errors.iter().map(|d| primary_text(&design, d)).collect();
    assert_eq!(heads, ["analog<A> for Short<N>", "analog<N, M> for Short<N, M>", "analog<N> for Short<A>", "analog for Short", "analog<N> for Plain<N>"]);
    // The first block and the one for a module without generics are fine.
    assert_eq!(errors.len(), 5);
    assert!(with_code(&design, "E0401").is_empty() && with_code(&design, "E0161").is_empty(), "{:?}", design.diagnostics());
}

#[test]
fn behavior_for_module_in_other_source_is_e0401() {
    let top = "mod b;\nuse b::M;\nanalog for M { }\nanalog for b::M { }\n";
    let design = Design::builder().source("top.msl", top).source("b.msl", "pub module M() {}\n").build();
    let errors = with_code(&design, "E0401");
    let at: Vec<_> = errors.iter().map(|d| primary_text(&design, d)).collect();
    assert_eq!(at, ["M", "b::M"]);
    // The module is found; it is in the wrong file, so no block is linked.
    let m = design.find("b::M").and_then(|d| d.as_module()).expect("module M in b.msl");
    assert!(m.behaviors().is_empty());
    assert!(design.root().children().into_iter().filter_map(|c| c.as_behavior()).all(|b| b.module().is_none()));
    // Beside its module, in its own file, a block is fine.
    let own = Design::builder().source("top.msl", "mod b;\n").source("b.msl", "pub module M() {}\nanalog for M { }\n").build();
    assert!(with_code(&own, "E0401").is_empty(), "{:?}", own.diagnostics());
    assert_eq!(own.find("b::M").and_then(|d| d.as_module()).unwrap().behaviors().len(), 1);
}

#[test]
fn port_kinds() {
    let src = "\
struct Pair { a: Electrical, b: Electrical }
struct Val { x: f64 }
enum Mode { A, B }
module M<N: Nature, T: Repr>(
    p: Electrical, q: [Bit; 4], r: Pair, in s: Pair, t: flip Pair, u: [flip Pair; 2],
    f: f64, o: Option<f64>, v: Val, m: Mode, g: N, ga: [N; 2], h: T, z: Nope,
) {}
";
    let design = with_prelude(src);
    let m = design.module("M").unwrap();
    let kinds: Vec<(&str, PortKind)> = m.ports().iter().map(|p| (p.name().unwrap(), p.kind())).collect();
    let expected = [
        ("p", PortKind::Carrier),
        ("q", PortKind::Carrier),
        ("r", PortKind::Bundle),
        ("s", PortKind::Bundle),
        ("t", PortKind::Bundle),
        ("u", PortKind::Bundle),
        ("f", PortKind::ValueInput),
        ("o", PortKind::ValueInput),
        ("v", PortKind::ValueInput),
        ("m", PortKind::ValueInput),
        // A generic bounded by `Nature` is a carrier; any other generic is a value.
        ("g", PortKind::Carrier),
        ("ga", PortKind::Carrier),
        ("h", PortKind::ValueInput),
        // Nothing declares `Nope`.
        ("z", PortKind::Unknown),
    ];
    assert_eq!(kinds, expected);
}

#[test]
fn struct_with_carrier_field_is_bundle() {
    let src = "\
struct Cell { in wl: Bit, bl: Electrical }
struct Arr { xs: [Electrical; 2] }
struct Nested { c: Cell, n: u8 }
struct Deep { inner: Nested }
struct Plain { x: f64, y: Option<f64> }
struct Empty {}
struct Generic<N: Nature> { a: N }
struct Dangling { q: Nope }
";
    let design = with_prelude(src);
    let bundle = |name: &str| design.find(name).and_then(|d| d.as_struct()).unwrap_or_else(|| panic!("struct {name}: {:?}", design.diagnostics())).is_bundle();
    for yes in ["Cell", "Arr", "Nested", "Deep", "Generic"] {
        assert!(bundle(yes), "`{yes}` is a bundle");
    }
    for no in ["Plain", "Empty", "Dangling"] {
        assert!(!bundle(no), "`{no}` is not a bundle");
    }
}

#[test]
fn self_referential_struct_bundle_check_ends() {
    let src = "\
struct Me { next: Me, x: f64 }
struct A { b: B }
struct B { a: A }
struct C { d: D }
struct D { c: C, wire: Electrical }
struct E { c: C }
";
    let design = with_prelude(src);
    let bundle = |name: &str| design.find(name).and_then(|d| d.as_struct()).unwrap().is_bundle();
    // A cycle with no carrier in it is no bundle.
    assert!(!bundle("Me") && !bundle("A") && !bundle("B"));
    // A cycle with a carrier on it is a bundle all around, whichever member is met first.
    assert!(bundle("C") && bundle("D") && bundle("E"));
}
