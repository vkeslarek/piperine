//! Value expressions of `piperine_lang::types` (TYP-02, TYP-06 AC 3, and the edge cases of
//! `let` and closures): literals, operators, `if`, blocks, `let`, calls, closures and the
//! method catalogue, one positive and one negative source per rule. Every source is a module
//! `Top` whose `analog` block holds the expressions under test.

#[path = "support/pom.rs"]
mod support;

use piperine_lang::pom::{DeclId, DeclKind, Design, DesignDiagnostic, ExprId, ExprKind, Reflect};
use piperine_lang::types::{Callee, Method, Ty, Types};

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

    // `Top` is checked first, then `lower` (declared above it), so the worklist meets the
    // later span first; the sort puts them back in source order.
    let (src, d) = diagnostics("fn lower() -> f64 { true }", "let a: bool = 1.0; let y = lower();");
    assert_eq!(d.len(), 2, "{d:?}");
    assert_eq!([snippet(&src, &d[0]), snippet(&src, &d[1])], ["true", "1.0"]);

    // Across files: the worklist goes top.msl, helper.msl, top.msl again.
    let design = Design::builder()
        .std_source("lib.msl", "pub mod prelude;\n")
        .std_source("prelude.msl", "")
        .source("top.msl", "mod helper;\npub fn g() -> f64 { true }\nmodule Top() { }\nanalog for Top { let a: bool = 1.0; let y = helper::f(); }")
        .source("helper.msl", "pub fn f() -> f64 { let z: bool = 1.0; super::g() }")
        .build();
    assert!(design.diagnostics().is_empty(), "{:?}", support::codes(&design));
    let d = Types::check(&design, "Top").expect_err("expected diagnostics");
    assert_eq!(d.len(), 3, "{d:?}");
    let files: Vec<_> = d.iter().map(|d| d.file).collect();
    assert!(files.windows(2).all(|w| w[0] <= w[1]), "{files:?}");
    let starts: Vec<usize> = d.iter().filter(|d| d.file == files[0]).map(|d| d.diagnostic.span().start).collect();
    assert!(starts.windows(2).all(|w| w[0] < w[1]), "{starts:?}");
}

#[test]
fn a_let_without_type_or_value_is_p0001() {
    let (src, d) = diagnostics("", "let x;");
    assert_eq!(codes_of(&d), ["P0001"]);
    assert_eq!(snippet(&src, &d[0]), "let x;");
}

const FUNCTIONS: &str = "fn half(a: f64) -> f64 { a / 2.0 }\nfn positive(a: f64) -> bool { a > 0.0 }\nfn apply(f: fn(f64, f64) -> f64, x: f64) -> f64 { x }\nfn fact(n: f64) -> f64 { if n <= 1.0 { 1.0 } else { n * fact(n - 1.0) } }";

#[test]
fn a_call_takes_its_declared_result() {
    let t = local_types(FUNCTIONS, "let a = half(1.0); let b = positive(a);", &["a", "b"]);
    assert_eq!(t, [Ty::F64, Ty::Bool]);

    // The callee is recorded and the function is reached.
    let src = source(FUNCTIONS, "let a = half(1.0);");
    let design = design_of(&src);
    let types = Types::check(&design, "Top").expect("clean");
    let half = design.function("half").expect("half").decl().id();
    let call = (0..design.expr_count()).map(|i| ExprId(i as u32)).find(|e| matches!(design.expr_node(*e).kind, ExprKind::Call { .. }) && src[design.expr_node(*e).span.range()].starts_with("half(")).expect("a call");
    assert_eq!(types.callee(call), Some(Callee::Function(half)));
    assert!(types.reached().contains(&half));
    // The `apply` body is not reached: nothing calls it.
    assert!(!types.reached().contains(&design.function("apply").unwrap().decl().id()));

    // Each argument is checked against its parameter, and the count is checked.
    let (src, d) = diagnostics(FUNCTIONS, "let a = half(true);");
    assert_eq!(codes_of(&d), ["P0002"]);
    assert_eq!(snippet(&src, &d[0]), "true");
    let (_, d) = diagnostics(FUNCTIONS, "let a = half(1.0, 2.0);");
    assert_eq!(codes_of(&d), ["P0002"]);
    let (_, d) = diagnostics(FUNCTIONS, "let a = half();");
    assert_eq!(codes_of(&d), ["P0002"]);

    // A function takes positional arguments only: the POM refuses a named one (`E0140`).
    let named = support::build(&source(FUNCTIONS, "let a = half(a: 1.0);"));
    assert_eq!(support::codes(&named), ["E0140"]);
}

#[test]
fn a_closure_takes_its_types_from_the_parameter() {
    let t = local_types(FUNCTIONS, "let r = apply(|a, b| a + b, 2.0); let s = apply(|a: f64, b: f64| a * b, 2.0);", &["r", "s"]);
    assert_eq!(t, [Ty::F64, Ty::F64]);

    // The body is checked against the declared result.
    let (src, d) = diagnostics(FUNCTIONS, "let r = apply(|a, b| a > b, 2.0);");
    assert_eq!(codes_of(&d), ["P0002"]);
    assert_eq!(snippet(&src, &d[0]), "a > b");
    // A parameter type written in the closure must be the declared one.
    let (src, d) = diagnostics(FUNCTIONS, "let r = apply(|a: bool, b| b, 2.0);");
    assert_eq!(codes_of(&d), ["P0002"]);
    assert_eq!(snippet(&src, &d[0]), "bool");
    // And so must the parameter count.
    let (_, d) = diagnostics(FUNCTIONS, "let r = apply(|a| a, 2.0);");
    assert_eq!(codes_of(&d), ["P0002"]);
}

#[test]
fn a_recursive_function_is_typed_from_its_signature() {
    let t = local_types(FUNCTIONS, "let r = fact(5.0);", &["r"]);
    assert_eq!(t, [Ty::F64]);
    let src = source(FUNCTIONS, "let r = fact(5.0);");
    let design = design_of(&src);
    let types = Types::check(&design, "Top").expect("clean");
    let fact = design.function("fact").unwrap().decl().id();
    assert_eq!(types.reached().iter().filter(|d| **d == fact).count(), 1);
}

#[test]
fn a_closure_outside_an_argument_is_p0001() {
    let (src, d) = diagnostics(FUNCTIONS, "let f = |x: f64| x;");
    assert_eq!(codes_of(&d), ["P0001"]);
    assert_eq!(snippet(&src, &d[0]), "|x: f64| x");
    // An argument whose parameter is not a function type does not take a closure either.
    let (src, d) = diagnostics(FUNCTIONS, "let a = half(|x| x);");
    assert_eq!(codes_of(&d), ["P0001"]);
    assert_eq!(snippet(&src, &d[0]), "|x| x");
}

#[test]
fn a_deferred_method_is_resolved() {
    // The M1 diode's own form: `(vd / nvt).exp()`, in a function the module calls.
    let items = "fn shockley(vd: f64, nvt: f64) -> f64 { (vd / nvt).exp() - 1.0 }";
    let src = source(items, "let i = shockley(0.5, 0.026);");
    let design = design_of(&src);
    let types = Types::check(&design, "Top").expect("clean");
    let call = (0..design.expr_count()).map(|i| ExprId(i as u32)).find(|e| matches!(design.expr_node(*e).kind, ExprKind::MethodCall { .. })).expect("a method call");
    assert_eq!(types.method(call), Some(Method::Exp));
    assert_eq!(types.ty(call), Some(&Ty::F64));

    // A method outside the table is refused by name.
    let (src, d) = diagnostics("fn p(a: f64) -> f64 { a.powf(2.0) }", "let x = p(1.0);");
    assert_eq!(codes_of(&d), ["P0001"]);
    assert!(d[0].diagnostic.message.contains("powf"), "{}", d[0].diagnostic.message);
    assert_eq!(snippet(&src, &d[0]), "powf");
}

#[test]
fn catalogue_methods_on_f64_are_f64() {
    let items = "fn all(a: f64) -> f64 { a.exp() + a.ln() + a.sqrt() + a.abs() + 2.0_f64.sqrt() }";
    let t = local_types(items, "let r = all(1.0);", &["r"]);
    assert_eq!(t, [Ty::F64]);

    // A method takes no argument; a bool receiver has no catalogue row.
    let (_, d) = diagnostics("fn p(a: f64) -> f64 { a.exp(1.0) }", "let x = p(1.0);");
    assert_eq!(codes_of(&d), ["P0002"]);
    let (_, d) = diagnostics("fn p(a: bool) -> f64 { a.exp() }", "let x = p(true);");
    assert_eq!(codes_of(&d), ["P0001"]);
}

#[test]
fn the_std_pnjlim_checks_clean() {
    let design = Design::builder().source("top.msl", "use std::analog::pnjlim;\nmodule Top() { }\nanalog for Top { let r = pnjlim(0.7, 0.6, 0.026, 0.6); }").build();
    assert!(design.diagnostics().is_empty(), "{:?}", support::codes(&design));
    let types = Types::check(&design, "Top").unwrap_or_else(|d| panic!("{:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>()));
    let pnjlim = design.function("pnjlim").expect("std pnjlim").decl().id();
    assert!(types.reached().contains(&pnjlim));
}

#[test]
fn a_function_is_a_value_only_as_an_argument() {
    let items = "fn h(a: f64, b: f64) -> f64 { a }\nfn apply(f: fn(f64, f64) -> f64, x: f64) -> f64 { x }";
    // As an argument whose parameter has a function type, it checks.
    local_types(items, "let r = apply(h, 2.0);", &["r"]);

    // Anywhere else it is outside the subset.
    let (src, d) = diagnostics(items, "let f = h;");
    assert_eq!(codes_of(&d), ["P0001"]);
    assert_eq!(snippet(&src, &d[0]), "h");

    // Not through an `if`: the function chosen there is no named function at the argument.
    let (src, d) = diagnostics(items, "let r = apply(if true { h } else { h }, 2.0);");
    assert_eq!(codes_of(&d), ["P0001", "P0001"]);
    assert_eq!(snippet(&src, &d[0]), "h");
    let (src, d) = diagnostics("fn lim(n: f64, o: f64) -> f64 { n }", "let x = limit(1.0, if true { lim } else { lim });");
    assert_eq!(codes_of(&d), ["P0001", "P0001"]);
    assert_eq!(snippet(&src, &d[0]), "lim");

    // A call whose callee is not a named function or built-in has no callee to record: refused.
    let (src, d) = diagnostics("fn run(f: fn(f64) -> f64, x: f64) -> f64 { f(x) }", "let y = run(|a| a, 1.0);");
    assert_eq!(codes_of(&d), ["P0001"]);
    assert_eq!(snippet(&src, &d[0]), "f(x)");
}

#[test]
fn an_unsupported_form_does_not_hide_an_independent_error() {
    // A refused annotation still has its initializer checked.
    let (src, d) = diagnostics("", "let x: u32 = 1.0 + true;");
    assert_eq!(codes_of(&d), ["P0001", "P0002"]);
    assert_eq!(snippet(&src, &d[1]), "true");

    // A generic function is refused and its body is checked as well.
    let (src, d) = diagnostics("fn g<T>(x: f64) -> f64 { x + true }", "let x = g(1.0);");
    assert_eq!(codes_of(&d), ["P0001", "P0002"]);
    assert!(d[0].diagnostic.message.contains("generic function"));
    assert_eq!(snippet(&src, &d[1]), "true");

    // A body that ends in a statement lacks its result whatever else it got wrong.
    let (src, d) = diagnostics("fn f(a: f64) -> f64 { let y = a + true; }", "let z = f(1.0);");
    assert_eq!(codes_of(&d), ["P0002", "P0002"]);
    assert_eq!(snippet(&src, &d[0]), "{ let y = a + true; }");
    assert_eq!(snippet(&src, &d[1]), "true");

    // A last statement that is not itself refused leaves the missing result its own error,
    // whatever it holds that is refused.
    let (src, d) = diagnostics("fn f(a: f64) -> f64 { let y = 1; }", "let z = f(1.0);");
    assert_eq!(codes_of(&d), ["P0002", "P0001"]);
    assert_eq!(snippet(&src, &d[0]), "{ let y = 1; }");
    let (src, d) = diagnostics("fn f(a: f64) -> f64 { a % 2.0; }", "let z = f(1.0);");
    assert_eq!(codes_of(&d), ["P0002", "P0001"]);
    assert_eq!(snippet(&src, &d[0]), "{ a % 2.0; }");
    assert_eq!(snippet(&src, &d[1]), "a % 2.0");

    // A refused `return` stands where the value would be: no missing result besides it.
    let (_, d) = diagnostics("fn f(a: f64) -> f64 { return a; }", "let z = f(1.0);");
    assert_eq!(codes_of(&d), ["P0001"]);
}
