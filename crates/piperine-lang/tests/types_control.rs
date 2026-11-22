//! Mutation, loops, exits, patterns and function values in `piperine_lang::types` (feature
//! `composite-values`, CV-01 to CV-07; docs/spec Part I §1.5 to §1.8, §3). Each test states the
//! rule it proves; every source is a design of `Top` over an empty standard library.

#[path = "support/pom.rs"]
mod support;
#[allow(dead_code)]
#[path = "support/checked.rs"]
mod checked;

use checked::CheckedSource;

use piperine_lang::pom::Design;
use piperine_lang::types::{Ty, Types};

// ---- T11: places, `let mut` and assignment -------------------------------------------------

#[test]
fn let_mut_and_every_compound_operator_type_check() {
    // §1.5.12: `place = value` is `()`; `place op= value` is `place = place op value` for every
    // operator of level 15, in an `analog` block and in a function.
    let body = "let a = 1.0; a = 2.0; a += 1.0; a -= 1.0; a *= 2.0; a /= 2.0;\n\
                let n = 1u8; n = n + 1; n %= 3; n &= 1; n |= 2; n ^= 3; n <<= 1; n >>= 1;\n\
                let b = true; b &= false; b |= true; b ^= true;\n\
                let s = twice(a);";
    let items = "fn twice(x: f64) -> f64 { let y = x; y += x; y }";
    let src = CheckedSource::source(items, body);
    CheckedSource::assert_clean(&src);
    let types = CheckedSource::assignment_types(&src);
    assert_eq!(types.len(), 16, "every assignment is typed: {types:?}");
    assert!(types.iter().all(|t| *t == Some(Ty::Unit)), "an assignment has the type `()` (§1.5.12): {types:?}");
    // a deferred local takes the type of its first assignment and is assigned on both paths
    CheckedSource::assert_clean(&CheckedSource::source("", "let x; let c = true; if c { x = 1.0; } else { x = 2.0; } let y = x + 1.0;"));
    CheckedSource::assert_clean(&CheckedSource::source("", "let x: f64; x = 1.0; let y = x;"));
    // a mismatching value is an ordinary mismatch
    CheckedSource::assert_refused(&CheckedSource::source("", "let a = 1.0; a = true;"), "E0180", "expected `f64`");
}

#[test]
fn assigning_a_non_place_is_e0183_naming_the_target() {
    let items = "nature E { potential: V { access: v, units: \"V\", abstol: 1u }, flow: I { access: i, units: \"A\", abstol: 1p } }\nconst K: f64 = 1.0;";
    let top = |params: &str, body: &str| format!("{items}\nmodule Top({params}) {{ r: f64; let w = 1.0; }}\nanalog for Top {{ {body} }}");
    // §1.5.12: a param, a const, a function parameter, a body `let` and a value that is no place at all.
    CheckedSource::assert_refused(&top("", "r = 2.0;"), "E0183", "cannot assign to `r`: it is a param");
    CheckedSource::assert_refused(&CheckedSource::source(items, "K = 2.0;"), "E0183", "cannot assign to `K`: it is a `const`");
    CheckedSource::assert_refused(&CheckedSource::source("fn f(p: f64) -> f64 { p = 1.0; p }", "let y = f(1.0);"), "E0183", "cannot assign to `p`: it is a function parameter");
    CheckedSource::assert_refused(&top("", "w = 2.0;"), "E0183", "cannot assign to `w`: it is a body `let`");
    CheckedSource::assert_refused(&CheckedSource::source(items, "1.0 = 2.0;"), "E0183", "not a place");
    // a carrier, written in a reaction body (`Bit` is in the standard library)
    let carrier = "module Top(in clk: Bit, out q: Bit<bool> = false) { }\ndigital for Top { on clk.rise() { q = true; } }";
    let design = Design::builder().source("top.msl", carrier).build();
    assert!(design.diagnostics().is_empty(), "{:?}", support::codes(&design));
    let found = Types::check(&design, "Top").err().unwrap_or_default();
    assert!(found.iter().any(|d| d.diagnostic.code == "E0183" && d.diagnostic.message.contains("`q`") && d.diagnostic.message.contains("a carrier")), "{:?}", found.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
}

#[test]
fn a_read_before_assignment_is_e0183() {
    // §1.7.5: a variable declared without a value is assigned on every path before it is read.
    CheckedSource::assert_refused(&CheckedSource::source("", "let x: f64; let y = x;"), "E0183", "`x` is read before it is assigned");
    CheckedSource::assert_refused(&CheckedSource::source("", "let x: f64; let c = true; if c { x = 1.0; } let y = x;"), "E0183", "`x` is read before it is assigned");
    CheckedSource::assert_refused(&CheckedSource::source("", "let x: f64; x += 1.0;"), "E0183", "`x` is read before it is assigned");
    CheckedSource::assert_clean(&CheckedSource::source("", "let x: f64; let c = true; if c { x = 1.0; } else { x = 2.0; } let y = x;"));
}

// ---- T12: loops -----------------------------------------------------------------------------

#[test]
fn every_loop_form_type_checks() {
    // §1.8.7 to §1.8.10: `while`, `while let`, `loop` with `break` and `break value`, `for` over
    // a range, `continue`.
    CheckedSource::assert_clean(&CheckedSource::source("", "let i = 0u8; while i < 3 { i += 1; }"));
    CheckedSource::assert_clean(&CheckedSource::source("", "let o: Option<u8> = Some(1); let k = 0u8; while let Some(x) = o { k += x; }"));
    CheckedSource::assert_clean(&CheckedSource::source("", "let j = 0; loop { j += 1; if j > 2 { break; } }"));
    CheckedSource::assert_clean(&CheckedSource::source("", "for i in 0..3 { continue; } for i in 0u8..=3 { let y = i + 1; }"));
    CheckedSource::assert_clean(&CheckedSource::source("", "let s = 0u8; for i in 0u8..4 { i += 1; s += i; } for _ in 0..2 { s += 1; }"));
    // `loop` takes the type of its `break` values (§1.8.8), from the position or from the break
    CheckedSource::assert_clean(&CheckedSource::source("", "let v: u8 = loop { break 7; }; let w = loop { break 2.0; }; let z = w + 1.0;"));
    CheckedSource::assert_clean(&CheckedSource::source("", "let u = loop { break; }; let t: () = u;"));
    // a `loop` with no `break` never completes, so it fits any type (§1.5.16)
    CheckedSource::assert_clean(&CheckedSource::source("fn spin() -> f64 { loop { } }", "let x = spin();"));
    // `while` and `for` are `()`; a deferred local assigned before a `loop` breaks is assigned after it
    CheckedSource::assert_clean(&CheckedSource::source("", "let x: f64; loop { x = 1.0; break; } let y = x;"));
    // `break` and `continue` leave the innermost loop of a nested pair
    CheckedSource::assert_clean(&CheckedSource::source("", "let n = 0; while n < 3 { n += 1; for i in 0..2 { if i == 1 { break; } continue; } }"));
    // a mismatching `break` value, an integer range over floats, a non-bool condition
    CheckedSource::assert_refused(&CheckedSource::source("", "let v = loop { if true { break 1u8; } break 2u16; };"), "E0180", "expected `u8`");
    CheckedSource::assert_refused(&CheckedSource::source("", "for i in 0.0..1.0 { }"), "E0180", "an integer type");
    CheckedSource::assert_refused(&CheckedSource::source("", "while 1 { }"), "E0180", "expected `bool`");
}

#[test]
fn break_or_continue_outside_a_loop_is_e0185() {
    // §1.8.10, §1.8.12: outside a loop, with a value outside a `loop`, and across a closure.
    CheckedSource::assert_refused(&CheckedSource::source("", "break;"), "E0185", "`break` outside a loop");
    CheckedSource::assert_refused(&CheckedSource::source("", "continue;"), "E0185", "`continue` outside a loop");
    CheckedSource::assert_refused(&CheckedSource::source("", "while true { break 1; }"), "E0185", "`break` with a value outside a `loop`");
    CheckedSource::assert_refused(&CheckedSource::source("", "for i in 0..2 { break 1; }"), "E0185", "`break` with a value outside a `loop`");
    CheckedSource::assert_refused(&CheckedSource::source("fn f() -> f64 { break; 1.0 }", "let x = f();"), "E0185", "`break` outside a loop");
    // a refutable pattern in a `for` is `E0184` (§1.6.11)
    CheckedSource::assert_refused(&CheckedSource::source("", "for 3 in 0..2 { }"), "E0184", "irrefutable");
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
                 fn in_loop(x: u8) -> u8 { let i = 0u8; while i < 9 { if i == x { return i; } i += 1; } 0 }";
    CheckedSource::assert_clean(&CheckedSource::source(items, "let a = early(1.0); unit(1.0); let b = both(1.0); let c = each(1.0); let d = in_loop(2);"));
    // §1.5.15: `e?` is the `T` of an `Option<T>`, in a function whose result is an `Option`
    let option = "fn first(a: Option<u8>, b: Option<u8>) -> Option<u8> { let x = a?; let y = b?; Some(x + y) }\n\
                  fn chained(a: Option<u8>) -> Option<u8> { Some(a? + 1) }";
    CheckedSource::assert_clean(&CheckedSource::source(option, "let s = first(Some(1), None); let t = chained(Some(1));"));
    // a `return` of the wrong type, and a missing value, are mismatches
    CheckedSource::assert_refused(&CheckedSource::source("fn f(x: f64) -> f64 { return true; }", "let y = f(1.0);"), "E0180", "expected `f64`");
    CheckedSource::assert_refused(&CheckedSource::source("fn f(x: f64) -> f64 { return; }", "let y = f(1.0);"), "E0180", "expected `f64`");
    // the operand of `?` is an `Option`
    CheckedSource::assert_refused(&CheckedSource::source("fn f(a: u8) -> Option<u8> { let x = a?; Some(x) }", "let y = f(1);"), "E0180", "an `Option`");
}

#[test]
fn return_outside_a_function_is_e0185() {
    // §1.8.11, §1.5.15: `return` and `?` belong to a function or a closure.
    CheckedSource::assert_refused(&CheckedSource::source("", "return;"), "E0185", "`return` outside a function or a closure");
    CheckedSource::assert_refused(&CheckedSource::source("", "let o: Option<u8> = Some(1); let x = o?;"), "E0185", "`?` outside a function or a closure");
    // `?` in a function whose result is not an `Option`
    CheckedSource::assert_refused(&CheckedSource::source("fn f(o: Option<u8>) -> u8 { let x = o?; x }", "let y = f(Some(1));"), "E0185", "not an `Option`");
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
    CheckedSource::assert_clean(&CheckedSource::source(items, "let a = classify(1); let b = floats(1.0); let c = payload(None); let d = named(2);"));
    // a guard is a `bool`
    CheckedSource::assert_refused(&CheckedSource::source("fn f(n: u8) -> u8 { match n { x if x => 1, _ => 0 } }", "let y = f(1);"), "E0180", "expected `bool`");
    // a range over an enum is a plain mismatch (D6)
    let en = "#[derive(Repr)]\nenum Mode { A, B }\nfn f(m: Mode) -> u8 { match m { Mode::A..=Mode::B => 1, _ => 0 } }";
    CheckedSource::assert_refused(&CheckedSource::source(en, "let y = f(Mode::A);"), "E0180", "range pattern");
}

#[test]
fn alternatives_binding_differently_are_e0184() {
    let f = |arms: &str| CheckedSource::source(&format!("fn f(o: Option<u8>) -> u8 {{ match o {{ {arms} }} }}"), "let y = f(None);");
    // §1.6.8: one alternative binds `n`, the other nothing; or each binds another name
    CheckedSource::assert_refused(&f("Some(n) | None => 0, _ => 1,"), "E0184", "bind different names");
    CheckedSource::assert_refused(&f("Some(a @ 1..=3) | Some(b @ 4..=9) => 0, _ => 1,"), "E0184", "bind different names");
    // §1.6.10: a name bound twice in one pattern
    CheckedSource::assert_refused(&f("n @ Some(n) => 0, _ => 1,"), "E0184", "bound twice");
    // §1.6.4: an empty range pattern
    CheckedSource::assert_refused(&f("Some(5..=1) => 0, _ => 1,"), "E0184", "empty range");
    CheckedSource::assert_refused(&f("Some(3..3) => 0, _ => 1,"), "E0184", "empty range");
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
    CheckedSource::assert_clean(&CheckedSource::source(items, "let r = a(1) + b(1) + c(1) + d(1) + e(1) + g(1) + h(1);"));
    // one value short of the type's range is not exhaustive
    let short = CheckedSource::source("fn a(x: u8) -> u8 { match x { 0..=254 => 1 } }", "let y = a(1);");
    let found = CheckedSource::outcome(&short);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].1.contains("255"), "{found:?}");
    let guarded = CheckedSource::source("fn a(x: u8) -> u8 { match x { 0..=255 if true => 1 } }", "let y = a(1);");
    assert_eq!(CheckedSource::outcome(&guarded).len(), 1, "a guarded arm covers nothing");
}

// ---- T15: refutable patterns and `E0184` ----------------------------------------------------------

#[test]
fn refutable_patterns_type_check_in_if_let_while_let_and_let_else() {
    // §1.8.2, §1.7.6: `if let`, `while let` and `let … else` take any refutable pattern: literal,
    // range, `None`, `Some` of a nested pattern, an enum variant.
    let items = "#[derive(Repr, PartialEq)]\nenum Mode { A, B }\n\
                 fn lit(n: u8) -> u8 { if let 3 = n { 1 } else { 0 } }\n\
                 fn range(n: u8) -> u8 { if let 4..=9 = n { 1 } else { 0 } }\n\
                 fn none(o: Option<u8>) -> u8 { if let None = o { 1 } else { 0 } }\n\
                 fn nested(o: Option<u8>) -> u8 { if let Some(1..=3) = o { 1 } else { 0 } }\n\
                 fn variant(m: Mode) -> u8 { if let Mode::B = m { 1 } else { 0 } }\n\
                 fn bound(o: Option<u8>) -> u8 { if let Some(x @ 5..) = o { x } else { 0 } }\n\
                 fn looped(o: Option<u8>) -> u8 { let n = 0u8; while let Some(x) = o { n += x; break; } n }\n\
                 fn guarded(o: Option<u8>) -> u8 { let Some(x) = o else { return 0; }; x }\n\
                 fn ranged(n: u8) -> u8 { let 1..=9 = n else { return 0; }; n }\n\
                 fn looping(o: Option<u8>) -> u8 { let s = 0u8; for i in 0u8..3 { let Some(x) = o else { continue; }; s += x + i; } s }";
    let body = "let a = lit(1) + range(1) + none(None) + nested(None) + variant(Mode::A) + bound(None) + looped(None) + guarded(None) + ranged(1) + looping(None);";
    CheckedSource::assert_clean(&CheckedSource::source(items, body));
    // irrefutable patterns need no `else`: a name, `_`, and a pattern that covers every value
    CheckedSource::assert_clean(&CheckedSource::source("", "let _ = 1.0; let n @ 0..=255 = 3u8; let (lo, hi) = (0.0, 1.0);"));
}

#[test]
fn a_non_exhaustive_match_or_refutable_let_is_e0184() {
    // §1.8.6: a `match` that leaves a value out is `E0184` naming one missing pattern.
    // each source reaches `f` from `Top`, so the checker visits its `match`
    let reach = |ty: &str, arms: &str, arg: &str| CheckedSource::source(&format!("#[derive(Repr, PartialEq)]\nenum Mode {{ A, B, C }}\nfn f(x: {ty}) -> u8 {{ match x {{ {arms} }} }}"), &format!("let y = f({arg});"));
    CheckedSource::assert_refused(&reach("bool", "true => 1,", "true"), "E0184", "`false` is not covered");
    CheckedSource::assert_refused(&reach("Mode", "Mode::A => 1, Mode::C => 2,", "Mode::A"), "E0184", "`Mode::B` is not covered");
    CheckedSource::assert_refused(&reach("Option<u8>", "Some(_) => 1,", "None"), "E0184", "`None` is not covered");
    CheckedSource::assert_refused(&reach("Option<bool>", "Some(true) => 1, None => 2,", "None"), "E0184", "`Some(false)` is not covered");
    CheckedSource::assert_refused(&reach("u8", "0..=9 => 1,", "1"), "E0184", "`10..=255` is not covered");
    CheckedSource::assert_refused(&reach("f64", "0.0..=1.0 => 1,", "1.0"), "E0184", "`_` is not covered");
    // a guarded arm covers nothing
    CheckedSource::assert_refused(&reach("bool", "true if true => 1, false => 2,", "true"), "E0184", "`true` is not covered");
    // §1.6.11: a refutable pattern in `let` without `else`, in a `for` and in a closure parameter
    CheckedSource::assert_refused(&CheckedSource::source("", "let o: Option<u8> = None; let Some(x) = o;"), "E0184", "`None`");
    CheckedSource::assert_refused(&CheckedSource::source("", "let n = 3u8; let 1..=9 = n;"), "E0184", "irrefutable");
    CheckedSource::assert_refused(&CheckedSource::source("", "let o: Option<u8> = None; let y = o.map(|3| 1u8);"), "E0184", "a closure parameter");
    // §1.7.6: the `else` block of a `let … else` must not complete
    CheckedSource::assert_refused(&CheckedSource::source("", "let o: Option<u8> = None; let Some(x) = o else { let z = 1; };"), "E0185", "can complete");
}

// ---- T16: closures anywhere -----------------------------------------------------------------------

const ELECTRICAL: &str = "nature E { potential: V { access: v, units: \"V\", abstol: 1u }, flow: I { access: i, units: \"A\", abstol: 1p } }\n";

#[test]
fn closures_type_check_where_rust_allows_them() {
    // §1.5.14, §3 rule 3: a closure bound by `let` and called, with its parameter types written, found
    // from the first call, or from a `return`; returned in a block; passed to a row of H.4.
    let body = "let k = 2.0;\n\
                let f = |x: f64| x * k;\n\
                let a = f(1.0);\n\
                let g = |x| x + 1.0;\n\
                let b = g(2.0);\n\
                let h = |x: u8, y: u8| -> u8 { x + y };\n\
                let c = h(1, 2);\n\
                let blk = { let m = 3.0; |x: f64| x * m };\n\
                let d = blk(1.0);\n\
                let o: Option<u8> = Some(1);\n\
                let e = o.map(|v| v + 1);\n\
                let early = |x: f64| -> f64 { if x > 1.0 { return 1.0; } x };\n\
                let r = early(2.0);\n\
                let unit = || 4.0;\n\
                let s = unit() + a + b + d + r;";
    CheckedSource::assert_clean(&CheckedSource::source("", body));
    // `limit` takes a closure: `limit(v(p), |a, b| a)`
    CheckedSource::assert_clean(&format!("{ELECTRICAL}module Top(p: E) {{ }}\nanalog for Top {{ let y = limit(v(p), |a, b| a); }}"));
    // a closure may be passed to a parameter of function type
    CheckedSource::assert_clean(&CheckedSource::source("fn apply(f: fn(f64) -> f64, x: f64) -> f64 { f(x) }", "let y = apply(|x: f64| x + 1.0, 2.0);"));
    // a call is typed from the closure's own signature
    CheckedSource::assert_refused(&CheckedSource::source("", "let f = |x: f64| x; let y = f(true);"), "E0180", "expected `f64`");
    // a parameter with no written type and no call is `E0181`; so is one with no `let` to find a call
    CheckedSource::assert_refused(&CheckedSource::source("", "let g = |x| x;"), "E0181", "never called");
    let used = CheckedSource::outcome(&CheckedSource::source("", "let g = |x| x; let h = g;"));
    assert!(!used.is_empty() && used.iter().all(|d| d.0 == "E0181"), "{used:?}");
    // the closure's value is the `Fn` type, so a mismatching annotation is a mismatch
    CheckedSource::assert_refused(&CheckedSource::source("", "let f: u8 = |x: f64| x;"), "E0180", "expected `u8`");
}

#[test]
fn a_closure_called_outside_its_class_is_e0122() {
    // §3 table: a solve-reading closure (it reads `now()`) is not callable in a const context, an
    // emitting one (it reaches `emit!`) not in a module body.
    let solve = "module Top() { r: f64 = { let f = |x: f64| x + now(); f(1.0) }; }";
    CheckedSource::assert_refused(solve, "E0122", "solve-reading");
    let emit = "module Top() { let f = |x: f64| { emit!(\"trace\", v = x); x }; let y = f(1.0); }";
    CheckedSource::assert_refused(emit, "E0122", "emitting");
    // the same closures are callable in `analog`
    CheckedSource::assert_clean(&CheckedSource::source("", "let f = |x: f64| x + now(); let y = f(1.0);"));
    // a function the closure calls gives it its class
    let through = "fn tick() -> f64 { now() }\nmodule Top() { r: f64 = { let f = || tick(); f() }; }";
    CheckedSource::assert_refused(through, "E0122", "solve-reading");
}

#[test]
fn a_closure_with_an_excluded_form_in_a_fn_is_e0121() {
    // §3 rule 6: a closure inside a function follows the function's rule; the parser reports it as
    // `E0121`, never `E0404`.
    let design = support::build("fn f() -> f64 { let g = || { q <- 1.0; }; 1.0 }");
    let codes = support::codes(&design);
    assert!(codes.contains(&"E0121".to_string()), "{codes:?}");
    assert!(!codes.contains(&"E0404".to_string()), "{codes:?}");
}

// ---- T17: function values ---------------------------------------------------------------------------------

const APPLY: &str = "fn h(a: f64, b: f64) -> f64 { a }\nfn apply(f: fn(f64) -> f64, x: f64) -> f64 { f(x) }\nfn twice(g: fn(f64, f64) -> f64, x: f64) -> f64 { g(x, x) }";

#[test]
fn a_function_name_is_a_function_value() {
    // §1.2.10: a function's name is a value of type `fn(T..) -> U`, bound, called, passed and chosen.
    let src = CheckedSource::source(APPLY, "let f = h; let r = f(1.0, 2.0); let t = twice(h, 1.0); let u = twice(if true { h } else { h }, 1.0);");
    assert_eq!(CheckedSource::local_type(&src, "f"), Ty::Fn(vec![Ty::F64, Ty::F64], Box::new(Ty::F64)));
    assert_eq!(CheckedSource::local_type(&src, "r"), Ty::F64);
    // a call through the value is typed from its signature
    CheckedSource::assert_refused(&CheckedSource::source(APPLY, "let f = h; let r = f(1.0);"), "E0141", "expected 2 argument(s)");
    // a function of another signature is no `fn(f64) -> f64`
    CheckedSource::assert_refused(&CheckedSource::source(APPLY, "let r = apply(h, 1.0);"), "E0180", "expected `fn(f64) -> f64`");
    // a parameter of function type is called
    CheckedSource::assert_clean(&CheckedSource::source("fn run(f: fn(f64) -> f64, x: f64) -> f64 { f(x) }", "let y = run(|a: f64| a, 1.0);"));
}

#[test]
fn a_non_capturing_closure_is_a_function_pointer() {
    // §1.2.10: a closure that captures nothing is accepted where a function pointer is expected.
    CheckedSource::assert_clean(&CheckedSource::source(APPLY, "let y = apply(|x: f64| x + 1.0, 2.0);"));
    CheckedSource::assert_clean(&CheckedSource::source(APPLY, "let c = |x: f64| x + 1.0; let y = apply(c, 2.0);"));
    CheckedSource::assert_clean(&CheckedSource::source(APPLY, "let g: fn(f64) -> f64 = |x: f64| x * 2.0; let y = g(1.0);"));
}

#[test]
fn a_capturing_closure_is_not_a_function_pointer() {
    // §1.2.10: a closure that captures is no function pointer, as in Rust.
    CheckedSource::assert_refused(&CheckedSource::source(APPLY, "let k = 2.0; let y = apply(|x: f64| x * k, 1.0);"), "P0001", "captures");
    CheckedSource::assert_refused(&CheckedSource::source(APPLY, "let k = 2.0; let c = |x: f64| x * k; let y = apply(c, 1.0);"), "P0001", "captures");
    CheckedSource::assert_refused(&CheckedSource::source("", "let k = 2.0; let g: fn(f64) -> f64 = |x: f64| x * k;"), "P0001", "captures");
}

#[test]
fn a_function_value_is_not_carried_e0112() {
    // §1 rule 9: a function value or a closure has no `Repr`: not as the type of a carrier, not as
    // the value committed to one.
    let in_type = "module Top(out q: Bit<fn(f64) -> f64>) { }";
    let design = Design::builder().source("top.msl", in_type).build();
    let found = Types::check(&design, "Top").err().unwrap_or_default();
    assert!(found.iter().any(|d| d.diagnostic.code == "E0112"), "{:?}", found.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
    let committed = "fn id(x: f64) -> f64 { x }\nmodule Top(in clk: Bit, out q: Bit<f64> = 0.0) { }\ndigital for Top { on clk.rise() { q <- id; let f = |x: f64| x; q <- f; } }";
    let design = Design::builder().source("top.msl", committed).build();
    assert!(design.diagnostics().is_empty(), "{:?}", support::codes(&design));
    let found = Types::check(&design, "Top").err().unwrap_or_default();
    assert_eq!(found.iter().filter(|d| d.diagnostic.code == "E0112").count(), 2, "{:?}", found.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
}

// ---- T18: recursion ---------------------------------------------------------------------------------------

#[test]
fn direct_and_mutual_recursion_type_check() {
    // §3 rule 12: a function that calls itself, directly or through others, type-checks; a call is
    // typed from the signature, so the recursion ends.
    let items = "fn fact(n: u8) -> u8 { if n <= 1 { 1 } else { n * fact(n - 1) } }\n\
                 fn even(n: u8) -> bool { if n == 0 { true } else { odd(n - 1) } }\n\
                 fn odd(n: u8) -> bool { if n == 0 { false } else { even(n - 1) } }";
    CheckedSource::assert_clean(&CheckedSource::source(items, "let f = fact(5); let e = even(4); let o = odd(3);"));
    // the mismatch inside a recursive body is still a mismatch
    CheckedSource::assert_refused(&CheckedSource::source("fn bad(n: u8) -> u8 { if n == 0 { 1 } else { bad(n - 1) + true } }", "let y = bad(2);"), "E0180", "expected `u8`");
    // the class of a recursive function is the fixed point of its body's classes (§3): `a` reads the
    // solve and `b` only calls `a`, so a closure calling `b` is solve-reading, and not callable in a
    // const context
    let cycle = "fn a(n: u8) -> f64 { if n == 0 { now() } else { b(n - 1) } }\nfn b(n: u8) -> f64 { a(n) }\n\
                 module Top() { r: f64 = { let f = || b(3); f() }; }";
    CheckedSource::assert_refused(cycle, "E0122", "solve-reading");
    // a recursion that reads nothing of the solve is pure: callable there
    let pure = "fn p(n: u8) -> f64 { if n == 0 { 1.0 } else { q(n - 1) } }\nfn q(n: u8) -> f64 { p(n) }\n\
                module Top() { r: f64 = { let f = || q(3); f() }; }";
    CheckedSource::assert_clean(pure);
}
