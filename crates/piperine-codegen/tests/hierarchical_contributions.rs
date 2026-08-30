//! LRM-COVERS: 5.6.8, 5.6.8.1, 5.6.8.2
//!
//! T31 (`lrm-conformance-wave3`, LRMW3-15): `V(top.drv.x) <+ 1.8;` drives
//! a hierarchical net (§5.6.8.1), `V(top.drv.br_v) <+ 1.8;` drives an
//! already-declared hierarchical *named* branch (§5.6.8), and a
//! hierarchical contribution that would turn a named branch's kind into
//! the §5.6.5 switch-branch promotion fails loud instead (§5.6.8.2 — that
//! promotion is legal only for a *local* branch inside a conditional,
//! never across a hierarchical boundary).
//!
//! **The net case** rides T30's `resolve_hier` fix directly: once a
//! hierarchical contribution *target* resolves through the same
//! `resolve_hier` path a hierarchical *read* already used, driving it is
//! nothing new — no separate mechanism, just T30 applied to a target
//! instead of a read. The blocking bug was one layer up:
//! `piperine-lang`'s own `net_ref_for_branch` (elaboration's contribution-
//! branch bookkeeping, `elab/lower/vams.rs`) rejected a `Hier` terminal
//! outright ("port connection must be a net name..."), failing the whole
//! module before codegen ever ran — fixed by accepting a `Hier` terminal
//! there as a plain dotted-name `NetRef` (identity bookkeeping only; the
//! real resolution still happens in codegen's `resolve_hier`).
//!
//! **The named-branch case** is new machinery:
//! `resolve/pom/body.rs::LowerCtx::resolve_named_branch_hier` walks the
//! *authored* (un-flattened) instance tree from the referencing module
//! down through the hierarchical path's prefix to find the module that
//! owns the named branch, then returns its two terminals for
//! `resolve/pom/expr.rs`'s `Expr::AccessFunction` arm to re-resolve
//! through the ordinary `resolve_hier` path and rebuild as an explicit
//! two-terminal access. `resolve_terminal_or_defer` (noise-source branch
//! attribution, a separate pre-`resolve_expr` pass over the *raw* target)
//! needed the identical lookup — a local named branch (`V(br) <+ ...`
//! with no hierarchical prefix) was, and remains, out of scope: no
//! codegen consumer resolves a branch by name without a net pair at all
//! (a pre-existing, unrelated gap; see the file's own disclosed-gap note
//! below).
//!
//! **Disclosed scope**: `resolve_named_branch_hier` only resolves one
//! level of *port*-typed branch terminals (the same one-level
//! `instance_ports` shortcut T30 relies on) — a branch terminal that is
//! itself a port two or more levels deep is not covered (no fixture in
//! this design exercises it, and the LRM's own §5.6.8 examples are
//! one level deep).

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::prelude::Context;

/// AC1 (§5.6.8.1): a hierarchical contribution to a net drives it —
/// asserted by solving and reading the leaf node.
#[test]
fn hierarchical_contribution_to_a_net_drives_it() {
    let src = r#"
        module drv(inout electrical a);
        endmodule
        module top();
            electrical x, gnd;
            drv u1(x);
            analog begin
                V(u1.a, gnd) <+ 1.8;
            end
        endmodule
    "#;
    let mut design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy())
        .unwrap_or_else(|e| panic!("fixture elaborates: {e:?}"));
    design.set_top("top");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) =
        compiler.build_circuit_mapped("top").unwrap_or_else(|e| panic!("top builds: {e:?}"));
    let dc = circuit.dc(Context::default()).unwrap().solve().expect("dc solve");
    let x = info.nets.get("x").expect("net `x`");
    let v = dc.get_node(x).expect("V(x) at the operating point");
    assert!((v - 1.8).abs() < 1e-9, "V(u1.a) <+ 1.8 must drive x to 1.8V, got {v}");
}

/// Discrimination companion: a different driven value must change what
/// the leaf net observes.
#[test]
fn hierarchical_contribution_to_a_net_tracks_a_different_value() {
    let src = r#"
        module drv(inout electrical a);
        endmodule
        module top();
            electrical x, gnd;
            drv u1(x);
            analog begin
                V(u1.a, gnd) <+ -2.5;
            end
        endmodule
    "#;
    let mut design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy())
        .unwrap_or_else(|e| panic!("fixture elaborates: {e:?}"));
    design.set_top("top");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) =
        compiler.build_circuit_mapped("top").unwrap_or_else(|e| panic!("top builds: {e:?}"));
    let dc = circuit.dc(Context::default()).unwrap().solve().expect("dc solve");
    let x = info.nets.get("x").expect("net `x`");
    let v = dc.get_node(x).expect("V(x)");
    assert!((v - -2.5).abs() < 1e-9, "expected -2.5V, got {v}");
}

/// AC2 (§5.6.8): a hierarchical contribution to a named branch applies —
/// `V(u1.br_v) <+ 1.8;` drives `br_v`'s own declared terminals (a, g), not
/// a literal net named "br_v".
#[test]
fn hierarchical_contribution_to_a_named_branch_applies() {
    let src = r#"
        module drv(inout electrical a, inout electrical g);
            branch (a, g) br_v;
        endmodule
        module top();
            electrical x, gnd;
            drv u1(x, gnd);
            analog begin
                V(u1.br_v) <+ 1.8;
            end
        endmodule
    "#;
    let mut design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy())
        .unwrap_or_else(|e| panic!("fixture elaborates: {e:?}"));
    design.set_top("top");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) =
        compiler.build_circuit_mapped("top").unwrap_or_else(|e| panic!("top builds: {e:?}"));
    let dc = circuit.dc(Context::default()).unwrap().solve().expect("dc solve");
    let x = info.nets.get("x").expect("net `x`");
    let v = dc.get_node(x).expect("V(x) driven through the named branch's `a` terminal");
    assert!((v - 1.8).abs() < 1e-9, "V(u1.br_v) <+ 1.8 must drive x (br_v's `a` terminal) to 1.8V, got {v}");
}

/// Discrimination companion for the named-branch case.
#[test]
fn hierarchical_contribution_to_a_named_branch_tracks_a_different_value() {
    let src = r#"
        module drv(inout electrical a, inout electrical g);
            branch (a, g) br_v;
        endmodule
        module top();
            electrical x, gnd;
            drv u1(x, gnd);
            analog begin
                V(u1.br_v) <+ 0.42;
            end
        endmodule
    "#;
    let mut design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy())
        .unwrap_or_else(|e| panic!("fixture elaborates: {e:?}"));
    design.set_top("top");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) =
        compiler.build_circuit_mapped("top").unwrap_or_else(|e| panic!("top builds: {e:?}"));
    let dc = circuit.dc(Context::default()).unwrap().solve().expect("dc solve");
    let x = info.nets.get("x").expect("net `x`");
    let v = dc.get_node(x).expect("V(x)");
    assert!((v - 0.42).abs() < 1e-9, "expected 0.42V, got {v}");
}

/// AC3 (§5.6.8.2): a hierarchical contribution whose access-function kind
/// disagrees with the target branch's already-established kind fails
/// loud — the §5.6.5 switch-branch promotion (legal only for a *local*
/// branch inside a conditional) never applies across a hierarchical
/// boundary. `drv`'s own body drives `br_v` as a Flow branch
/// (`I(br_v) <+ ...`); `top`'s hierarchical `V(u1.br_v) <+ ...` (Potential)
/// disagrees.
#[test]
fn hierarchical_contribution_that_would_switch_a_named_branch_fails_loud() {
    let src = r#"
        module drv(inout electrical a, inout electrical g);
            branch (a, g) br_v;
            analog I(br_v) <+ 1.0;
        endmodule
        module top();
            electrical x, gnd;
            drv u1(x, gnd);
            analog begin
                V(u1.br_v) <+ 1.8;
            end
        endmodule
    "#;
    let mut design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy())
        .unwrap_or_else(|e| panic!("fixture elaborates: {e:?}"));
    design.set_top("top");
    let err = piperine_codegen::resolve::lower_bodies(&design)
        .expect_err("V(u1.br_v) <+ ... disagrees with br_v's own Flow kind and must fail loud");
    let piperine_codegen::resolve::pom::LowerErrors(errs) = err;
    assert!(
        errs.iter().any(|e| e.what.contains("§5.6.8.2") && e.name.contains("br_v")),
        "expected a §5.6.8.2 error naming `br_v`, got: {errs:?}"
    );
}
