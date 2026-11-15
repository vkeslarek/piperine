//! Modules, params, nodes and placements of `piperine_lang::types` (TYP-03): params of `f64` and
//! `bool` with their defaults and `where`, ports and signals as nodes of a conservative nature,
//! `gnd`, node initial values, and the checks on a placement (count, nature, param values). Every
//! source declares its own natures: the support design has an empty standard library.

#[path = "support/pom.rs"]
mod support;

use piperine_lang::pom::{DeclId, Design, DesignDiagnostic, ExprId, Reflect};
use piperine_lang::types::{Callee, Ty, Types};

const NATURES: &str = "nature E { potential: V { access: v, units: \"V\", abstol: 1u }, flow: I { access: i, units: \"A\", abstol: 1p } }\n\
nature T { potential: Tp { access: temp, units: \"K\", abstol: 1m }, flow: P { access: pwr, units: \"W\", abstol: 1n } }\n\
module Resistor(p: E, n: E) { r: f64; }\n";

fn design_of(src: &str) -> Design {
    let design = support::build(src);
    assert!(design.diagnostics().is_empty(), "the source must parse and resolve: {:?}\n{src}", support::codes(&design));
    design
}

/// The diagnostics of checking `Top` in `NATURES` followed by `body`, and the source.
fn diagnostics(body: &str) -> (String, Vec<DesignDiagnostic>) {
    let src = format!("{NATURES}{body}");
    let design = design_of(&src);
    let err = Types::check(&design, "Top").expect_err("expected diagnostics");
    (src, err)
}

fn codes_of(diagnostics: &[DesignDiagnostic]) -> Vec<String> {
    diagnostics.iter().map(|d| d.diagnostic.code.to_string()).collect()
}

fn snippet<'a>(src: &'a str, d: &DesignDiagnostic) -> &'a str {
    &src[d.diagnostic.span().range()]
}

/// The expression of the design whose source text is exactly `text` (the first one).
fn expr_of(design: &Design, src: &str, text: &str) -> ExprId {
    (0..design.expr_count()).map(|i| ExprId(i as u32)).find(|e| &src[design.expr_node(*e).span.range()] == text).unwrap_or_else(|| panic!("no expression `{text}`"))
}

#[test]
fn params_are_f64_or_bool() {
    let src = format!("{NATURES}module Top() {{ a: f64 = 1.0; b: bool = true; }}");
    let design = design_of(&src);
    let types = Types::check(&design, "Top").expect("f64 and bool params are in the subset");
    assert_eq!(types.ty(expr_of(&design, &src, "1.0")), Some(&Ty::F64));
    assert_eq!(types.ty(expr_of(&design, &src, "true")), Some(&Ty::Bool));

    // Any other type is outside the subset.
    for ty in ["u32", "Option<f64>", "(f64, f64)"] {
        let (src, d) = diagnostics(&format!("module Top() {{ a: {ty}; }}"));
        assert_eq!(codes_of(&d), ["P0001"], "{ty}");
        assert!(snippet(&src, &d[0]).starts_with(ty.split('<').next().unwrap_or(ty)), "{ty}: {}", snippet(&src, &d[0]));
    }
}

#[test]
fn a_default_of_the_wrong_type_is_rejected() {
    let (src, d) = diagnostics("module Top() { a: f64 = true; }");
    assert_eq!(codes_of(&d), ["P0002"]);
    assert_eq!(snippet(&src, &d[0]), "true");
    let message = &d[0].diagnostic.message;
    assert!(message.contains("f64") && message.contains("bool"), "{message}");

    let (src, d) = diagnostics("module Top() { a: bool = 1.0; }");
    assert_eq!(snippet(&src, &d[0]), "1.0");
}

#[test]
fn a_where_that_is_not_bool_is_e0318() {
    let (src, d) = diagnostics("module Top() { a: f64 = 1.0 where a + 1.0; }");
    assert_eq!(codes_of(&d), ["E0318"]);
    assert_eq!(snippet(&src, &d[0]), "a + 1.0");

    // A `bool` condition checks.
    let src = format!("{NATURES}module Top() {{ a: f64 = 1.0 where a > 0.0; }}");
    let design = design_of(&src);
    Types::check(&design, "Top").expect("a bool where");
}

#[test]
fn ports_and_signals_are_nodes_and_gnd_fits_any() {
    let src = format!("{NATURES}module Top(a: E, b: T) {{ signal s: E; let r1 = Resistor {{ r: 1.0 }}(a, gnd); let r2 = Resistor {{ r: 1.0 }}(s, gnd); }}");
    let design = design_of(&src);
    let types = Types::check(&design, "Top").expect("ports, signals and gnd are nodes");
    let nature = design.find("E").expect("nature E").id();
    assert_eq!(types.ty(expr_of(&design, &src, "a")), Some(&Ty::Node(nature)));
    assert_eq!(types.ty(expr_of(&design, &src, "s")), Some(&Ty::Node(nature)));
    assert_eq!(types.ty(expr_of(&design, &src, "gnd")), Some(&Ty::AnyNode));
    // `gnd` fits a node of the other nature as well.
    let src = format!("{NATURES}module Top(b: T) {{ let r1 = Resistor {{ r: 1.0 }}(gnd, gnd); }}");
    let design = design_of(&src);
    Types::check(&design, "Top").expect("gnd of any nature");

    // A discrete nature or a value input is not a conservative node: refused by name.
    let (_, d) = diagnostics("nature Bit2 { value: bool }\nmodule Top(a: Bit2) { }");
    assert_eq!(codes_of(&d), ["P0001"]);
    let (_, d) = diagnostics("module Top(a: f64) { }");
    assert_eq!(codes_of(&d), ["P0001"]);
}

#[test]
fn a_node_init_that_is_not_f64_is_e0210() {
    let (src, d) = diagnostics("module Top() { signal s: E = true; }");
    assert_eq!(codes_of(&d), ["E0210"]);
    assert_eq!(snippet(&src, &d[0]), "true");

    let src = format!("{NATURES}module Top() {{ signal s: E = 0.5; }}");
    let design = design_of(&src);
    Types::check(&design, "Top").expect("an f64 initial value");
}

#[test]
fn a_placement_checks_nodes_count_and_params() {
    let src = format!("{NATURES}module Top(a: E, b: E) {{ let r1 = Resistor {{ r: 1k }}(a, b); }}");
    let design = design_of(&src);
    let types = Types::check(&design, "Top").expect("a well-formed placement");
    let placement = expr_of(&design, &src, "Resistor { r: 1k }(a, b)");
    let resistor: DeclId = design.module("Resistor").expect("Resistor").decl().id();
    assert_eq!(types.callee(placement), Some(Callee::Module(resistor)));
    assert_eq!(types.ty(placement), Some(&Ty::Module(resistor)));
    assert!(types.reached().contains(&resistor));

    // A param of the wrong type: at the value.
    let (src, d) = diagnostics("module Top(a: E, b: E) { let r1 = Resistor { r: true }(a, b); }");
    assert_eq!(codes_of(&d), ["P0002"]);
    assert_eq!(snippet(&src, &d[0]), "true");

    // Too few and too many connections are `E0312`, at the placement.
    for args in ["a", "a, b, b"] {
        let (src, d) = diagnostics(&format!("module Top(a: E, b: E) {{ let r1 = Resistor {{ r: 1k }}({args}); }}"));
        assert_eq!(codes_of(&d), ["E0312"], "{args}");
        assert!(snippet(&src, &d[0]).starts_with("Resistor"), "{}", snippet(&src, &d[0]));
    }

    // A param the module does not have, and one written twice, are `E0320`: the POM reports
    // them, and a design with diagnostics is given back as it is.
    for block in ["{ q: 1.0 }", "{ r: 1.0, r: 2.0 }"] {
        let src = format!("{NATURES}module Top(a: E, b: E) {{ let r1 = Resistor {block}(a, b); }}");
        let design = support::build(&src);
        assert_eq!(support::codes(&design), ["E0320"], "{block}");
        let err = Types::check(&design, "Top").expect_err("a design with diagnostics");
        assert_eq!(codes_of(&err), ["E0320"], "{block}");
    }
}

#[test]
fn a_node_of_another_nature_is_e0251() {
    let (src, d) = diagnostics("module Top(a: E, t: T) { let r1 = Resistor { r: 1.0 }(a, t); }");
    assert_eq!(codes_of(&d), ["E0251"]);
    assert_eq!(snippet(&src, &d[0]), "t");

    // A value where a node is wanted is a plain mismatch.
    let (src, d) = diagnostics("module Top(a: E) { let r1 = Resistor { r: 1.0 }(a, 1.0); }");
    assert_eq!(codes_of(&d), ["P0002"]);
    assert_eq!(snippet(&src, &d[0]), "1.0");
}
