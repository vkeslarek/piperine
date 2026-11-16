//! The subset boundary of `piperine_lang::types` (TYP-05): every language construct of the
//! feature's Out of Scope table is refused with one `P0001` that names it, and an item holding
//! such a construct that the top does not reach raises nothing. Each row is one source whose
//! `Top` reaches the construct; the fragment is what the diagnostic must name.

#[path = "support/pom.rs"]
mod support;

use piperine_lang::pom::{Design, DesignDiagnostic};
use piperine_lang::types::Types;

const HEAD: &str = "nature E { potential: V { access: v, units: \"V\", abstol: 1u }, flow: I { access: i, units: \"A\", abstol: 1p } }\n\
module Resistor(p: E, n: E) { r: f64; }\n";

/// One row: a construct outside the subset, the source that reaches it from `Top` (after
/// `HEAD`), and the words its diagnostic must contain.
struct Row {
    construct: &'static str,
    source: &'static str,
    names: &'static str,
}

const ROWS: &[Row] = &[
    Row { construct: "index", source: "module Top() { }\nanalog for Top { let x = 1.0; let y = x[0]; }", names: "index expression" },
    Row { construct: "closure that is not a limit function", source: "module Top() { }\nanalog for Top { let f = |x: f64| x; }", names: "closure" },
    Row { construct: "struct as a parameter type", source: "struct S { a: f64 }\nfn f(s: S) -> f64 { 1.0 }\nmodule Top() { }\nanalog for Top { let x = f(1.0); }", names: "`struct` item used as a type" },
    Row { construct: "analysis of a kind other than Static", source: "module Top() { }\nanalog for Top { let x = analysis(Ac); }", names: "`analysis`" },
    Row { construct: "settle", source: "module Top() { }\nanalog for Top { let x = settle; }", names: "`settle`" },
    Row { construct: "f32", source: "module Top() { }\nanalog for Top { let x: f32 = 1.0; }", names: "`f32`" },
    Row { construct: "Float<N>", source: "module Top() { }\nanalog for Top { let x: Float<32> = 1.0; }", names: "`Float`" },
    Row { construct: "str", source: "fn f(s: &str) -> f64 { 1.0 }\nmodule Top() { }\nanalog for Top { let x = f(\"a\"); }", names: "reference type" },
    Row { construct: "string literal", source: "module Top() { }\nanalog for Top { let x = \"a\"; }", names: "string literal" },
    Row { construct: "tuple type", source: "module Top() { }\nanalog for Top { let x: (f64, f64) = 1.0; }", names: "tuple type" },
    Row { construct: "tuple", source: "module Top() { }\nanalog for Top { let x = (1.0, 2.0); }", names: "tuple expression" },
    Row { construct: "array", source: "module Top() { }\nanalog for Top { let x = [1.0, 2.0]; }", names: "array expression" },
    Row { construct: "array type", source: "module Top() { }\nanalog for Top { let x: [f64; 2] = 1.0; }", names: "array type" },
    Row { construct: "slice", source: "fn f(s: [f64]) -> f64 { 1.0 }\nmodule Top() { }\nanalog for Top { let x = f(1.0); }", names: "slice type" },
    Row { construct: "reference", source: "module Top() { }\nanalog for Top { let x = 1.0; let y = &x; }", names: "reference `&`" },
    Row { construct: "reference type", source: "module Top() { }\nanalog for Top { let x: &f64 = 1.0; }", names: "reference type" },
    Row { construct: "struct", source: "struct S { a: f64 }\nmodule Top() { }\nanalog for Top { let s = S { a: 1.0 }; }", names: "struct expression" },
    Row { construct: "generation if", source: "module Top(a: E) { if true { let r = Resistor { r: 1.0 }(a, gnd); } }", names: "generation" },
    Row { construct: "match", source: "module Top() { }\nanalog for Top { let x = 1.0; let y = match x { _ => 1.0 }; }", names: "`match` expression" },
    Row { construct: "struct with an impl (its methods are never reached)", source: "struct S { a: f64 }\nimpl S { fn get(self) -> f64 { 1.0 } }\nmodule Top() { }\nanalog for Top { let s = S { a: 1.0 }; let x = s.get(); }", names: "struct expression" },
    Row { construct: "generic function", source: "fn g<T>(x: f64) -> f64 { x }\nmodule Top() { }\nanalog for Top { let x = g(1.0); }", names: "generic function" },
    Row { construct: "generic module", source: "module G<N: Nature>(a: N) { }\nmodule Top(a: E) { let g = G(a); }", names: "generic module" },
    Row { construct: "const generic", source: "module G<const N: usize>() { }\nmodule Top() { let g = G(); }", names: "generic module" },
    Row { construct: "Option", source: "module Top() { a: Option<f64>; }", names: "`Option`" },
    Row { construct: "Some", source: "module Top() { }\nanalog for Top { let x = Some(1.0); }", names: "built-in `Some`" },
    Row { construct: "Unknown", source: "module Top() { }\nanalog for Top { let x = Unknown; }", names: "`Unknown`" },
    Row { construct: "discrete nature", source: "nature B { value: bool }\nmodule Top(a: B) { }", names: "discrete nature `B`" },
    Row { construct: "bundle", source: "struct Bn { a: E }\nmodule Top(p: Bn) { }", names: "`struct` item used as a type" },
    Row { construct: "digital block", source: "module Top() { }\ndigital for Top { }", names: "`digital` block" },
    Row { construct: "reaction and event", source: "module Top() { }\ndigital for Top { on settle { } }", names: "`digital` block" },
    Row { construct: "definition", source: "module Top() { signal s: E; s = 1.0; }", names: "definition" },
    Row { construct: "emit!", source: "module Top() { }\nanalog for Top { emit!(\"o\", x = 1.0); }", names: "`emit!`" },
    Row { construct: "while", source: "module Top() { }\nanalog for Top { while true { } }", names: "`while` loop" },
    Row { construct: "loop", source: "module Top() { }\nanalog for Top { loop { } }", names: "`loop`" },
    Row { construct: "for", source: "module Top() { }\nanalog for Top { for i in 0..3 { } }", names: "`for` loop" },
    Row { construct: "break", source: "module Top() { }\nanalog for Top { break; }", names: "`break`" },
    Row { construct: "continue", source: "module Top() { }\nanalog for Top { continue; }", names: "`continue`" },
    Row { construct: "return", source: "fn f() -> f64 { return 1.0; }\nmodule Top() { }\nanalog for Top { let x = f(); }", names: "`return`" },
    Row { construct: "let mut", source: "module Top() { }\nanalog for Top { let mut x = 1.0; }", names: "`let mut` binding" },
    Row { construct: "assignment", source: "module Top() { }\nanalog for Top { let x = 1.0; x = 2.0; }", names: "assignment" },
    Row { construct: "let else", source: "module Top() { }\nanalog for Top { let x = 1.0 else { return; }; }", names: "`let ... else`" },
    Row { construct: "try", source: "module Top() { }\nanalog for Top { let x = 1.0?; }", names: "`?` expression" },
    Row { construct: "template", source: "module Top(a: E, b: E) { let t = Resistor { r: 1.0 }; }", names: "module template" },
    Row { construct: "placement through a template", source: "module Top(a: E, b: E) { let t = Resistor { r: 1.0 }; let r = t(a, b); }", names: "module template" },
    Row { construct: "value input", source: "module Top(in x: f64) { }", names: "value input port" },
    Row { construct: "module result", source: "module M() -> f64 { 1.0 }\nmodule Top() { let m = M(); }", names: "module result" },
    Row { construct: "extern module", source: "#[link(\"x.osdi\", model = \"x\")]\nextern \"osdi\" module X(a: E) { }\nmodule Top(a: E) { let x = X(a); }", names: "`extern` module" },
    Row { construct: "generation", source: "module Top(a: E) { for i in 0..2 { let r = Resistor { r: 1.0 }(a, gnd); } }", names: "generation" },
    Row { construct: "bound_step", source: "module Top() { }\nanalog for Top { bound_step(1.0); }", names: "`bound_step`" },
    Row { construct: "ddx", source: "module Top(a: E) { }\nanalog for Top { let x = ddx(v(a), v(a)); }", names: "`ddx`" },
    Row { construct: "delay", source: "module Top(a: E) { }\nanalog for Top { let x = delay(v(a), 1.0); }", names: "`delay`" },
    Row { construct: "noise", source: "module Top() { }\nanalog for Top { let x = noise(1.0); }", names: "`noise`" },
    Row { construct: "cross", source: "module Top(a: E) { }\nanalog for Top { let x = cross(v(a)); }", names: "`cross`" },
    Row { construct: "at", source: "module Top() { }\nanalog for Top { let x = at(1.0); }", names: "`at`" },
    Row { construct: "simparam", source: "module Top() { }\nanalog for Top { let x = simparam(1.0); }", names: "`simparam`" },
    Row { construct: "now", source: "module Top() { }\nanalog for Top { let x = now(); }", names: "`now`" },
    Row { construct: "freq", source: "module Top() { }\nanalog for Top { let x = freq(); }", names: "`freq`" },
    Row { construct: "inflow", source: "module Top(a: E) { }\nanalog for Top { let x = a.inflow(); }", names: "inflow" },
    Row { construct: "flow probe read as a value", source: "module Top(a: E) { }\nanalog for Top { let x = i(a); }", names: "flow probe" },
    Row { construct: "trait", source: "trait Tr { fn get(self) -> f64; }\nimpl Tr for f64 { fn get(self) -> f64 { self } }\nmodule Top() { }\nanalog for Top { let y = 1.0.get(); }", names: "method `get`" },
    Row { construct: "commit", source: "module Top() { signal s: E; }\ndigital for Top { on settle { s <- 1.0; } }", names: "`digital` block" },
];

fn check(row: &Row) -> (String, Vec<DesignDiagnostic>) {
    let src = format!("{HEAD}{}", row.source);
    let design = support::build(&src);
    assert!(design.diagnostics().is_empty(), "{}: the source must parse and resolve: {:?}\n{src}", row.construct, support::codes(&design));
    match Types::check(&design, "Top") {
        Ok(_) => panic!("{}: expected a P0001, the check passed\n{src}", row.construct),
        Err(d) => (src, d),
    }
}

#[test]
fn a_construct_outside_the_subset_is_p0001() {
    for row in ROWS {
        let (src, d) = check(row);
        assert_eq!(d.len(), 1, "{}: one diagnostic per construct, got {:?}\n{src}", row.construct, d.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
        assert_eq!(d[0].diagnostic.code, "P0001", "{}: {}", row.construct, d[0].diagnostic.message);
        let span = d[0].diagnostic.span();
        assert!(span.end > span.start && span.end <= src.len(), "{}: a span of the construct", row.construct);
    }
}

#[test]
fn p0001_names_the_construct() {
    for row in ROWS {
        let (_, d) = check(row);
        let message = &d[0].diagnostic.message;
        assert!(message.contains(row.names), "{}: `{message}` does not name `{}`", row.construct, row.names);
    }
}

#[test]
fn unreached_items_are_not_reported() {
    // Everything below is outside the subset and nothing reaches it from `Top`.
    let src = format!(
        "{HEAD}\
         struct S {{ a: f64 }}\nenum C {{ A, B }}\ntrait Tr {{ fn get(self) -> f64; }}\nimpl S {{ fn get(self) -> f64 {{ 1.0 }} }}\n\
         const N: u32 = 3;\n\
         fn loops(x: f64) -> f64 {{ let mut y = x; while y > 0.0 {{ y = y - 1.0; }} match y {{ _ => y }} }}\n\
         fn generic<T>(x: T) -> T {{ x }}\n\
         module Generic<N: Nature>(a: N) {{ }}\n\
         module Digital(in d: bool) -> f64 {{ 1.0 }}\n\
         digital for Digital {{ }}\n\
         module Top(a: E) {{ let r = Resistor {{ r: 1.0 }}(a, gnd); }}"
    );
    let design = support::build(&src);
    assert!(design.diagnostics().is_empty(), "{:?}", support::codes(&design));
    let types = Types::check(&design, "Top").unwrap_or_else(|d| panic!("{:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>()));
    assert_eq!(types.reached().len(), 2, "only Top and Resistor are reached");

    // The same names, reached, are refused.
    let reached = format!("{HEAD}fn loops(x: f64) -> f64 {{ let mut y = x; y }}\nmodule Top() {{ }}\nanalog for Top {{ let x = loops(1.0); }}");
    let design: Design = support::build(&reached);
    assert!(Types::check(&design, "Top").is_err());
}

/// Constructs that only resolve against the real standard library: `Bit`, `Repr` and the known
/// flag are refused by name as well.
const STD_ROWS: &[Row] = &[
    Row { construct: "Bit", source: "module Top(c: Bit) { }", names: "`Bit`" },
    Row { construct: "Repr", source: "module Top() { }\nanalog for Top { let x: Repr = 1.0; }", names: "`Repr`" },
    Row { construct: "known flag", source: "module Top() { }\nanalog for Top { let x = 1.0.level(1.0); }", names: "method `level`" },
];

#[test]
fn std_types_outside_the_subset_are_p0001() {
    for row in STD_ROWS {
        let src = format!("{HEAD}{}", row.source);
        let design = Design::builder().source("top.msl", &src).build();
        assert!(design.diagnostics().is_empty(), "{}: the source must parse and resolve: {:?}\n{src}", row.construct, support::codes(&design));
        let d = Types::check(&design, "Top").expect_err(row.construct);
        assert_eq!(d.len(), 1, "{}: {:?}", row.construct, d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(d[0].diagnostic.code, "P0001", "{}: {}", row.construct, d[0].diagnostic.message);
        assert!(d[0].diagnostic.message.contains(row.names), "{}: {}", row.construct, d[0].diagnostic.message);
    }
}
