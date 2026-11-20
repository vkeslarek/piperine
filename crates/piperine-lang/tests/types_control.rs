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

// ---- T12: loops -----------------------------------------------------------------------------

#[test]
fn every_loop_form_type_checks() {
    // §1.8.7 to §1.8.10: `while`, `while let`, `loop` with `break` and `break value`, `for` over
    // a range, `continue`.
    assert_clean(&source("", "let mut i = 0u8; while i < 3 { i += 1; }"));
    assert_clean(&source("", "let o: Option<u8> = Some(1); let mut k = 0u8; while let Some(x) = o { k += x; }"));
    assert_clean(&source("", "let mut j = 0; loop { j += 1; if j > 2 { break; } }"));
    assert_clean(&source("", "for i in 0..3 { continue; } for i in 0u8..=3 { let y = i + 1; }"));
    assert_clean(&source("", "let mut s = 0u8; for mut i in 0u8..4 { i += 1; s += i; } for _ in 0..2 { s += 1; }"));
    // `loop` takes the type of its `break` values (§1.8.8), from the position or from the break
    assert_clean(&source("", "let v: u8 = loop { break 7; }; let w = loop { break 2.0; }; let z = w + 1.0;"));
    assert_clean(&source("", "let u = loop { break; }; let t: () = u;"));
    // a `loop` with no `break` never completes, so it fits any type (§1.5.16)
    assert_clean(&source("fn spin() -> f64 { loop { } }", "let x = spin();"));
    // `while` and `for` are `()`; a deferred local assigned before a `loop` breaks is assigned after it
    assert_clean(&source("", "let x: f64; loop { x = 1.0; break; } let y = x;"));
    // `break` and `continue` leave the innermost loop of a nested pair
    assert_clean(&source("", "let mut n = 0; while n < 3 { n += 1; for i in 0..2 { if i == 1 { break; } continue; } }"));
    // a mismatching `break` value, an integer range over floats, a non-bool condition
    assert_refused(&source("", "let v = loop { if true { break 1u8; } break 2u16; };"), "P0002", "expected `u8`");
    assert_refused(&source("", "for i in 0.0..1.0 { }"), "P0002", "an integer type");
    assert_refused(&source("", "while 1 { }"), "P0002", "expected `bool`");
}

#[test]
fn break_or_continue_outside_a_loop_is_e0185() {
    // §1.8.10, §1.8.12: outside a loop, with a value outside a `loop`, and across a closure.
    assert_refused(&source("", "break;"), "E0185", "`break` outside a loop");
    assert_refused(&source("", "continue;"), "E0185", "`continue` outside a loop");
    assert_refused(&source("", "while true { break 1; }"), "E0185", "`break` with a value outside a `loop`");
    assert_refused(&source("", "for i in 0..2 { break 1; }"), "E0185", "`break` with a value outside a `loop`");
    assert_refused(&source("fn f() -> f64 { break; 1.0 }", "let x = f();"), "E0185", "`break` outside a loop");
    // assigning an immutable deferred local from a `while` body may assign it twice (§1.7.5)
    assert_refused(&source("", "let x: f64; while true { x = 1.0; }"), "E0183", "may be assigned twice");
    // a refutable pattern in a `for` is `E0184` (§1.6.11)
    assert_refused(&source("", "for 3 in 0..2 { }"), "E0184", "irrefutable");
}

// ---- T13: `return` and `?` ------------------------------------------------------------------

#[test]
fn return_and_question_mark_type_check() {
    // §1.8.11: `return value` has the result type, `return` alone `()`; the value of the body is the
    // result when no `return` runs. §1.5.16: `return` fits any required type.
    let items = "fn early(x: f64) -> f64 { if x > 1.0 { return 1.0; } x }\n\
                 fn unit(x: f64) { if x > 1.0 { return; } }\n\
                 fn both(x: f64) -> f64 { let y = if x > 0.0 { x } else { return 0.0 }; y }\n\
                 fn each(x: f64) -> f64 { match x > 0.0 { true => return 1.0, false => 2.0 } }\n\
                 fn in_loop(x: u8) -> u8 { let mut i = 0u8; while i < 9 { if i == x { return i; } i += 1; } 0 }";
    assert_clean(&source(items, "let a = early(1.0); unit(1.0); let b = both(1.0); let c = each(1.0); let d = in_loop(2);"));
    // §1.5.15: `e?` is the `T` of an `Option<T>`, in a function whose result is an `Option`
    let option = "fn first(a: Option<u8>, b: Option<u8>) -> Option<u8> { let x = a?; let y = b?; Some(x + y) }\n\
                  fn chained(a: Option<u8>) -> Option<u8> { Some(a? + 1) }";
    assert_clean(&source(option, "let s = first(Some(1), None); let t = chained(Some(1));"));
    // a `return` of the wrong type, and a missing value, are mismatches
    assert_refused(&source("fn f(x: f64) -> f64 { return true; }", "let y = f(1.0);"), "P0002", "expected `f64`");
    assert_refused(&source("fn f(x: f64) -> f64 { return; }", "let y = f(1.0);"), "P0002", "expected `f64`");
    // the operand of `?` is an `Option`
    assert_refused(&source("fn f(a: u8) -> Option<u8> { let x = a?; Some(x) }", "let y = f(1);"), "P0002", "an `Option`");
}

#[test]
fn return_outside_a_function_is_e0185() {
    // §1.8.11, §1.5.15: `return` and `?` belong to a function or a closure.
    assert_refused(&source("", "return;"), "E0185", "`return` outside a function or a closure");
    assert_refused(&source("", "let o: Option<u8> = Some(1); let x = o?;"), "E0185", "`?` outside a function or a closure");
    // `?` in a function whose result is not an `Option`
    assert_refused(&source("fn f(o: Option<u8>) -> u8 { let x = o?; x }", "let y = f(Some(1));"), "E0185", "not an `Option`");
}

// ---- T14: guards, range patterns, or-bindings and `@` ------------------------------------------

#[test]
fn guards_ranges_or_bindings_and_at_type_check() {
    // §1.6.4, §1.6.8, §1.6.2, §1.8.3: ranges over integers and floats (closed, half-open, open),
    // or-patterns whose alternatives bind the same names, `name @ p`, and a guard that sees the
    // names of its pattern.
    let items = "const LIMIT: u8 = 100;\n\
                 fn classify(code: u8) -> u8 { match code { 0 => 1, 1..=9 => 2, 10 | 11 => 3, n @ 12..=15 => n, n if n > LIMIT => n, 16..LIMIT => 4, _ => 0 } }\n\
                 fn floats(x: f64) -> u8 { match x { 0.0..=1.0 => 1, 2.0.. => 2, ..=-1.0 => 3, _ => 0 } }\n\
                 fn payload(o: Option<u8>) -> u8 { match o { Some(n @ 1..=3) | Some(n @ 7..=9) => n, Some(0) | None => 0, Some(n) => n } }\n\
                 fn named(n: u8) -> u8 { match n { whole => whole + 1 } }";
    assert_clean(&source(items, "let a = classify(1); let b = floats(1.0); let c = payload(None); let d = named(2);"));
    // a guard is a `bool`
    assert_refused(&source("fn f(n: u8) -> u8 { match n { x if x => 1, _ => 0 } }", "let y = f(1);"), "P0002", "expected `bool`");
    // a range over an enum is a plain mismatch (D6)
    let en = "#[derive(Repr)]\nenum Mode { A, B }\nfn f(m: Mode) -> u8 { match m { Mode::A..=Mode::B => 1, _ => 0 } }";
    assert_refused(&source(en, "let y = f(Mode::A);"), "P0002", "range pattern");
}

#[test]
fn alternatives_binding_differently_are_e0184() {
    let f = |arms: &str| source(&format!("fn f(o: Option<u8>) -> u8 {{ match o {{ {arms} }} }}"), "let y = f(None);");
    // §1.6.8: one alternative binds `n`, the other nothing; or each binds another name
    assert_refused(&f("Some(n) | None => 0, _ => 1,"), "E0184", "bind different names");
    assert_refused(&f("Some(a @ 1..=3) | Some(b @ 4..=9) => 0, _ => 1,"), "E0184", "bind different names");
    // §1.6.10: a name bound twice in one pattern
    assert_refused(&f("n @ Some(n) => 0, _ => 1,"), "E0184", "bound twice");
    // §1.6.4: an empty range pattern
    assert_refused(&f("Some(5..=1) => 0, _ => 1,"), "E0184", "empty range");
    assert_refused(&f("Some(3..3) => 0, _ => 1,"), "E0184", "empty range");
}

#[test]
fn an_integer_match_covered_by_ranges_is_exhaustive() {
    // §1.8.6: for an integer type every value of its range, by literals and ranges, at any width up
    // to 64; a guarded arm covers nothing.
    let items = "fn a(x: u8) -> u8 { match x { 0..=127 => 1, 128..=255 => 2 } }\n\
                 fn b(x: i8) -> u8 { match x { ..=-1 => 1, 0 => 2, 1.. => 3 } }\n\
                 fn c(x: u64) -> u8 { match x { 0..=9 => 1, 10.. => 2 } }\n\
                 fn d(x: i16) -> u8 { match x { -32768..=0 | 1..=32767 => 1 } }\n\
                 fn e(x: i64) -> u8 { match x { ..=0 => 1, 1.. => 2 } }\n\
                 fn g(x: u32) -> u8 { match x { 0..=4294967295 => 1 } }\n\
                 fn h(x: u8) -> u8 { match x { n if n > 3 => 1, _ => 2 } }";
    assert_clean(&source(items, "let r = a(1) + b(1) + c(1) + d(1) + e(1) + g(1) + h(1);"));
    // one value short of the type's range is not exhaustive
    let short = source("fn a(x: u8) -> u8 { match x { 0..=254 => 1 } }", "let y = a(1);");
    let found = outcome(&short);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].1.contains("255"), "{found:?}");
    let guarded = source("fn a(x: u8) -> u8 { match x { 0..=255 if true => 1 } }", "let y = a(1);");
    assert_eq!(outcome(&guarded).len(), 1, "a guarded arm covers nothing");
}
