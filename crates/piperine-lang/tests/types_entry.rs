//! The entry point, reach and determinism of `piperine_lang::types` (TYP-01, TYP-06): what
//! `Types::check` returns for a design with diagnostics, for an unknown top, for the same
//! design twice, and for a `const` read by several expressions. The M1 corpus joins in T9.

#[path = "support/pom.rs"]
mod support;

use piperine_lang::pom::{DeclKind, Design, Reflect};
use piperine_lang::types::{Ty, Types};

#[test]
fn a_design_with_diagnostics_gives_them_back() {
    let design = support::build("module Top() { x: f64 = ; }");
    assert!(!design.diagnostics().is_empty(), "the source must hold a diagnostic");
    let err = Types::check(&design, "Top").expect_err("no table for a design with diagnostics");
    assert_eq!(err, design.diagnostics().to_vec());
    assert_eq!(err.len(), support::codes(&design).len());
}

#[test]
fn an_unknown_top_is_one_diagnostic() {
    let design = support::build("module Top() { }");
    assert!(design.diagnostics().is_empty());
    let err = Types::check(&design, "Missing").expect_err("no such module");
    assert_eq!(err.len(), 1);
    assert!(err[0].diagnostic.message.contains("Missing"), "{}", err[0].diagnostic.message);
    assert_eq!(err[0].diagnostic.code, "P0003", "an unknown top has the compiler-owned code P0003");

    // A module of the standard library is not a module of the root crate.
    let design = Design::builder().std_source("lib.msl", "pub mod prelude;\npub module Lib() { }\n").std_source("prelude.msl", "").source("top.msl", "module Top() { }").build();
    let err = Types::check(&design, "Lib").expect_err("a std module is no top");
    assert_eq!(err.len(), 1);
    assert!(err[0].diagnostic.message.contains("Lib"));
    assert_eq!(err[0].diagnostic.code, "P0003");
}

const SHARED: &str = "const K: f64 = 2.0;\nmodule Top() { }\nanalog for Top { let a = K; let b = a; }";

#[test]
fn checking_twice_gives_equal_tables() {
    let design = support::build(SHARED);
    assert!(design.diagnostics().is_empty(), "{:?}", support::codes(&design));
    let first = Types::check(&design, "Top").expect("the source is in the subset");
    let second = Types::check(&design, "Top").expect("the source is in the subset");
    assert_eq!(first, second);
    assert!(!first.reached().is_empty());
}

#[test]
fn a_const_is_typed_once_by_its_declared_type() {
    let design = support::build(SHARED);
    let types = Types::check(&design, "Top").expect("the source is in the subset");
    let top = design.module("Top").unwrap().decl().id();
    let konst = design.find("K").expect("the const").id();
    // Reached in first-reach order: the top, then what its body reads, each once.
    assert_eq!(types.reached().iter().filter(|d| **d == konst).count(), 1);
    assert_eq!(types.reached()[0], top);
    // Its initializer is typed by the declared type.
    let DeclKind::Const(data) = &design.decl_data(konst).kind else { panic!("K is a const") };
    assert_eq!(types.ty(data.value.expect("a value")), Some(&Ty::F64));

    // However many readers it has, a bad initializer is reported once.
    let design = support::build("const K: f64 = true;\nmodule Top() { }\nanalog for Top { let a = K; let b = K; }");
    let err = Types::check(&design, "Top").expect_err("a bool initializer of an f64 const");
    assert_eq!(err.len(), 1, "{err:?}");
}
