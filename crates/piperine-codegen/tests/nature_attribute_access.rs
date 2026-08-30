//! LRM-COVERS: 5.5.3
//!
//! lrm-conformance-wave3 (Phase 6, T36 follow-on): §5.5.3 "Accessing
//! attributes" — `net_identifier.potential_or_flow.nature_attribute_
//! identifier` reads a nature attribute's declared value through a net
//! reference, and the section's own text calls out `ddt()`'s second
//! argument as the typical place that value is used: "the `abstol`
//! attribute of a nature may also be accessed simply by using the
//! nature's identifier ... to the `ddt()`, `idt()`, or `idtmod()`
//! operators". This target proves the *net-reference* spelling
//! (`a.potential.abstol`), not the already-covered bare-nature-identifier
//! spelling (`crates/piperine-codegen/tests/ddt_abstol_arg.rs`).
//!
//! Before this task `resolve_expr` only matched a bare `Expr::Ident`
//! against `ctx.nature_abstols` (LRMW2-era work) — `a.potential.abstol`
//! parses as `Expr::Hier { path: ["a", "potential", "abstol"] }` and fell
//! straight through to ordinary hierarchical-name resolution, which fails
//! with "unresolved name" (the probed-gap table's LOWER-ERR row for
//! §5.5.3). `resolve_nature_attr_access` in
//! `crates/piperine-codegen/src/resolve/pom/expr.rs` closes that gap: a
//! three-segment `Hier` path is tried against the current module's own
//! net/discipline/nature chain before falling back to the hierarchical
//! path.

use piperine_codegen::resolve::{StateKind, StateVar};

fn ddt_state(src: &str) -> StateVar {
    let design = piperine_lang::parse_and_elaborate(src, &piperine_lang::SourceMap::dummy())
        .expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let body = &bodies["twocap"];
    body.symbols
        .states()
        .find_map(|(_, s)| matches!(s.kind, StateKind::Ddt { .. }).then(|| s.clone()))
        .expect("a ddt state")
}

fn abstol_of(state: StateVar) -> f64 {
    let StateKind::Ddt { abstol } = state.kind else { unreachable!() };
    let Some(piperine_lang::ast::Expr::Number { value, .. }) = abstol else {
        panic!("expected a resolved numeric abstol")
    };
    match value {
        piperine_lang::lexer::NumberLit::Real(v) => v,
        piperine_lang::lexer::NumberLit::Integer(v) => v as f64,
        other => panic!("expected a Real/Integer abstol literal, got {other:?}"),
    }
}

/// §5.5.3's own worked example (`twocap`'s `a.potential.abstol` line,
/// module name kept), minus the LRM's second `branch (n1, n2) cap;`
/// contribution (out of this clause's scope — it exercises §5.4.1 branch
/// probing, not attribute access) and modulo whitespace the LRM's
/// PDF-to-Markdown extraction mangled (`parameterreal c= 1p;` ->
/// `parameter real c = 1p;`): `a.potential.abstol` resolves to
/// `electrical`'s bound potential nature's declared `abstol`, exactly the
/// value a bare `electrical_volt` identifier would have resolved to.
#[test]
fn net_dot_potential_dot_abstol_resolves_to_the_bound_natures_abstol() {
    let src = r#"
        nature electrical_volt;
            units = "V";
            access = V;
            abstol = 1u;
        endnature
        nature electrical_curr;
            units = "A";
            access = I;
            abstol = 1p;
        endnature
        discipline electrical;
            potential electrical_volt;
            flow electrical_curr;
        enddiscipline
        module twocap (inout electrical a, inout electrical b);
        parameter real c = 1p;
        analog begin
            I(a,b) <+ c*ddt(V(a,b), a.potential.abstol);
        end
        endmodule
    "#;
    let abstol = abstol_of(ddt_state(src));
    assert!(
        (abstol - 1e-6).abs() < 1e-12,
        "a.potential.abstol must resolve to electrical_volt's declared abstol (1u) — got {abstol}"
    );
}

/// §3.6.2.5's discipline-level override wins over the bound nature's own
/// declared value — the same precedence T23 gave the solver-facing
/// tolerance path, now also observable through a §5.5.3 attribute read.
#[test]
fn discipline_override_wins_over_the_bound_natures_own_abstol() {
    let src = r#"
        nature electrical_volt;
            units = "V";
            access = V;
            abstol = 1u;
        endnature
        nature electrical_curr;
            units = "A";
            access = I;
            abstol = 1p;
        endnature
        discipline electrical;
            potential electrical_volt;
            flow electrical_curr;
            potential.abstol = 10u;
        enddiscipline
        module twocap (inout electrical a, inout electrical b);
        parameter real c = 1p;
        analog begin
            I(a,b) <+ c*ddt(V(a,b), a.potential.abstol);
        end
        endmodule
    "#;
    let abstol = abstol_of(ddt_state(src));
    assert!(
        (abstol - 10e-6).abs() < 1e-12,
        "a.potential.abstol must see the discipline's own override (10u), not the base nature's 1u — got {abstol}"
    );
}

/// An attribute the nature does not declare fails loud (falls back to
/// ordinary hierarchical-name resolution, which reports an unresolved
/// name) rather than silently resolving to `0.0` or some other default —
/// the spec's own edge case for this clause.
#[test]
fn undeclared_attribute_is_not_silently_accepted() {
    let src = r#"
        nature electrical_volt;
            units = "V";
            access = V;
        endnature
        nature electrical_curr;
            units = "A";
            access = I;
        endnature
        discipline electrical;
            potential electrical_volt;
            flow electrical_curr;
        enddiscipline
        module twocap (inout electrical a, inout electrical b);
        analog begin
            I(a,b) <+ ddt(V(a,b), a.potential.abstol);
        end
        endmodule
    "#;
    let design = piperine_lang::parse_and_elaborate(src, &piperine_lang::SourceMap::dummy())
        .expect("elaborates (the attribute reference itself is legal grammar)");
    let err = piperine_codegen::resolve::lower_bodies(&design)
        .expect_err("a.potential.abstol must fail loud when electrical_volt declares no abstol");
    let msg = format!("{err}");
    assert!(
        msg.to_lowercase().contains("unresolved") || msg.to_lowercase().contains("a.potential.abstol"),
        "expected an unresolved-name style error naming the failed reference, got: {msg}"
    );
}
