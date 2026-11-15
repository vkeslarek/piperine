//! Scope: the entry point of `piperine_lang::elab` (ELAB-01): `Elaborator::run` returns the
//! front-end and type-check diagnostics unchanged and no `Ir`, refuses an unknown or ported top
//! with one diagnostic naming it, and returns a checked `Ir` for a clean top. The M1 corpus and
//! its golden IR join in T9.

#[path = "support/elab.rs"]
mod support;

use piperine_lang::elab::Elaborator;
use support::Elab;

#[test]
fn front_end_diagnostics_are_returned_without_an_ir() {
    // The design itself holds a diagnostic (a malformed param): handed back as it is.
    let design = Elab::design("module Top() { x: f64 = ; }");
    assert!(!design.diagnostics().is_empty(), "the source must hold a diagnostic");
    let err = Elaborator::new(&design).run("Top").expect_err("no Ir for a design with diagnostics");
    assert_eq!(err, design.diagnostics().to_vec());

    // The design is clean, the type check is not: its diagnostic comes back, and no Ir.
    let design = Elab::design("module Top() { x: f64 = true; }");
    assert!(design.diagnostics().is_empty(), "{:?}", Elab::codes(design.diagnostics()));
    let err = Elaborator::new(&design).run("Top").expect_err("a bool default of an f64 param");
    assert_eq!(Elab::codes(&err), ["P0002"]);
}

#[test]
fn an_unknown_or_ported_top_is_one_diagnostic() {
    let design = Elab::design("module Top() { }");
    let err = Elaborator::new(&design).run("Missing").expect_err("no such module");
    assert_eq!(err.len(), 1);
    assert!(err[0].diagnostic.message.contains("Missing"), "{}", err[0].diagnostic.message);

    let src = "module Top(a: Electrical) { }";
    let design = Elab::design(src);
    assert!(design.diagnostics().is_empty(), "{:?}", Elab::codes(design.diagnostics()));
    let err = Elaborator::new(&design).run("Top").expect_err("a top with a port");
    assert_eq!(err.len(), 1);
    assert_eq!(err[0].diagnostic.code, "P0001");
    assert!(err[0].diagnostic.message.contains("Top"), "{}", err[0].diagnostic.message);
    let span = err[0].diagnostic.span();
    assert_eq!(&src[span.start..span.end], "Top", "the diagnostic sits on the module's name");
}

#[test]
fn an_empty_top_is_one_body_and_one_instance() {
    let ir = Elab::ir("module Top() { }", "Top");
    assert_eq!(ir.bodies().count(), 1);
    assert_eq!(ir.instances().count(), 1);
    assert_eq!(ir.top().path(), "Top");
}
