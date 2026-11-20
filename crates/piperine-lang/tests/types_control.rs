//! Mutation, loops, exits, patterns and function values in `piperine_lang::types` (feature
//! `composite-values`, CV-01 to CV-07; docs/spec Part I §1.5 to §1.8, §3). Each test states the
//! rule it proves; every source is a design of `Top` over an empty standard library.

#[path = "support/pom.rs"]
mod support;

use piperine_lang::pom::{Design, DesignDiagnostic, ExprId, ExprKind};
use piperine_lang::types::{Ty, Types};

/// A design of `Top` with `items` before it and `body` as its analog block.
fn source(items: &str, body: &str) -> String {
    format!("{items}\nmodule Top() {{ }}\nanalog for Top {{ {body} }}")
}

fn design_of(src: &str) -> Design {
    let design = support::build(src);
    assert!(design.diagnostics().is_empty(), "the source must parse and resolve: {:?}\n{src}", support::codes(&design));
    design
}

/// The diagnostics of checking `Top`, as `(code, message)`; empty when the check passes.
fn outcome(src: &str) -> Vec<(String, String)> {
    let design = design_of(src);
    match Types::check(&design, "Top") {
        Ok(_) => Vec::new(),
        Err(found) => found.iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect(),
    }
}

/// The source checks clean.
fn assert_clean(src: &str) {
    let found = outcome(src);
    assert!(found.is_empty(), "expected a clean check, got {found:?}\n{src}");
}

/// The source is refused with exactly one diagnostic of `code`, whose message holds `words`.
fn assert_refused(src: &str, code: &str, words: &str) {
    let found = outcome(src);
    assert_eq!(found.len(), 1, "expected one diagnostic, got {found:?}\n{src}");
    assert_eq!(found[0].0, code, "{found:?}\n{src}");
    assert!(found[0].1.contains(words), "`{}` does not name `{words}`\n{src}", found[0].1);
}

/// Every expression of the design of kind `Assign`, with the type the checker gave it.
fn assignment_types(src: &str) -> Vec<Option<Ty>> {
    let design = design_of(src);
    let types = Types::check(&design, "Top").unwrap_or_else(|d| panic!("{:?}\n{src}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>()));
    (0..design.expr_count())
        .map(|e| ExprId(e as u32))
        .filter(|e| matches!(design.expr_node(*e).kind, ExprKind::Assign { .. }))
        .map(|e| types.ty(e).cloned())
        .collect()
}

#[allow(dead_code)]
fn diagnostics(src: &str) -> Vec<DesignDiagnostic> {
    Types::check(&design_of(src), "Top").err().unwrap_or_default()
}

// ---- T11: places, `let mut` and assignment -------------------------------------------------

#[test]
fn let_mut_and_every_compound_operator_type_check() {
    // §1.5.12: `place = value` is `()`; `place op= value` is `place = place op value` for every
    // operator of level 15, in an `analog` block and in a function.
    let body = "let mut a = 1.0; a = 2.0; a += 1.0; a -= 1.0; a *= 2.0; a /= 2.0;\n\
                let mut n = 1u8; n = n + 1; n %= 3; n &= 1; n |= 2; n ^= 3; n <<= 1; n >>= 1;\n\
                let mut b = true; b &= false; b |= true; b ^= true;\n\
                let s = twice(a);";
    let items = "fn twice(x: f64) -> f64 { let mut y = x; y += x; y }";
    let src = source(items, body);
    assert_clean(&src);
    let types = assignment_types(&src);
    assert_eq!(types.len(), 16, "every assignment is typed: {types:?}");
    assert!(types.iter().all(|t| *t == Some(Ty::Unit)), "an assignment has the type `()` (§1.5.12): {types:?}");
    // a deferred local takes the type of its first assignment and is assigned on both paths
    assert_clean(&source("", "let x; let c = true; if c { x = 1.0; } else { x = 2.0; } let y = x + 1.0;"));
    assert_clean(&source("", "let x: f64; x = 1.0; let y = x;"));
    // a mismatching value is an ordinary mismatch
    assert_refused(&source("", "let mut a = 1.0; a = true;"), "P0002", "expected `f64`");
}

#[test]
fn assigning_a_non_place_is_e0183_naming_the_target() {
    let items = "nature E { potential: V { access: v, units: \"V\", abstol: 1u }, flow: I { access: i, units: \"A\", abstol: 1p } }\nconst K: f64 = 1.0;";
    let top = |params: &str, body: &str| format!("{items}\nmodule Top({params}) {{ r: f64; let w = 1.0; }}\nanalog for Top {{ {body} }}");
    // §1.5.12: a local without `mut`, a param, a const, a function parameter, a body `let` and
    // a value that is no place at all.
    assert_refused(&source(items, "let x = 1.0; x = 2.0;"), "E0183", "cannot assign to `x`");
    assert_refused(&top("", "r = 2.0;"), "E0183", "cannot assign to `r`: it is a param");
    assert_refused(&source(items, "K = 2.0;"), "E0183", "cannot assign to `K`: it is a `const`");
    assert_refused(&source("fn f(p: f64) -> f64 { p = 1.0; p }", "let y = f(1.0);"), "E0183", "cannot assign to `p`: it is a function parameter");
    assert_refused(&top("", "w = 2.0;"), "E0183", "cannot assign to `w`: it is a body `let`");
    assert_refused(&source(items, "1.0 = 2.0;"), "E0183", "not a place");
    // a carrier, written in a reaction body (`Bit` is in the standard library)
    let carrier = "module Top(in clk: Bit, out q: Bit<bool> = false) { }\ndigital for Top { on clk.rise() { q = true; } }";
    let design = Design::builder().source("top.msl", carrier).build();
    assert!(design.diagnostics().is_empty(), "{:?}", support::codes(&design));
    let found = Types::check(&design, "Top").err().unwrap_or_default();
    assert!(found.iter().any(|d| d.diagnostic.code == "E0183" && d.diagnostic.message.contains("`q`") && d.diagnostic.message.contains("a carrier")), "{:?}", found.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
    // a local assigned from a second assignment without `mut` is `E0183` as well (§1.7.5)
    assert_refused(&source("", "let x; x = 1.0; x = 2.0;"), "E0183", "may be assigned twice");
}

#[test]
fn a_read_before_assignment_is_e0183() {
    // §1.7.5: a variable declared without a value is assigned on every path before it is read.
    assert_refused(&source("", "let x: f64; let y = x;"), "E0183", "`x` is read before it is assigned");
    assert_refused(&source("", "let x: f64; let c = true; if c { x = 1.0; } let y = x;"), "E0183", "`x` is read before it is assigned");
    assert_refused(&source("", "let mut x: f64; x += 1.0;"), "E0183", "`x` is read before it is assigned");
    assert_clean(&source("", "let x: f64; let c = true; if c { x = 1.0; } else { x = 2.0; } let y = x;"));
}

#[test]
fn let_mut_in_a_module_body_is_e0331() {
    // §8.2: a module body is not a block; `let mut` there is the parser's `E0331`.
    let design = support::build("module Top() { let mut x = 1.0; }");
    assert!(support::codes(&design).contains(&"E0331".to_string()), "{:?}", support::codes(&design));
}
