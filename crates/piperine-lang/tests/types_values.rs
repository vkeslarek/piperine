//! Value expressions of `piperine_lang::types` (TYP-02, TYP-06 AC 3, and the edge cases of
//! `let` and closures): literals, operators, `if`, blocks, `let`, calls, closures and the
//! method catalogue, one positive and one negative source per rule. Every source is a module
//! `Top` whose `analog` block holds the expressions under test.

#[path = "support/pom.rs"]
mod support;

use piperine_lang::pom::{DeclId, DeclKind, Design, DesignDiagnostic, Reflect};
use piperine_lang::types::{Ty, Types};

/// A design of `Top` with `body` as its analog block and `items` before it.
fn source(items: &str, body: &str) -> String {
    format!("{items}\nmodule Top() {{ }}\nanalog for Top {{ {body} }}")
}

fn design_of(src: &str) -> Design {
    let design = support::build(src);
    assert!(design.diagnostics().is_empty(), "the source must parse and resolve: {:?}\n{src}", support::codes(&design));
    design
}

fn binding(design: &Design, name: &str) -> DeclId {
    design.decls().into_iter().find(|d| d.name() == Some(name) && matches!(d.kind(), DeclKind::Binding(_))).unwrap_or_else(|| panic!("no binding `{name}`")).id()
}

/// The type of each named `let` binding of the analog block.
fn local_types(items: &str, body: &str, names: &[&str]) -> Vec<Ty> {
    let src = source(items, body);
    let design = design_of(&src);
    let types = Types::check(&design, "Top").unwrap_or_else(|d| panic!("expected a clean check: {:?}\n{src}", d.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>()));
    names.iter().map(|n| types.local(binding(&design, n)).unwrap_or_else(|| panic!("no type for `{n}`")).clone()).collect()
}

fn diagnostics(items: &str, body: &str) -> (String, Vec<DesignDiagnostic>) {
    let src = source(items, body);
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

#[test]
fn literals_are_f64_or_bool() {
    let t = local_types("", "let a = 1.0; let b = 1.0_f64; let c = 2k; let d = 1e-14; let e = true; let f = false;", &["a", "b", "c", "d", "e", "f"]);
    assert_eq!(t, [Ty::F64, Ty::F64, Ty::F64, Ty::F64, Ty::Bool, Ty::Bool]);

    // An unsuffixed integer literal where `f64` is required is a type error, not a coercion.
    let (src, d) = diagnostics("", "let x: f64 = 100;");
    assert_eq!(codes_of(&d), ["P0002"]);
    assert_eq!(snippet(&src, &d[0]), "100");
}

#[test]
fn operators_type_as_rust_types_them() {
    let t = local_types(
        "",
        "let a = 1.0 + 2.0; let b = 1.0 - 2.0; let c = 1.0 * 2.0; let d = 1.0 / 2.0; let e = -1.0; \
         let f = 1.0 < 2.0; let g = 1.0 <= 2.0; let h = 1.0 > 2.0; let i = 1.0 >= 2.0; let j = 1.0 == 2.0; let k = 1.0 != 2.0; \
         let l = true && false; let m = true || false; let n = !true;",
        &["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n"],
    );
    assert!(t[..5].iter().all(|t| *t == Ty::F64));
    assert!(t[5..].iter().all(|t| *t == Ty::Bool));

    // Each operator refuses the other type.
    for body in ["let x = 1.0 + true;", "let x = true && 1.0;", "let x = !1.0;", "let x = -true;", "let x = 1.0 < true;"] {
        let (_, d) = diagnostics("", body);
        assert_eq!(codes_of(&d), ["P0002"], "{body}");
    }
}

#[test]
fn if_and_block_take_their_value_type() {
    let t = local_types("", "let a = if true { 1.0 } else { 2.0 }; let b = { 1.0 }; let c = { let z = 1.0; }; let d = if 1.0 > 0.0 { true } else if false { false } else { true };", &["a", "b", "c", "d"]);
    assert_eq!(t, [Ty::F64, Ty::F64, Ty::Unit, Ty::Bool]);

    // The arms of an `if` have one type.
    let (src, d) = diagnostics("", "let x = if true { 1.0 } else { false };");
    assert_eq!(codes_of(&d), ["P0002"]);
    assert_eq!(snippet(&src, &d[0]), "false");
}

#[test]
fn a_let_takes_its_initializer_type() {
    let t = local_types("", "let a = true; let b = a; let c: f64 = 1.0; let d = c * 2.0;", &["a", "b", "c", "d"]);
    assert_eq!(t, [Ty::Bool, Ty::Bool, Ty::F64, Ty::F64]);
}

#[test]
fn a_mismatch_names_both_types() {
    let (src, d) = diagnostics("", "let a = 1.0; let x: f64 = if a > 0.0 { a } else { true };");
    assert_eq!(codes_of(&d), ["P0002"]);
    assert_eq!(snippet(&src, &d[0]), "true");
    let message = &d[0].diagnostic.message;
    assert!(message.contains("f64") && message.contains("bool"), "{message}");

    // A `let` and its annotation.
    let (src, d) = diagnostics("", "let x: bool = 1.0;");
    assert_eq!(snippet(&src, &d[0]), "1.0");
    let message = &d[0].diagnostic.message;
    assert!(message.contains("f64") && message.contains("bool"), "{message}");
}

#[test]
fn diagnostics_come_in_source_order_one_per_expression() {
    let (src, d) = diagnostics("", "let a: bool = 1.0; let b: f64 = true; let c: f64 = 1.0 + true;");
    assert_eq!(d.len(), 3, "{d:?}");
    let starts: Vec<usize> = d.iter().map(|d| d.diagnostic.span().start).collect();
    assert!(starts.windows(2).all(|w| w[0] < w[1]), "{starts:?}");
    assert_eq!([snippet(&src, &d[0]), snippet(&src, &d[1]), snippet(&src, &d[2])], ["1.0", "true", "true"]);

    // One mistake inside nested parentheses is one diagnostic, not one per level.
    let (_, d) = diagnostics("", "let x: f64 = ((true));");
    assert_eq!(d.len(), 1, "{d:?}");
}

#[test]
fn a_let_without_type_or_value_is_p0001() {
    let (src, d) = diagnostics("", "let x;");
    assert_eq!(codes_of(&d), ["P0001"]);
    assert_eq!(snippet(&src, &d[0]), "let x;");
}
