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


#[test]
fn derived_traits_give_comparison_and_default() {
    // CV-12 AC 1, H.8 rules 2 to 4: a derived `PartialEq` gives `==` and `!=`, `PartialOrd` gives the
    // four orderings, `Default` gives `T::default()`; the built-in types implement them by rule 7
    // (tuples, arrays and `Option` per part).
    let items = "#[derive(PartialEq, Eq, PartialOrd, Ord, Default)]\nstruct Key { a: u8, b: bool }\n\
                 #[derive(PartialEq, PartialOrd, Default)]\nstruct Pt { x: f64, y: f64 }\n\
                 #[derive(Repr, PartialEq, Eq, PartialOrd, Ord)]\nenum Mode { Off, On(u8) }";
    let body = "let k = Key { a: 1, b: true }; let same = k == k; let other = k != Key::default(); let before = k < Key { a: 2, b: false }; \
                let p = Pt::default(); let near = p <= Pt { x: 0.0, y: 1.0 }; \
                let t = (1u8, 2.0) == (1u8, 2.0); let arr = [1u8, 2] < [1u8, 3]; let o = Some(1u8) == None; \
                let m = Mode::On(1) > Mode::Off;";
    for name in ["same", "other", "before", "near", "t", "arr", "o", "m"] {
        assert_eq!(local_type(&source(items, body), name), Ty::Bool, "{name}");
    }
    let src = source(items, body);
    assert_eq!(local_type(&src, "p"), Ty::Struct(type_decl(&src, "Pt")), "`T::default()` has the type `T`");
    // H.8 rule 7: the defaults of the built-in types
    let defaults = "let a: u8 = u8::default(); let b: bool = bool::default(); let c: f64 = f64::default();";
    assert_clean(&source("", defaults));
}

#[test]
fn comparing_without_the_trait_is_refused_naming_it() {
    // CV-12 AC 2, D6: `==` on a type without `PartialEq` and `<` on one without `PartialOrd` are
    // refused naming the trait; a type has a trait only when it derives it (H.8 rule 7: a tuple or
    // an array, when every part has it).
    let plain = "struct P { x: f64 }";
    assert_refused(&source(plain, "let a = P { x: 1.0 } == P { x: 1.0 };"), "E0180", "a type that derives `PartialEq`");
    assert_refused(&source(plain, "let a = P { x: 1.0 } != P { x: 1.0 };"), "E0180", "a type that derives `PartialEq`");
    let eq_only = "#[derive(PartialEq)]\nstruct P { x: f64 }";
    assert_refused(&source(eq_only, "let a = P { x: 1.0 } < P { x: 1.0 };"), "E0180", "a type that derives `PartialOrd`");
    assert_refused(&source(plain, "let a = (1.0, P { x: 1.0 }) == (1.0, P { x: 1.0 });"), "E0180", "a type that derives `PartialEq`");
    assert_refused(&source(plain, "let a = [P { x: 1.0 }] == [P { x: 1.0 }];"), "E0180", "a type that derives `PartialEq`");
    // H.8 rule 6: a derive needs the trait on every field; a float has no `Eq` (H.8 resolution)
    assert_refused(&source("#[derive(PartialEq, Eq)]\nstruct P { x: f64 }", "let h = P { x: 1.0 };"), "E0180", "`Eq`");
    assert_refused(&source("#[derive(PartialEq)]\nstruct Q { x: u8 }\n#[derive(PartialEq)]\nstruct P { q: Q, n: f64 }\n#[derive(Default)]\nstruct D { q: Q }", "let h = D::default();"), "E0180", "`Default`");
    // H.8 rule 3: `Ord` requires `Eq` and `PartialOrd`
    assert_refused(&source("#[derive(Ord)]\nstruct P { x: u8 }", "let h = P { x: 1 };"), "E0180", "`#[derive(Ord)]` on struct `P` needs `#[derive(Eq)]`");
}

#[test]
fn debug_and_hash_derives_are_accepted() {
    // CV-12 AC 3, H.8 rule 5: `Debug` and `Hash` add nothing a model calls; a float has no `Hash`
    // (H.8 resolution) and a derive on a field without the trait is refused.
    let items = "#[derive(Debug, Hash, PartialEq, Eq)]\nstruct Id { n: u8, up: bool }\n#[derive(Repr, Debug, Hash)]\nenum Edge { Rise, Fall }";
    assert_clean(&source(items, "let a = Id { n: 1, up: true }; let e = Edge::Rise;"));
    assert_clean(&source("#[derive(Debug)]\nstruct Pt { x: f64 }", "let p = Pt { x: 1.0 };"));
    assert_refused(&source("#[derive(Hash)]\nstruct Pt { x: f64 }", "let p = Pt { x: 1.0 };"), "E0180", "`Hash`");
    // an unknown derive name is E0161 at the name, and `Clone` and `Copy` are unknown names
    for name in ["Clone", "Copy", "Frobnicate"] {
        let design = support::build(&source(&format!("#[derive({name})]\nstruct Pt {{ x: f64 }}"), "let p = Pt { x: 1.0 };"));
        assert_eq!(support::codes(&design), ["E0161"], "{name}");
    }
}

#[test]
fn an_enum_deriving_default_is_refused() {
    // CV-12 AC 1, D4, H.8 rule 4: "An enum does not derive `Default`", refused naming `Default`.
    let src = source("#[derive(Repr, Default)]\nenum Mode { Off, On }", "let m = Mode::Off;");
    assert_refused(&src, "E0180", "`Default`");
}

#[test]
fn the_rows_of_h5_to_h7_type_check() {
    // H.5 to H.7, every row by value (§1.4.6): an array's rows read a copy of it, `iter` yields `T`,
    // `first`/`last`/`get` give `Option<T>`, `contains(x: T)`, a closure takes `T`; `for` reads an
    // array's elements, a range and an adaptor chain (§1.8.9); `rev` only where H.7 rule 3 allows.
    let body = "let a = [1u8, 2, 3]; \
                let n = a.len(); let e = a.is_empty(); let has = a.contains(2); let f = a.first(); let l = a.last(); let g = a.get(1); \
                let it = a.iter(); \
                let doubled = a.iter().map(|x| x * 2); let big = a.iter().filter(|x| x > 1); \
                let total = a.iter().fold(0, |acc, x| acc + x); let s = a.iter().sum(); let c = a.iter().count(); \
                let z = a.iter().zip(a.iter()); let en = a.iter().enumerate(); let r = a.iter().rev(); \
                let any = a.iter().any(|x| x == 3); let all = a.iter().all(|x| x > 0); \
                let inside = (0u8..4).contains(2); let inclusive = (0.0..=1.0).contains(0.5); let empty = (3u8..3).is_empty(); \
                let squares = (0u8..4).map(|k| k * k).rev(); let sum_range = (1u8..=3).sum(); \
                for x in a { let y: u8 = x; } for x in a.iter().rev() { let y: u8 = x; } for (i, x) in a.iter().enumerate() { let k: usize = i; let y: u8 = x; } \
                for x in (0u8..4).map(|k| k + 1) { let y: u8 = x; } for (p, q) in a.iter().zip(a.iter()) { let s: u8 = p + q; } for x in 0u8..3 { let y: u8 = x; }";
    let src = source("", body);
    let iterator = |element: Ty, reversible: bool| Ty::Iterator { element: Box::new(element), reversible };
    let expected = [
        ("n", Ty::UInt(64)), ("e", Ty::Bool), ("has", Ty::Bool), ("f", Ty::Option(Box::new(Ty::UInt(8)))), ("l", Ty::Option(Box::new(Ty::UInt(8)))), ("g", Ty::Option(Box::new(Ty::UInt(8)))),
        ("it", iterator(Ty::UInt(8), true)), ("doubled", iterator(Ty::UInt(8), true)), ("big", iterator(Ty::UInt(8), false)),
        ("total", Ty::UInt(8)), ("s", Ty::UInt(8)), ("c", Ty::UInt(64)),
        ("z", iterator(Ty::Tuple(vec![Ty::UInt(8), Ty::UInt(8)]), true)), ("en", iterator(Ty::Tuple(vec![Ty::UInt(64), Ty::UInt(8)]), true)), ("r", iterator(Ty::UInt(8), true)),
        ("any", Ty::Bool), ("all", Ty::Bool), ("inside", Ty::Bool), ("inclusive", Ty::Bool), ("empty", Ty::Bool),
        ("squares", iterator(Ty::UInt(8), true)), ("sum_range", Ty::UInt(8)),
    ];
    for (name, ty) in expected {
        assert_eq!(local_type(&src, name), ty, "{name}");
    }
    // H.7 rule 3: `rev` is refused after `filter`; `sum` needs an integer or a float element
    assert_refused(&source("", "let r = [1u8, 2].iter().filter(|x| x > 1).rev();"), "E0161", "rev");
    assert_refused(&source("", "let s = [true, false].iter().sum();"), "E0180", "an integer or a float element type");
    // H.6 rule 3: a float range or a range without a bound is no iterator
    assert_refused(&source("", "let m = (0.0..1.0).count();"), "E0161", "count");
    // a closure takes the element by value: a wrong parameter use is the plain mismatch
    assert_refused(&source("", "let m = [1u8, 2].iter().map(|x| x && true);"), "E0180", "bool");
    // `contains` needs `PartialEq` of the element (H.8)
    assert_refused(&source("struct P { x: f64 }", "let a = [P { x: 1.0 }]; let h = a.contains(P { x: 1.0 });"), "E0180", "`PartialEq`");
}

#[test]
fn a_method_outside_appendix_h_is_e0161() {
    // H rule 1: the catalogue of arrays, ranges and iterators is closed; `clone` is no method (§1.4.6).
    for (receiver, method) in [("[1u8, 2]", "push(3)"), ("[1u8, 2]", "clone()"), ("[1u8, 2]", "sort()"), ("(0u8..3)", "len()"), ("[1u8, 2].iter()", "next()"), ("[1u8, 2].iter()", "collect()")] {
        let found = outcome(&source("", &format!("let v = {receiver}.{method};")));
        assert!(found.iter().any(|(code, message)| code == "E0161" && message.contains(method.split('(').next().unwrap_or(""))), "{receiver}.{method}: {found:?}");
    }
    // the element type is named in the message
    assert_refused(&source("", "let v = [1u8, 2].push(3);"), "E0161", "[u8; 2]");
}

/// The design of `src` over the embedded standard library, which must parse and resolve: a source
/// that names `Bit`.
fn standard_design_of(src: &str) -> Design {
    let design = Design::builder().source("top.msl", src).build();
    assert!(design.diagnostics().is_empty(), "the source must parse and resolve: {:?}\n{src}", design.diagnostics().iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
    design
}

/// The level counts the `T::WIDTH` paths of the checked source read, in source order.
fn widths(src: &str) -> Vec<u32> {
    let design = design_of(src);
    let types = Types::check(&design, "Top").unwrap_or_else(|d| panic!("{:?}\n{src}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>()));
    (0..design.expr_count()).filter_map(|i| types.width(piperine_lang::pom::ExprId(i as u32))).collect()
}

#[test]
fn composite_carriers_type_check() {
    // §2 table and rules 2, 5, 6: a tuple, an array, an `Option`, a derived struct and a derived
    // payload enum have a `Repr`, so each is a carried type; `T::WIDTH` is the table's count.
    let items = "#[derive(Repr, PartialEq)]\nstruct Pair { a: u3, b: bool }\n\
                 #[derive(Repr, PartialEq)]\nenum Tx { Idle, Start, Data { n: u3 }, Stop }\n\
                 #[derive(Repr)]\n#[repr(u4)]\nenum Wide { A, B(u2, bool) }";
    for carried in ["(u8, bool)", "[u4; 3]", "Option<u8>", "Pair", "Tx", "Option<Pair>", "[(u2, bool); 2]", "Wide"] {
        let src = format!("{items}\nmodule Top() {{ signal s: Bit<{carried}>; }}");
        assert_clean_module(&src, carried);
    }
    // WIDTH: tuple/array/Option sums by the table; `Tx` is a 2-level tag and a 3-level payload
    // (the Resolution under §2 rule 6); `Wide` is the `#[repr(u4)]` tag and its widest payload.
    let body = "let a = Pair::WIDTH; let b = Tx::WIDTH; let c = Wide::WIDTH;";
    assert_eq!(widths(&source(items, body)), [4, 5, 7]);
}

/// The source, a module, checks clean; `what` names the case.
fn assert_clean_module(src: &str, what: &str) {
    let design = standard_design_of(src);
    if let Err(found) = Types::check(&design, "Top") {
        panic!("{what}: {:?}", found.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
    }
}

#[test]
fn deriving_repr_with_a_field_without_repr_is_e0110() {
    // §2 rule 3: `#[derive(Repr)]` needs every field to have a `Repr`, naming the field: a `str`, a
    // struct that does not derive it, a tuple that holds one, a variant field of an enum.
    let plain = "struct Plain { x: u8 }";
    for (items, field) in [
        ("#[derive(Repr)]\nstruct S { name: str }", "name"),
        (&format!("{plain}\n#[derive(Repr)]\nstruct S {{ inner: Plain }}"), "inner"),
        (&format!("{plain}\n#[derive(Repr)]\nstruct S {{ pair: (u8, Plain) }}"), "pair"),
        ("#[derive(Repr)]\nenum E { A, B { text: str } }", "text"),
    ] {
        let used = source(&format!("{items}\nfn f(v: {}) {{}}", if items.contains("enum E") { "E" } else { "S" }), "let h = f;");
        assert_refused(&used, "E0110", &format!("`{field}`"));
    }
}

#[test]
fn an_enum_without_repr_is_a_value_but_not_a_carried_type() {
    // CV-11 AC 1 (part I section 1.3 and 2: every enum is a value type): an enum with a payload and no `Repr` is built,
    // matched, passed and returned by value; a carrier of it is `E0112`.
    let items = "enum Shape { Circle(f64), Rect { w: f64, h: f64 } }\nfn area(s: Shape) -> f64 { match s { Shape::Circle(r) => r * r, Shape::Rect { w, h } => w * h } }";
    assert_clean(&source(items, "let a = area(Shape::Rect { w: 2.0, h: 3.0 }); let b = area(Shape::Circle(1.0));"));
    let src = format!("{items}\nmodule Top() {{ signal s: Bit<Shape>; }}");
    let found = Types::check(&standard_design_of(&src), "Top").err().unwrap_or_default();
    assert!(found.iter().any(|d| d.diagnostic.code == "E0112"), "{:?}", found.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
}

#[test]
fn a_carried_type_without_repr_is_e0112() {
    // §2 table: a type without a `Repr` is E0112 as a carried type, whatever holds it.
    let items = "struct Plain { x: u8 }\n#[derive(Repr)]\nstruct Good { x: u8 }";
    for carried in ["Plain", "(u8, str)", "[str; 2]", "Option<Plain>", "[Plain; 2]"] {
        let src = format!("{items}\nmodule Top() {{ signal s: Bit<{carried}>; }}");
        let design = standard_design_of(&src);
        let found = Types::check(&design, "Top").err().unwrap_or_default();
        assert!(found.iter().any(|d| d.diagnostic.code == "E0112"), "{carried}: {:?}", found.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
    }
}

#[test]
fn composite_params_and_record_fields_type_check() {
    // CV-18 AC 1 (§8.1.15, §8.1.16): a param of struct, tuple or array type, with or without a
    // default and a `where`, is bound at a placement; CV-19 AC 1 (§19 rule 11): an `emit!` field of a
    // composite type.
    let items = "#[derive(PartialEq)]\nstruct Gains { kp: f64, ki: f64 }\n\
                 module Child(o: Electrical) { \
                   gains: Gains = Gains { kp: 1.0, ki: 0.5 } where gains.kp > 0.0; \
                   taps: [f64; 3] = [0.1, 0.2, 0.3]; \
                   pair: (f64, bool); }";
    let top = "module Top() { signal n: Electrical; let c = Child(n) { gains: Gains { kp: 2.0, ki: 1.0 }, taps: [1.0, 2.0, 3.0], pair: (1.0, true) }; }";
    let design = standard_design_of(&format!("{items}\n{top}"));
    if let Err(found) = Types::check(&design, "Top") {
        panic!("{:?}", found.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
    }
    // a bound value of the wrong composite type is the plain mismatch
    let wrong = format!("{items}\nmodule Top() {{ signal n: Electrical; let c = Child(n) {{ taps: [1.0, 2.0], pair: (1.0, true) }}; }}");
    let found = Types::check(&standard_design_of(&wrong), "Top").err().unwrap_or_default();
    assert!(found.iter().any(|d| d.diagnostic.code == "E0180" && d.diagnostic.message.contains("[f64; 3]")), "{:?}", found.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
    // a field of a composite type is a value of a record
    let record = "#[derive(PartialEq)]\nstruct Pt { x: f64, tag: u8 }\nmodule Top(in clk: Bit) { }\ndigital for Top { on clk.rise() { let p = Pt { x: 1.0, tag: 2 }; emit!(\"trace\", at = p, pair = (1.0, true), taps = [1u8, 2]); } }";
    let design = standard_design_of(record);
    if let Err(found) = Types::check(&design, "Top") {
        panic!("{:?}", found.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
    }
}

#[test]
fn two_sites_giving_a_field_two_composite_types_is_e0605() {
    // CV-19 AC 2, §19 rule 12: one field of one stream has one type in a module.
    let src = "module Top(in clk: Bit) { }\ndigital for Top { on clk.rise() { emit!(\"trace\", v = (1.0, true)); emit!(\"trace\", v = [1.0, 2.0]); } }";
    let found = Types::check(&standard_design_of(src), "Top").err().unwrap_or_default();
    assert_eq!(found.iter().map(|d| d.diagnostic.code.to_string()).collect::<Vec<_>>(), ["E0605"], "{:?}", found.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
    assert!(found[0].diagnostic.message.contains("`v`"), "{}", found[0].diagnostic.message);
}

#[test]
fn phase_c_and_d_forms_stay_p0001_by_name() {
    // CV-34 AC 2: an array of carriers (phase C), a carrier of `f32` and of `u128` (phase D) are
    // refused by name, never silently typed.
    for (carried, form) in [("[Bit<u8>; 4]", "an array of `Bit<u8>`"), ("Bit<f32>", "f32"), ("Bit<u128>", "u128")] {
        let src = format!("module Top() {{ signal s: {carried}; }}");
        let found = Types::check(&standard_design_of(&src), "Top").err().unwrap_or_default();
        assert!(found.iter().any(|d| d.diagnostic.code == "P0001" && d.diagnostic.message.contains(form)), "{carried}: {:?}", found.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
    }
}
