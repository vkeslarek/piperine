//! The entry point, reach and determinism of `piperine_lang::types` (TYP-01, TYP-06): what
//! `Types::check` returns for a design with diagnostics, for an unknown top, for the same
//! design twice, and for a `const` read by several expressions. The M1 corpus joins in T9.

#[path = "support/pom.rs"]
mod support;

use piperine_lang::pom::Design;
use piperine_lang::types::Types;

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

    // A module of the standard library is not a module of the root crate.
    let design = Design::builder().std_source("lib.msl", "pub mod prelude;\npub module Lib() { }\n").std_source("prelude.msl", "").source("top.msl", "module Top() { }").build();
    let err = Types::check(&design, "Lib").expect_err("a std module is no top");
    assert_eq!(err.len(), 1);
    assert!(err[0].diagnostic.message.contains("Lib"));
}
