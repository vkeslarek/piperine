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
