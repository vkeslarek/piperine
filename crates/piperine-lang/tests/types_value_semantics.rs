//! Places and copies in `piperine_lang::types` (feature `value-semantics`, VS-03; docs/spec Part I
//! §1.5.12, §1.6.9, §1.7.5, §1.7.7, §3.1.10). Each test states the rule it proves; every source is
//! a design of `Top` over an empty standard library.

#[path = "support/pom.rs"]
mod support;

use piperine_lang::pom::Design;
use piperine_lang::types::Types;

/// A design of `Top` with `items` before it and `body` as its analog block.
fn source(items: &str, body: &str) -> String {
    format!("{items}\nmodule Top() {{ }}\nanalog for Top {{ {body} }}")
}

/// The diagnostics of checking `Top`, as `(code, message)`; empty when the check passes.
fn outcome(src: &str) -> Vec<(String, String)> {
    let design: Design = support::build(src);
    assert!(design.diagnostics().is_empty(), "the source must parse and resolve: {:?}\n{src}", support::codes(&design));
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

#[test]
fn a_let_without_mut_is_assigned_with_every_operator() {
    // §1.5.12, §1.7.7 (VS-03 AC 1): a `let` with no marker is a place for `=` and every compound operator,
    // and so is a field or an element of it.
    assert_clean(&source("", "let x = 1.0; x = 2.0; x += 1.0; x -= 1.0; x *= 2.0; x /= 2.0;"));
    assert_clean(&source("", "let n = 6u32; n %= 4; n &= 3; n |= 8; n ^= 1; n <<= 1; n >>= 1;"));
    assert_clean(&source("struct P { a: f64 }", "let p = P { a: 1.0 }; p.a = 2.0; p.a += 1.0;"));
    assert_clean(&source("", "let t = [1.0, 2.0]; t[0] = 3.0; t[1] += 1.0;"));
}

#[test]
fn a_parameter_a_capture_and_a_body_let_are_not_places() {
    // §1.5.12, §3.1.10 (VS-03 AC 2): a parameter of a function or of a closure, a name a closure captures, a
    // body `let`, a `const`, a param: each is `E0183`, naming the target and what it is.
    let items = "const K: f64 = 1.0;";
    let top = |body: &str| format!("{items}\nmodule Top() {{ r: f64; let w = 1.0; }}\nanalog for Top {{ {body} }}");
    assert_refused(&source("fn f(p: f64) -> f64 { p = 1.0; p }", "let y = f(1.0);"), "E0183", "cannot assign to `p`: it is a function parameter");
    assert_refused(&source("", "let f = |p: f64| { p = 1.0; p }; let y = f(1.0);"), "E0183", "cannot assign to `p`: it is a closure parameter");
    assert_refused(&source("", "let c = 1.0; let f = |d: f64| { c = d; d }; let y = f(2.0);"), "E0183", "cannot assign to `c`: it is a name a closure captures");
    assert_refused(&top("w = 2.0;"), "E0183", "cannot assign to `w`: it is a body `let`");
    assert_refused(&top("r = 2.0;"), "E0183", "cannot assign to `r`: it is a param");
    assert_refused(&top("K = 2.0;"), "E0183", "cannot assign to `K`: it is a `const`");
}

#[test]
fn a_pattern_bound_name_is_a_place() {
    // §1.6.9 (VS-03 AC 3): the names a `match` arm, an `if let`, a `while let` and a `for` bind are places.
    assert_clean(&source("", "let o = Some(1.0); match o { Some(v) => { v += 1.0; } None => {} }"));
    assert_clean(&source("", "let o = Some(1.0); if let Some(v) = o { v *= 2.0; }"));
    assert_clean(&source("", "let o = Some(1u32); while let Some(v) = o { v += 1; break; }"));
    assert_clean(&source("", "for i in 0..3 { i += 1; }"));
}

#[test]
fn a_deferred_let_is_assigned_twice_and_never_read_first() {
    // §1.7.5 (VS-03 AC 4): `let x;` takes any number of assignments; a read before an assignment on some path
    // stays `E0183`.
    assert_clean(&source("", "let x; x = 1.0; x = 2.0; let y = x;"));
    assert_clean(&source("", "let x: f64; while true { x = 1.0; break; }"));
    assert_clean(&source("", "let x: f64; let c = true; if c { x = 1.0; } else { x = 2.0; } x = 3.0; let y = x;"));
    assert_refused(&source("", "let x: f64; let y = x;"), "E0183", "`x` is read before it is assigned");
    assert_refused(&source("", "let x: f64; let c = true; if c { x = 1.0; } let y = x;"), "E0183", "`x` is read before it is assigned");
}

#[test]
fn str_is_a_value_type_in_every_position() {
    // §1.2.9 (VS-07 AC 1): `str` is the type of a parameter, a `const`, a local, the payload of an `Option`, a
    // struct field and a record field; two strings compare with `==` and `!=`; a derive of `Repr` over a `str`
    // field is `E0110`.
    assert_clean(&source("fn tag(name: str) -> bool { name == \"a\" }", "let t = tag(\"a\");"));
    assert_clean(&source("const NAME: str = \"a\";\nfn same(name: str) -> bool { name != NAME }", "let t = same(\"b\");"));
    assert_clean(&source("", "let s = \"abc\"; let t = s == \"abc\"; let u = s != \"x\";"));
    assert_clean(&source("fn pick(name: Option<str>) -> bool { true }", "let t = pick(Some(\"a\")); let u = pick(None);"));
    assert_clean(&source("struct Tagged { name: str, level: f64 }", "let t = Tagged { name: \"a\", level: 1.0 }; let s = t.name == \"a\";"));
    assert_clean("module Top() { }\ndigital for Top { on at(1u) { emit!(\"log\", name = \"a\", level = 1.0); } }");
    assert_refused(&source("#[derive(Repr)] struct Tagged { name: str }", "let t = Tagged { name: \"a\" };"), "E0110", "the field `name` has type `str`");
}

#[test]
fn a_range_index_is_an_array_of_the_folded_length() {
    // §1.5.9 (VS-06 AC 1): `e[a..b]`, `e[a..=b]`, `e[..b]`, `e[a..]` and `e[..]` give `[T; b - a]`; a bound folds
    // through a `const` and arithmetic. The result is passed where an array of that length is wanted.
    let items = "const TAPS: [f64; 4] = [0.25, 0.5, 0.5, 0.25];\nconst LOW: usize = 1;\nfn pair(xs: [f64; 2]) -> f64 { xs[0] + xs[1] }\nfn all(xs: [f64; 4]) -> f64 { xs[0] }\nfn tail(xs: [f64; 3]) -> f64 { xs[0] }\nfn head(xs: [f64; 3]) -> f64 { xs[0] }";
    assert_clean(&source(items, "let a = pair(TAPS[1..3]); let b = pair(TAPS[LOW..=LOW + 1]); let c = all(TAPS[..]); let d = tail(TAPS[1..]); let e = head(TAPS[..3]);"));
    assert_refused(&source(items, "let a = pair(TAPS[1..4]);"), "P0002", "[f64; 3]");
}

#[test]
fn a_range_bound_that_is_not_const_is_e0188() {
    // §1.5.9 (VS-06 AC 2): a bound that is a local is no const expression, and the diagnostic names it.
    let items = "const TAPS: [f64; 4] = [0.25, 0.5, 0.5, 0.25];";
    assert_refused(&source(items, "let n = 2u8; let a = TAPS[0..n];"), "E0188", "the bound `n` of a range index");
}

#[test]
fn a_range_index_out_of_the_array_is_refused() {
    // §1.5.25 (VS-06 AC 3): a range that ends beyond the length, or starts after its end, is `E0104`.
    let items = "const TAPS: [f64; 4] = [0.25, 0.5, 0.5, 0.25];";
    assert_refused(&source(items, "let a = TAPS[2..5];"), "E0104", "outside an array of 4 elements");
    assert_refused(&source(items, "let a = TAPS[3..1];"), "E0104", "outside an array of 4 elements");
    assert_clean(&source(items, "let a = TAPS[4..];"));
}

#[test]
fn an_array_rest_pattern_binds_an_array() {
    // §1.6.7 (VS-06 AC 4): `name @ ..` binds `[T; N - k]`; more sub-patterns than elements is `E0180`.
    let items = "fn sum2(xs: [f64; 2]) -> f64 { xs[0] + xs[1] }";
    assert_clean(&source(items, "let a = [1.0, 2.0, 3.0, 4.0]; let [first, rest @ ..] = a; let [x, y, z] = rest; let [.., last] = a; let r = sum2([first, last]);"));
    assert_clean(&source(items, "let a = [1.0, 2.0, 3.0, 4.0]; let [head, mid @ .., tail] = a; let s = sum2(mid);"));
    assert_refused(&source("", "let a = [1.0, 2.0]; let [p, q, r, ..] = a;"), "E0180", "a pattern of at most 2 elements");
}

#[test]
fn an_array_is_passed_as_itself() {
    // §1.2.4 (VS-06 AC 5): `fn f(xs: [f64; 3])` takes the array, with no `&`; `[x; N]` takes an `x` of any type.
    let items = "fn first(xs: [f64; 3]) -> f64 { xs[0] }\n#[derive(PartialEq)] enum Mode { Off, On }";
    assert_clean(&source(items, "let a = [1.0, 2.0, 3.0]; let b = first(a); let c = first([0.5; 3]); let m = [(1.0, true); 2];"));
    assert_refused(&source(items, "let a = [1.0, 2.0]; let b = first(a);"), "P0002", "[f64; 3]");
}

#[test]
fn clone_and_copy_are_e0161_with_a_help() {
    // VS-05 AC 1 (§1.4.6, H.8 rule 1): `Clone` and `Copy` are no names of the language; in a derive or a
    // bound each is `E0161` at the name, and the help says every value is copied.
    for src in [
        "#[derive(Clone)]\nstruct S { a: f64 }\nmodule Top() { }",
        "#[derive(Copy, PartialEq)]\nenum E { A, B }\nmodule Top() { }",
        "fn f<T: Clone>(x: T) -> T { x }\nmodule Top() { }",
        "fn f<T: Copy>(x: T) -> T { x }\nmodule Top() { }",
    ] {
        let design: Design = support::build(src);
        let found: Vec<_> = design.diagnostics().iter().filter(|d| d.diagnostic.code == "E0161").collect();
        assert_eq!(found.len(), 1, "{src}: {:?}", support::codes(&design));
        let name = if src.contains("Clone") { "Clone" } else { "Copy" };
        assert!(found[0].diagnostic.message.contains(name), "{src}: {}", found[0].diagnostic.message);
        assert_eq!(found[0].diagnostic.help.as_deref(), Some("every value is copied; delete the name"), "{src}");
    }
}

#[test]
fn clone_as_a_method_is_e0161() {
    // VS-05 AC 3 (§1.4.6): a value is copied where it is used, so `clone()` names no method: `E0161` naming
    // the method and the type of the receiver.
    for (items, body, ty) in [
        ("", "let x = 1.0; let y = x.clone();", "f64"),
        ("struct S { a: f64 }", "let s = S { a: 1.0 }; let t = s.clone();", "S"),
        ("", "let a = [1.0, 2.0]; let b = a.clone();", "[f64; 2]"),
    ] {
        let src = source(items, body);
        let design: Design = support::build(&src);
        let found = Types::check(&design, "Top").expect_err(&src);
        let found: Vec<_> = found.iter().filter(|d| d.diagnostic.code == "E0161").collect();
        assert_eq!(found.len(), 1, "{src}");
        let message = &found[0].diagnostic.message;
        assert!(message.contains("`clone`") && message.contains(ty), "{message}");
        assert_eq!(found[0].diagnostic.help.as_deref(), Some("every value is copied; delete the name"), "{src}");
    }
}
