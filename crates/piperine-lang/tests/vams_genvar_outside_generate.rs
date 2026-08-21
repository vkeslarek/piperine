//! T17/LRMW2-13 (§3.5): a declared `genvar` registers in the POM
//! (`Module::genvars`), and using its name in an ordinary (non-generate)
//! parameter default — where a genvar has no runtime storage or value —
//! fails loud citing §3.5, instead of falling through to a generic
//! "undefined name" error. Before this fix `elab/lower/vams.rs` dropped
//! `ModuleItem::Genvar` entirely (`ModuleItem::Genvar(_) => {}`), so
//! `Module::genvars()` was always empty and nothing distinguished a
//! genvar-name misuse from any other undefined identifier.
//!
//! LRM-COVERS: 3.5

use piperine_lang::pom::{ElabError, ElabErrorKind};
use piperine_lang::{parse_and_elaborate, SourceMap};

const DECLARED_SRC: &str = "
module m ();
    genvar i, j;
endmodule
";

const MISUSE_SRC: &str = "
module m ();
    genvar i;
    parameter real p = i;
endmodule
";

/// A declared `genvar i, j;` is visible in the POM's `Module::genvars()`.
#[test]
fn declared_genvar_is_visible_in_the_pom() {
    let design = parse_and_elaborate(DECLARED_SRC, &SourceMap::dummy()).expect("elaborates");
    let m = design.module("m").expect("module m");
    assert_eq!(m.genvars(), &["i".to_string(), "j".to_string()]);
}

/// Referencing a declared genvar's name in an ordinary parameter default
/// — outside any generate construct — fails loud, typed as
/// `ElabErrorKind::Other` citing §3.5 and naming the genvar.
#[test]
fn genvar_used_outside_generate_fails_loud_by_typed_kind() {
    let report = parse_and_elaborate(MISUSE_SRC, &SourceMap::dummy())
        .expect_err("a genvar used outside generate must fail loud, not silently resolve");
    let err = report
        .downcast_ref::<ElabError>()
        .unwrap_or_else(|| panic!("elaboration failure is a typed ElabError, got: {report}"));
    match &err.kind {
        ElabErrorKind::Other(msg) => {
            assert!(msg.contains('i'), "error must name the genvar: {msg}");
            assert!(msg.contains("3.5"), "error must cite LRM §3.5: {msg}");
        }
        other => panic!("expected ElabErrorKind::Other citing §3.5, got {other:?}"),
    }
}
