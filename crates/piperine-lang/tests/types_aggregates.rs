//! Aggregate types in `piperine_lang::types` (feature `composite-values`, CV-09 to CV-14; docs/spec
//! Part I §1.2 to §1.6, §1.3, §2): structs, tuples, arrays, slices and borrows as value types,
//! struct and tuple expressions, payload enums with `#[repr]` and discriminants, indexing and
//! slicing, and the codes of their errors. Each test states the rule it proves; every source is a
//! design of `Top` over an empty standard library.

#[path = "support/pom.rs"]
mod support;

use piperine_lang::pom::{DeclId, DeclKind, Design, Reflect};
use piperine_lang::types::{Ty, Types};

/// A design of `Top` with `items` before it and `body` as its analog block.
fn source(items: &str, body: &str) -> String {
    format!("{items}\nmodule Top() {{ }}\nanalog for Top {{ {body} }}")
}

/// The design of `src`, which must parse and resolve.
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

/// The declaration of kind `kind` named `name`.
fn declaration(design: &Design, name: &str, kind: fn(&DeclKind) -> bool) -> DeclId {
    design.decls().into_iter().find(|d| d.name() == Some(name) && kind(&d.kind())).unwrap_or_else(|| panic!("no declaration `{name}`")).id()
}

/// The type of the local, parameter or binding `name` in the checked source.
fn local_type(src: &str, name: &str) -> Ty {
    let design = design_of(src);
    let types = Types::check(&design, "Top").unwrap_or_else(|d| panic!("{:?}\n{src}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>()));
    let decl = declaration(&design, name, |k| matches!(k, DeclKind::Binding(_) | DeclKind::FnParam(_)));
    types.local(decl).cloned().unwrap_or_else(|| panic!("`{name}` has no type\n{src}"))
}

/// The struct or enum declaration named `name`.
fn type_decl(src: &str, name: &str) -> DeclId {
    declaration(&design_of(src), name, |k| matches!(k, DeclKind::Struct(_) | DeclKind::Enum(_)))
}

#[test]
fn every_struct_form_is_a_value_type() {
    // §1.3.1: a struct has named fields, positional fields or none, and each declaration is a type
    // of its own; §1.2.4 and §1.2.5 add tuples and arrays. A function takes each as a value.
    let items = "struct Named { x: f64, y: u8 }\nstruct Pair(f64, bool);\nstruct Marker;\n\
                 fn f(a: Named, b: Pair, c: Marker, d: (f64, u8), e: [f64; 3], i: (u8,), j: [[bool; 2]; 4]) -> f64 { 1.0 }";
    let src = source(items, "let h = f;");
    assert_eq!(local_type(&src, "a"), Ty::Struct(type_decl(&src, "Named")));
    assert_eq!(local_type(&src, "b"), Ty::Struct(type_decl(&src, "Pair")));
    assert_eq!(local_type(&src, "c"), Ty::Struct(type_decl(&src, "Marker")));
    assert_eq!(local_type(&src, "d"), Ty::Tuple(vec![Ty::F64, Ty::UInt(8)]));
    assert_eq!(local_type(&src, "e"), Ty::Array(Box::new(Ty::F64), 3), "§1.2.5: the length is part of the type");
    assert_eq!(local_type(&src, "i"), Ty::Tuple(vec![Ty::UInt(8)]), "a tuple of one field is written `(T,)`");
    assert_eq!(local_type(&src, "j"), Ty::Array(Box::new(Ty::Array(Box::new(Ty::Bool), 2)), 4));
    // §1.3.1: two structs with the same fields are different types, and a length folds from a `const`
    let src = source("struct A { x: f64 }\nstruct B { x: f64 }\nconst N: usize = 2 + 1;\nfn f(a: A, v: [u8; N]) {}", "let h = f;");
    assert_ne!(local_type(&src, "a"), Ty::Struct(type_decl(&src, "B")));
    assert_eq!(local_type(&src, "v"), Ty::Array(Box::new(Ty::UInt(8)), 3));
    // §1.4.8: tuples and arrays are equal by their parts, structs by their declaration
    assert_clean(&source("fn same(a: (f64, [u8; 2])) -> (f64, [u8; 2]) { a }", "let h = same;"));
    // a generic struct stays outside the subset (`generics`)
    assert_refused(&source("struct G<T> { t: f64 }\nfn f(g: G<f64>) {}", "let h = f;"), "P0001", "generic struct `G`");
    // §1.2.5: the length of an array type is a constant
    assert_refused(&source("const F: f64 = 1.0;\nfn f(v: [u8; F]) {}", "let h = f;"), "E0151", "constant");
}

#[test]
fn a_self_containing_type_is_e0186() {
    // §1.3.8: a type contains no copy of itself, directly or through structs, tuples, arrays and Option.
    for items in [
        "struct A { a: A }\nfn f(x: A) {}",
        "struct B { c: C }\nstruct C { b: B }\nfn f(x: B) {}",
        "struct D { d: Option<D> }\nfn f(x: D) {}",
        "struct E { e: [E; 2] }\nfn f(x: E) {}",
        "struct F { e: (u8, F) }\nfn f(x: F) {}",
    ] {
        let found = outcome(&source(items, "let h = f;"));
        assert!(found.iter().any(|(code, message)| code == "E0186" && message.contains("contains itself")), "{items}: {found:?}");
    }
    // §1.3.2: the field names of a struct are distinct
    assert_refused(&source("struct H { x: f64, x: u8 }\nfn f(a: H) {}", "let h = f;"), "E0186", "declared twice");
}

#[test]
fn struct_expressions_and_field_reads_type_check() {
    // §1.3.3: named fields in any order, shorthand, a tuple struct built by a call, a unit struct by
    // its name; §1.3.4: `s.x`, `t.0` and nested reads .
    let items = "struct P { x: f64, y: u8 }\nstruct Pair(f64, bool);\nstruct Marker;\nstruct Line { from: P, to: P }";
    let body = "let y: u8 = 3; \
                let p = P { y, x: 1.0 }; \
                let q = Pair(2.0, true); \
                let m = Marker; \
                let l = Line { from: P { x: 0.0, y: 1 }, to: p }; \
                let a: f64 = p.x; let b: u8 = l.to.y; let c: bool = q.1; let d: f64 = q.0;";
    let src = source(items, body);
    assert_eq!(local_type(&src, "p"), Ty::Struct(type_decl(&src, "P")));
    assert_eq!(local_type(&src, "q"), Ty::Struct(type_decl(&src, "Pair")));
    assert_eq!(local_type(&src, "m"), Ty::Struct(type_decl(&src, "Marker")));
    assert_eq!(local_type(&src, "a"), Ty::F64);
    assert_eq!(local_type(&src, "b"), Ty::UInt(8), "nested reads give the innermost field");
    // a tuple field of a tuple value
    assert_eq!(local_type(&source("", "let t = (1.0, (2u8, true)); let u = t.1.0; let v = t.1.1;"), "u"), Ty::UInt(8));
}

#[test]
fn a_bad_struct_expression_is_e0186_naming_the_field() {
    // §1.3.3: every field exactly once; §1.3.4: a field access the type does not have.
    let items = "struct P { x: f64, y: f64 }";
    assert_refused(&source(items, "let p = P { x: 1.0 };"), "E0186", "`y`");
    assert_refused(&source(items, "let p = P { x: 1.0, y: 2.0, x: 3.0 };"), "E0186", "`x` is named twice");
    // an unknown field of a struct literal is found where names resolve (the pom classifier)
    assert_eq!(support::codes(&support::build(&source(items, "let p = P { x: 1.0, y: 2.0, z: 3.0 };"))), ["E0186"]);
    assert_refused(&source(items, "let p = P { x: 1.0, y: 2.0 }; let z = p.z;"), "E0186", "no field `z`");
    assert_refused(&source("", "let t = (1.0, 2.0); let z = t.2;"), "E0186", "no field `2`");
    // a field is checked against its type
    assert_refused(&source(items, "let p = P { x: true, y: 2.0 };"), "E0180", "expected `f64`, found `bool`");
}

#[test]
fn struct_update_syntax_is_a_parse_error() {
    // §1.3.3 and B.3.7: there is no form that copies the remaining fields from another value (D3).
    let src = source("struct P { x: f64, y: f64 }", "let a = P { x: 1.0, y: 2.0 }; let b = P { x: 3.0, ..a };");
    let design = support::build(&src);
    assert!(!design.diagnostics().is_empty(), "`..base` must not parse");
}

#[test]
fn tuples_and_tuple_patterns_type_check() {
    // §1.2.4 and §1.6.5: a tuple is built, returned, passed and nested anywhere a value is, and a
    // tuple pattern binds each field in a `let`, a `match`, an `if let` and a closure parameter.
    let items = "fn swap(p: (f64, u8)) -> (u8, f64) { let (a, b) = p; (b, a) }\n\
                 fn first(p: ((u8, bool), f64)) -> u8 { let ((n, _), _) = p; n }";
    let body = "let t = swap((1.0, 2)); let n: u8 = t.0; \
                let (x, y) = (1.0, true); \
                let u = match t { (0, f) => f, (_, f) => f + 1.0 }; \
                let w = first(((1, false), 0.5)); \
                let one = (3u8,); let z = one.0;";
    let src = source(items, body);
    assert_eq!(local_type(&src, "t"), Ty::Tuple(vec![Ty::UInt(8), Ty::F64]));
    assert_eq!(local_type(&src, "x"), Ty::F64);
    assert_eq!(local_type(&src, "y"), Ty::Bool);
    assert_eq!(local_type(&src, "u"), Ty::F64);
    assert_eq!(local_type(&src, "w"), Ty::UInt(8));
    assert_eq!(local_type(&src, "z"), Ty::UInt(8));
    // §1.6.5: `..` stands for the fields between; the arity is the type's
    assert_clean(&source("fn f(t: (u8, u8, u8)) -> u8 { let (a, .., c) = t; a + c }", "let h = f;"));
    assert_refused(&source("", "let (a, b) = (1.0, 2.0, 3.0);"), "E0180", "a pattern of 3 fields");
    // §1.6.11: a refutable pattern in a `let` is E0184; §1.6.10: a name is bound once
    assert_refused(&source("", "let (a, 1) = (true, 2u8);"), "E0184", "irrefutable");
    assert_refused(&source("", "let (a, a) = (1.0, 2.0);"), "E0184", "bound twice");
    // an `if let` over a tuple and an exhaustive `match` over a tuple of `bool`s
    assert_clean(&source("", "let t = (true, 2u8); if let (true, n) = t { let m: u8 = n; } let r = match (true, false) { (true, _) => 1, (false, true) => 2, (false, false) => 3 };"));
    assert_refused(&source("", "let r = match (true, false) { (true, _) => 1, (false, true) => 2 };"), "E0184", "(false, false)");
}

#[test]
fn payload_enums_build_and_match() {
    // §1.3.5 and §1.3.6: a variant has no field, positional fields or named fields, and is built by its
    // name, a call or a struct expression; the arms bind the fields and join the exhaustiveness of §1.8.6.
    let items = "#[derive(Repr, PartialEq)]\nenum Tx { Idle, Data(u8), Frame { n: u3, last: bool } }\n\
                 fn size(t: Tx) -> u8 { match t { Tx::Idle => 0, Tx::Data(b) => b, Tx::Frame { n, last: true } => n as u8, Tx::Frame { n, .. } => 1 } }";
    let src = source(items, "let va = Tx::Idle; let vb = Tx::Data(3); let vc = Tx::Frame { n: 2, last: false }; let s = size(vc);");
    for name in ["va", "vb", "vc"] {
        assert_eq!(local_type(&src, name), Ty::Enum(type_decl(&src, "Tx")), "{name}");
    }
    assert_eq!(local_type(&src, "s"), Ty::UInt(8));
    // §1.8.6: a variant left out is named, with its fields as wildcards
    let missing = "#[derive(Repr)]\nenum Tx { Idle, Data(u8) }\nfn f(t: Tx) -> u8 { match t { Tx::Idle => 0 } }";
    assert_refused(&source(missing, "let h = f;"), "E0184", "Tx::Data(0..=255)");
    // `Option` of an aggregate behaves as an enum with a payload
    assert_clean(&source("struct P { x: f64 }\nfn f(o: Option<P>) -> f64 { match o { Some(P { x }) => x, None => 0.0 } }", "let h = f;"));
    // a fieldless enum converts to an integer (its discriminant), a payload enum does not
    assert_clean(&source("#[derive(Repr)]\nenum M { A, B }\nfn f(m: M) -> u8 { m as u8 }", "let h = f;"));
    // a variant with fields used as a bare name needs its fields
    assert_refused(&source("#[derive(Repr)]\nenum Tx { Idle, Data(u8) }", "let t = Tx::Data;"), "P0001", "has fields");
    // a field the variant lacks, and a payload that does not contain the enum (§1.3.8)
    assert_eq!(support::codes(&support::build(&source("#[derive(Repr)]\nenum Tx { Idle, Frame { n: u3 } }", "let t = Tx::Frame { m: 1 };"))), ["E0186"]);
    assert_refused(&source("#[derive(Repr)]\nenum L { Nil, Cons(u8, L) }\nfn f(l: L) {}", "let h = f;"), "E0186", "contains itself");
}

#[test]
fn a_discriminant_that_does_not_fit_or_repeats_is_e0111() {
    // §1.3.7 and §2 rule 5: explicit discriminants count from 0 in order unless written, must fit the
    // tag's levels (`N` of `#[repr(uN)]`, else `ceil(log2(V))`) and are never repeated.
    let clean = "#[derive(Repr)]\n#[repr(u8)]\nenum Op { Add = 0, Sub = 1, Mul = 0x80 }\nfn f(o: Op) -> u8 { o as u8 }";
    assert_clean(&source(clean, "let h = f;"));
    let narrow = "#[derive(Repr)]\n#[repr(u2)]\nenum Op { A = 0, B = 4 }\nfn f(o: Op) {}";
    assert_refused(&source(narrow, "let h = f;"), "E0111", "does not fit");
    let repeated = "#[derive(Repr)]\n#[repr(u8)]\nenum Op { A = 1, B = 1 }\nfn f(o: Op) {}";
    assert_refused(&source(repeated, "let h = f;"), "E0111", "`B` has the discriminant 1");
    // an implicit discriminant is the one before plus one: `B = 0` then `C` is 1 again
    let implicit = "#[derive(Repr)]\n#[repr(u8)]\nenum Op { A = 1, B = 0, C }\nfn f(o: Op) {}";
    assert_refused(&source(implicit, "let h = f;"), "E0111", "`C` has the discriminant 1");
    // without `#[repr]` the tag has `ceil(log2(V))` levels: two variants, one level, so 2 does not fit
    let tag = "#[derive(Repr)]\nenum Op { A, B = 2 }\nfn f(o: Op) {}";
    assert_refused(&source(tag, "let h = f;"), "E0111", "1 levels");
    // §1.3.7: a discriminant only where no variant has a field
    let fields = "#[derive(Repr)]\nenum Op { A = 0, B(u8) }\nfn f(o: Op) {}";
    assert_refused(&source(fields, "let h = f;"), "E0186", "has a discriminant");
    // §1.3.2: variant names are distinct
    let twice = "#[derive(Repr)]\nenum Op { A, A }\nfn f(o: Op) {}";
    assert_refused(&source(twice, "let h = f;"), "E0186", "declared twice");
    // a one-variant enum has a tag of no levels, its only discriminant is 0
    assert_clean(&source("#[derive(Repr)]\nenum One { Only }\nfn f(o: One) {}", "let h = f;"));
}

#[test]
fn arrays_and_indexing_type_check() {
    // §1.2.5: `[a, b, c]` has one element type, `[x; N]` repeats a value, `e[i]` takes any integer
    // type, an element is assigned, arrays nest; §1.6.7: array patterns. A range index is an array of
    // its own (`types_value_semantics.rs`).
    let items = "fn pick(a: [[u8; 2]; 2]) -> u8 { a[1][0] }";
    let body = "let arr = [1.0, 2.0, 3.0]; let r = [0u8; 4]; let i: u16 = 2; let x = arr[i]; let y: f64 = arr[0]; \
                let m = [[1u8, 2], [3, 4]]; m[1][0] = 9; let p = pick(m);";
    let src = source(items, body);
    assert_eq!(local_type(&src, "arr"), Ty::Array(Box::new(Ty::F64), 3));
    assert_eq!(local_type(&src, "r"), Ty::Array(Box::new(Ty::UInt(8)), 4));
    assert_eq!(local_type(&src, "x"), Ty::F64);
    assert_eq!(local_type(&src, "m"), Ty::Array(Box::new(Ty::Array(Box::new(Ty::UInt(8)), 2)), 2));
    assert_eq!(local_type(&src, "p"), Ty::UInt(8));
    assert_refused(&source("", "let a = [1.0, true];"), "E0180", "expected `f64`, found `bool`");
    assert_refused(&source("", "let a = [1.0, 2.0]; let x = a[1.5];"), "E0180", "an integer type");
    assert_refused(&source("", "let a = 3.0; let x = a[0];"), "E0180", "an array");
    // an empty array has no element type of its own (§1.4.4), unless the position gives one
    assert_refused(&source("", "let a = [];"), "E0181", "element type");
    assert_clean(&source("", "let a: [u8; 0] = [];"));
    assert_clean(&source("fn f(a: [u8; 4]) -> u8 { let [x, .., z] = a; let [h, rest @ ..] = a; x + z + h + rest[0] }", "let h = f;"));
    assert_refused(&source("fn f(a: [u8; 2]) { let [x, y, z] = a; }", "let h = f;"), "E0180", "a pattern of 2 elements");
    assert_clean(&source("fn f(a: [bool; 2]) -> u8 { match a { [true, _] => 1, [false, true] => 2, [false, false] => 3 } }", "let h = f;"));
}

#[test]
fn a_non_constant_array_length_is_refused_naming_it() {
    // §1.2.5: `N` of `[x; N]` is a const expression; the specification names no code for another
    // (D6), so the plain mismatch names the length.
    assert_refused(&source("", "let n: u8 = 3; let a = [0u8; n];"), "E0180", "`n`");
    assert_refused(&source("const F: f64 = 2.0;", "let a = [0u8; F];"), "E0180", "`F`");
    // a `const` folds, and so does arithmetic of constants
    assert_clean(&source("const N: usize = 2;", "let a = [0u8; N * 2 + 1];"));
}

