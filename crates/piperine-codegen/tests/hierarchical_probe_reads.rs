//! LRM-COVERS: 5.5.4
//!
//! T30 (`lrm-conformance-wave3`, LRMW3-15): a hierarchical probe read
//! (`V(u1.a, gnd)`) resolves at circuit build instead of failing with
//! "unresolved node". Wave-2's T50 already resolved a hierarchical
//! `Expr::Hier` path to a plain `Ident` during `lower_bodies`
//! (`resolve/pom/body.rs::resolve_hier`) — but for a path that resolves
//! through the *one-level* `instance_ports` shortcut (`"u1.a"` → the
//! parent's own node, e.g. `"x"`), `resolve_hier` emitted the alias
//! string `"u1.a"` itself rather than the node's actual canonical
//! registered name — a name that exists nowhere in the module's own
//! `SymbolTable`, so circuit build/JIT-emit failed with `unresolved node
//! "u1.a"` even though `lower_bodies` itself reported success. Fixed by a
//! new `LowerCtx::node_names` (id → canonical name) reverse map,
//! consulted by `resolve_hier` before emitting the resolved `Ident`.
//!
//! The multi-level case (`u1.u2.w2`, a nested *wire* `FlattenHierarchy`
//! already lifts under that exact dotted name) was never broken — it is
//! `tests/hierarchical_ref_e2e.rs`'s own subject at the repo root and is
//! not re-tested here.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::prelude::Context;

/// AC1: a `$strobe`/expression read of a leaf net returns the leaf's
/// solved value — the one-level `instance.port` form (`V(u1.a, gnd)`),
/// which pre-fix failed at circuit build with "unresolved node `u1.a`"
/// even though elaboration/lowering reported success.
#[test]
fn one_level_hierarchical_read_resolves_to_the_leaf_value() {
    let src = r#"
        module drv(inout electrical a);
            analog V(a) <+ 1.8;
        endmodule
        module top();
            electrical x, y, gnd;
            drv u1(x);
            analog begin
                V(y, gnd) <+ V(u1.a, gnd);
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
    let y = info.nets.get("y").expect("net `y`");
    let v = dc.get_node(y).expect("V(y) at the operating point");
    assert!((v - 1.8).abs() < 1e-9, "V(u1.a) must read the leaf's 1.8V, got {v}");
}

/// Discrimination companion: a different driven value at the leaf must
/// change what the hierarchical read observes.
#[test]
fn one_level_hierarchical_read_tracks_a_different_leaf_value() {
    let src = r#"
        module drv(inout electrical a);
            analog V(a) <+ -0.75;
        endmodule
        module top();
            electrical x, y, gnd;
            drv u1(x);
            analog begin
                V(y, gnd) <+ V(u1.a, gnd);
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
    let y = info.nets.get("y").expect("net `y`");
    let v = dc.get_node(y).expect("V(y)");
    assert!((v - -0.75).abs() < 1e-9, "expected -0.75V, got {v}");
}

/// AC2: an unnamed branch is created in the *reading* module for a
/// hierarchical read — asserted through the branch catalog
/// (`Design::module("top").branches()`), not inferred from the simulated
/// result. `top`'s own body never writes a contribution naming `u1.a`
/// directly (only reads it inside another contribution's *value*), so
/// this branch could only exist if the read itself created it.
#[test]
fn hierarchical_read_creates_an_unnamed_branch_in_the_reading_module() {
    let src = r#"
        module drv(inout electrical a);
            analog V(a) <+ 1.8;
        endmodule
        module top();
            electrical x, y, gnd;
            drv u1(x);
            analog begin
                V(y, gnd) <+ V(u1.a, gnd);
            end
        endmodule
    "#;
    let design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy())
        .unwrap_or_else(|e| panic!("fixture elaborates: {e:?}"));
    let top = design.module("top").expect("top in authored modules");
    let found = top
        .branches()
        .iter()
        .any(|b| b.is_implicit() && b.pos().net() == "u1.a" && b.neg().map(|n| n.net()) == Some("gnd"));
    assert!(
        found,
        "expected an unnamed branch (u1.a, gnd) in top's own branch catalog, got: {:?}",
        top.branches().iter().map(|b| (b.pos().to_string(), b.neg().map(|n| n.to_string()))).collect::<Vec<_>>()
    );
}

/// AC3: an unresolvable hierarchical path fails loud naming the path and
/// the nearest resolved prefix (a pre-existing T50 guarantee — the
/// canonical-name fix above must not weaken it).
#[test]
fn unresolvable_hierarchical_path_fails_loud() {
    let src = r#"
        module drv(inout electrical a);
            analog V(a) <+ 1.8;
        endmodule
        module top();
            electrical x, y, gnd;
            drv u1(x);
            analog begin
                V(y, gnd) <+ V(u1.bogus, gnd);
            end
        endmodule
    "#;
    let mut design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy())
        .unwrap_or_else(|e| panic!("fixture elaborates: {e:?}"));
    design.set_top("top");
    let err = piperine_codegen::resolve::lower_bodies(&design)
        .expect_err("`u1.bogus` names no real port/net and must fail lowering");
    let piperine_codegen::resolve::pom::LowerErrors(errs) = err;
    assert!(
        errs.iter().any(|e| e.what.contains("6.7") && e.name.contains("bogus")),
        "expected a §6.7 hierarchical-net error naming the failing segment `bogus`, got: {errs:?}"
    );
}
