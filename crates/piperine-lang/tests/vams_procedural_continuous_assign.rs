//! LRM-COVERS: 8.5.3.1, 8.5.3.2
//!
//! LRMW2-47 (`lrm-conformance-wave2`, T53): procedural continuous
//! assignment and force/release (§8.5.3.2, Annex A.6.4) had no grammar
//! production at all — `assign`/`deassign`/`force`/`release` used as a
//! *statement* (inside `initial`/`always`, as opposed to the already-
//! implemented module-level `assign` *item*, T10/§8.5.3.1) failed to
//! parse. This file covers the language/POM layer: all four parse into
//! their own dedicated `Stmt` variants, and — since §9.3 of IEEE
//! 1364-2005 gives all four no analog counterpart — each is rejected
//! inside an `analog` block citing Annex C.7, the same family
//! `<=`/`casex`/`casez` already belong to. The *simulated* proof (force
//! overriding a driven value across time, and `assign`/`deassign`'s
//! parallel behavior on a register) is root
//! `tests/procedural_continuous_assign_e2e.rs`.

use piperine_lang::ast::Stmt;
use piperine_lang::pom::ElabError;
use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

fn first_digital_stmt(d: &piperine_lang::Design, module: &str) -> Stmt {
    let m = d.module(module).expect("module elaborated");
    let digital = m.behaviors().iter().find(|b| b.is_digital()).expect("a digital behavior");
    match &digital.body()[0] {
        Stmt::Block { stmts, .. } => stmts[0].clone(),
        other => other.clone(),
    }
}

/// `force lvalue = expr;` parses as `Stmt::Force`.
#[test]
fn force_parses_as_its_own_statement() {
    let d = elab(
        r#"
        module top ();
            reg r1;
            initial begin
                force r1 = 1;
            end
        endmodule
        "#,
    );
    match first_digital_stmt(&d, "top") {
        Stmt::Force { target, value, .. } => {
            assert!(matches!(target, piperine_lang::ast::Expr::Ident { name, .. } if name == "r1"));
            assert!(matches!(value, piperine_lang::ast::Expr::Number { .. }));
        }
        other => panic!("expected Stmt::Force, got {other:?}"),
    }
}

/// `release lvalue;` parses as `Stmt::Release`.
#[test]
fn release_parses_as_its_own_statement() {
    let d = elab(
        r#"
        module top ();
            reg r1;
            initial begin
                release r1;
            end
        endmodule
        "#,
    );
    match first_digital_stmt(&d, "top") {
        Stmt::Release { target, .. } => {
            assert!(matches!(target, piperine_lang::ast::Expr::Ident { name, .. } if name == "r1"));
        }
        other => panic!("expected Stmt::Release, got {other:?}"),
    }
}

/// `assign lvalue = expr;` (the statement form, distinct from the
/// module-level `assign` item T10 already implements) parses as
/// `Stmt::ProceduralAssign`.
#[test]
fn procedural_assign_parses_as_its_own_statement() {
    let d = elab(
        r#"
        module top ();
            reg r1;
            initial begin
                assign r1 = 1;
            end
        endmodule
        "#,
    );
    match first_digital_stmt(&d, "top") {
        Stmt::ProceduralAssign { target, .. } => {
            assert!(matches!(target, piperine_lang::ast::Expr::Ident { name, .. } if name == "r1"));
        }
        other => panic!("expected Stmt::ProceduralAssign, got {other:?}"),
    }
}

/// `deassign lvalue;` parses as `Stmt::ProceduralDeassign`.
#[test]
fn procedural_deassign_parses_as_its_own_statement() {
    let d = elab(
        r#"
        module top ();
            reg r1;
            initial begin
                deassign r1;
            end
        endmodule
        "#,
    );
    match first_digital_stmt(&d, "top") {
        Stmt::ProceduralDeassign { target, .. } => {
            assert!(matches!(target, piperine_lang::ast::Expr::Ident { name, .. } if name == "r1"));
        }
        other => panic!("expected Stmt::ProceduralDeassign, got {other:?}"),
    }
}

/// §9.3 of IEEE 1364-2005 gives `force`/`assign`/`deassign`/`release` no
/// analog counterpart — used inside an `analog` block, each is rejected
/// citing Annex C.7 (the same subset gate `<=`/`casex`/`casez` already
/// use), never silently accepted or misinterpreted as a contribution.
#[test]
fn force_inside_an_analog_block_fails_loud_citing_annex_c() {
    let src = r#"
        module top ();
            electrical out;
            analog begin
                force out = 1.0;
            end
        endmodule
    "#;
    let report = parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("`force` inside an analog block must fail elaboration (LRM Annex C.7)");
    let err = report
        .downcast_ref::<ElabError>()
        .unwrap_or_else(|| panic!("elaboration failure is a typed ElabError, got: {report}"));
    let msg = format!("{err}");
    assert!(msg.contains("force"), "expected the error to name `force`, got: {msg}");
    assert!(msg.to_lowercase().contains("annex c"), "expected the error to cite Annex C, got: {msg}");
}

/// Discrimination: `assign`/`deassign` inside an analog block are
/// rejected the same way `force`/`release` are — proving the Annex C.7
/// gate covers all four statement kinds independently, not only `force`.
#[test]
fn procedural_assign_inside_an_analog_block_fails_loud_citing_annex_c() {
    let src = r#"
        module top ();
            electrical out;
            analog begin
                assign out = 1.0;
            end
        endmodule
    "#;
    let report = parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("procedural `assign` inside an analog block must fail elaboration (LRM Annex C.7)");
    let err = report
        .downcast_ref::<ElabError>()
        .unwrap_or_else(|| panic!("elaboration failure is a typed ElabError, got: {report}"));
    let msg = format!("{err}");
    assert!(msg.to_lowercase().contains("annex c"), "expected the error to cite Annex C, got: {msg}");
}
