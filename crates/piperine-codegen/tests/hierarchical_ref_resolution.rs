//! LRM-COVERS: 6.7, 6.2.1
//!
//! LRMW2-42 (`lrm-conformance-wave2`, T50): `resolve/pom/expr.rs`'s
//! `resolve_expr` resolves a POM `Expr::Hier` (`u1.u2.net`, `$root.top.net`)
//! against the current module's own flattened instance tree
//! (`Design::flat_modules`, MD-25's sanctioned side artifact — `flat_modules`
//! is never `Design::modules`) into a plain `Ident` naming the resolved flat
//! net, or records a fail-loud, typed `LowerError` naming the failing
//! segment. This file covers `lower_bodies`'s own resolution behavior
//! directly (both success and the three disclosed failure modes); the
//! end-to-end *simulated* proof (AC15) is
//! `tests/hierarchical_ref_e2e.rs` at the repo root.
//!
//! Fixture: `leaf` is a genuine leaf (behavioral) module — never inlined by
//! `FlattenHierarchy`. `mid2`/`mid1` are purely structural (no behavior of
//! their own) — each level down, `FlattenHierarchy::inline` lifts the
//! child's own wire under a fresh dotted name (`elab/lower/flatten.rs`'s
//! `"{inst}.{wire}"` rename), so by the time `top` is flattened, `leaf`'s
//! connected net is named `u1.u2.w2` in `top`'s own node table — a genuine
//! two-level dotted path, not the pre-existing one-level `instance.port`
//! shortcut (`LowerCtx::instance_ports`).

use piperine_lang::parse_and_elaborate;
use piperine_lang::ast::{Expr, Stmt};
use piperine_lang::SourceMap;
use piperine_codegen::resolve::pom::LowerErrors;

const FIXTURE: &str = r#"
    module leaf (inout electrical p, inout electrical gnd);
        analog V(p, gnd) <+ 3.0;
    endmodule

    module mid2 (inout electrical gnd);
        electrical w2;
        leaf u3 (w2, gnd);
    endmodule

    module mid1 (inout electrical gnd);
        mid2 u2 (gnd);
    endmodule

    module top ();
        electrical gnd, outnode1, outnode2;
        mid1 u1 (gnd);
        analog begin
            V(outnode1, gnd) <+ V(u1.u2.w2, gnd);
            V(outnode2, gnd) <+ V($root.top.u1.u2.w2, gnd);
        end
    endmodule
"#;

/// Flattens every `Stmt::Block`/plain-stmt shape a behavior body might use
/// down to its leaf `Contribution`s (the fixture's `analog begin ... end`
/// wraps two).
fn contributions(stmts: &[Stmt]) -> Vec<(&Expr, &Expr)> {
    let mut out = vec![];
    for s in stmts {
        match s {
            Stmt::Contribution { target, value, .. } => out.push((target, value)),
            Stmt::Block { stmts, .. } => out.extend(contributions(stmts)),
            _ => {}
        }
    }
    out
}

fn ident_name(e: &Expr) -> Option<&str> {
    match e {
        Expr::Ident { name, .. } => Some(name.as_str()),
        _ => None,
    }
}

/// A resolved hierarchical read sits inside `V(<resolved>, gnd)` — this
/// unwraps the `AccessFunction`'s `pos` to inspect what the `Hier` actually
/// resolved to.
fn access_pos_name(e: &Expr) -> Option<&str> {
    match e {
        Expr::AccessFunction { pos, .. } => ident_name(pos),
        _ => None,
    }
}

/// A contribution target is `V(<pos>, <neg>)` — an `AccessFunction`, not a
/// bare `Ident` — so identifying *which* contribution is "the one to
/// `outnode1`" reads its `pos` terminal.
fn target_pos_name(e: &Expr) -> Option<&str> {
    match e {
        Expr::AccessFunction { pos, .. } => ident_name(pos),
        _ => None,
    }
}

/// Success path, both `root: false` (`u1.u2.w2`) and `root: true`
/// (`$root.top.u1.u2.w2`) forms: each resolves to the *same* flat node name
/// `Ident("u1.u2.w2")` — proof that `$root.<top>.` stripping and the local
/// form converge on identical resolution, and that a `Hier` never survives
/// `lower_bodies` (MD-25-adjacent: the resolved body, not `Design::modules`,
/// is what gets rewritten).
#[test]
fn local_and_root_forms_both_resolve_to_the_same_flat_node_ident() {
    let mut design = parse_and_elaborate(FIXTURE, &SourceMap::dummy()).expect("fixture elaborates");
    design.set_top("top");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let top = &bodies["top"];
    let analog = top.analog.as_ref().expect("top has an analog body");
    let contribs = contributions(&analog.stmts);
    assert_eq!(contribs.len(), 2, "both contributions must survive lowering");

    let local = contribs.iter().find(|(t, _)| target_pos_name(t) == Some("outnode1")).expect("outnode1 contribution");
    assert_eq!(
        access_pos_name(local.1),
        Some("u1.u2.w2"),
        "the local `V(u1.u2.w2, gnd)` form must resolve its Hier arg to the flat node Ident, not survive as Hier"
    );

    let rooted = contribs.iter().find(|(t, _)| target_pos_name(t) == Some("outnode2")).expect("outnode2 contribution");
    assert_eq!(
        access_pos_name(rooted.1),
        Some("u1.u2.w2"),
        "`V($root.top.u1.u2.w2, gnd)` must resolve to the SAME flat node Ident as the local form"
    );
}

/// MD-25: resolving `Hier` rewrites only the *resolved* body — the authored
/// POM (`Design::modules`) must still show `top` instantiating `mid1` (not
/// `leaf` directly, not a collapsed 2-level tree) exactly as written.
#[test]
fn hier_resolution_never_touches_authored_pom_structure() {
    let mut design = parse_and_elaborate(FIXTURE, &SourceMap::dummy()).expect("fixture elaborates");
    design.set_top("top");
    let _ = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let top = design.module("top").expect("top in authored modules");
    assert_eq!(top.instances.len(), 1, "top must still authored-instantiate exactly one instance (mid1)");
    assert_eq!(top.instances[0].module, "mid1", "top's sole instance must still be `mid1`, not inlined/collapsed");
    let mid1 = design.module("mid1").expect("mid1 in authored modules");
    assert_eq!(mid1.instances.len(), 1, "mid1 must still authored-instantiate exactly one instance (mid2)");
    assert_eq!(mid1.instances[0].module, "mid2");
}

/// An unresolvable local path (a typo'd final segment) fails loud as a
/// typed `LowerErrors`/`LowerError` naming the failing segment — never a
/// silently grounded/zeroed net.
#[test]
fn unresolvable_local_segment_fails_loud_naming_it() {
    let src = FIXTURE.replace("u1.u2.w2", "u1.u2.bogus").replace(
        "V(outnode2, gnd) <+ V($root.top.u1.u2.bogus, gnd);",
        "V(outnode2, gnd) <+ 0;", // keep the $root line trivially valid so only the local form is under test
    );
    let mut design = parse_and_elaborate(&src, &SourceMap::dummy()).expect("fixture elaborates");
    design.set_top("top");
    let err = piperine_codegen::resolve::lower_bodies(&design)
        .expect_err("`u1.u2.bogus` names no real flat net and must fail lowering");
    let LowerErrors(errs) = err;
    assert!(
        errs.iter().any(|e| e.what.contains("6.7") && e.name.contains("bogus")),
        "expected a §6.7 hierarchical-net error naming the failing segment `bogus`, got: {errs:?}"
    );
}

/// `$root`'s first segment must name the design's actual top-level module
/// (§6.2.1) — a wrong name fails loud citing that segment, never silently
/// falling back to the real top.
#[test]
fn root_with_wrong_top_module_segment_fails_loud() {
    let src = FIXTURE.replace("$root.top.u1.u2.w2", "$root.nottop.u1.u2.w2");
    let mut design = parse_and_elaborate(&src, &SourceMap::dummy()).expect("fixture elaborates");
    design.set_top("top");
    let err = piperine_codegen::resolve::lower_bodies(&design)
        .expect_err("`$root.nottop...` must fail: `nottop` is not the design's top module (`top`)");
    let LowerErrors(errs) = err;
    assert!(
        errs.iter().any(|e| e.what.contains("6.2.1") && e.name.contains("nottop")),
        "expected a §6.2.1 error naming the failing segment `nottop`, got: {errs:?}"
    );
}

/// A `$root` reference read from a module that is *not* the design's
/// top-level module is a disclosed scope gap (T50 SPEC_DEVIATION: each
/// module compiles once and is instantiated many times, so a non-top
/// module's kernel has no channel to reach into a different module's
/// node space) — fails loud rather than silently resolving something
/// unrelated.
#[test]
fn root_reference_from_a_non_top_module_fails_loud() {
    let src = r#"
        module leaf (inout electrical p, inout electrical gnd);
            analog V(p, gnd) <+ 3.0;
        endmodule
        module mid (inout electrical gnd);
            electrical w, probe;
            leaf u1 (w, gnd);
            analog V(probe, gnd) <+ $root.top.u1.u2.w2;
        endmodule
        module top ();
            electrical gnd, probeout;
            mid u1 (gnd);
        endmodule
    "#;
    let mut design = parse_and_elaborate(src, &SourceMap::dummy()).expect("fixture elaborates");
    design.set_top("top");
    let err = piperine_codegen::resolve::lower_bodies(&design)
        .expect_err("`$root` read from `mid` (not the design's top) must fail");
    let LowerErrors(errs) = err;
    assert!(
        errs.iter().any(|e| e.module == "mid" && e.what.contains("6.2.1") && e.what.contains("non-top")),
        "expected a §6.2.1 error naming module `mid` and the non-top-module gap, got: {errs:?}"
    );
}
