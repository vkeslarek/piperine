//! LRM-COVERS: 6.7, 6.2.1
//!
//! LRMW2-42 (`lrm-conformance-wave2`, T50): spec.md AC15's own requirement
//! ("a hierarchical reference reads a nested net") — end-to-end, *simulated*
//! (DC operating point), not merely elaborated. Before this task, `u1.u2.net`
//! and `$root.top.net` had no grammar production at all: `Tok::Ident("u1")`
//! became a bare `Expr::Ident`, and the following `.u2.net` was an
//! unexpected-token parse error (`grammar/expr.rs::parse_primary` had no
//! `.`-postfix handling; `$root` lexed as an ordinary `Tok::SystemIdent`,
//! parsed as a zero-arg `$root()` system-function call, then also choked on
//! the trailing `.top.net`).
//!
//! Fixture: three structural levels (`top` → `mid1` → `mid2`) wrap a leaf
//! device (`leaf`, a fixed voltage source) — `mid1`/`mid2` are pure
//! structural pass-throughs with their own declared wire, so
//! `FlattenHierarchy` lifts `mid2`'s wire `w2` through two dotted renames
//! (`elab/lower/flatten.rs`) into `top`'s own flat node `u1.u2.w2`: a
//! genuinely two-level-deep hierarchical read, not the pre-existing
//! one-level `instance.port` shortcut. `top`'s own analog body reads that
//! net through both the local (`u1.u2.w2`) and `$root`-rooted
//! (`$root.top.u1.u2.w2`) forms, each driving its own ideal-voltage-follower
//! output node.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::prelude::Context;

fn run(leaf_voltage: f64) -> (f64, f64) {
    let src = format!(
        r#"
        module leaf (inout electrical p, inout electrical gnd);
            analog V(p, gnd) <+ {leaf_voltage};
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
        "#
    );
    let mut design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy())
        .unwrap_or_else(|e| panic!("fixture elaborates: {e:?}"));
    design.set_top("top");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) =
        compiler.build_circuit_mapped("top").unwrap_or_else(|e| panic!("top builds: {e:?}"));
    let dc = circuit.dc(Context::default()).unwrap().solve().expect("dc solve");
    let n1 = info.nets.get("outnode1").expect("net `outnode1`");
    let n2 = info.nets.get("outnode2").expect("net `outnode2`");
    (
        dc.get_node(n1).expect("V(outnode1) at the operating point"),
        dc.get_node(n2).expect("V(outnode2) at the operating point"),
    )
}

/// AC15: both the local (`u1.u2.w2`) and `$root`-rooted
/// (`$root.top.u1.u2.w2`) hierarchical reads reach the same nested net two
/// structural levels down, tracking its 3.0 V source exactly (an ideal
/// voltage-follower read, no loading error).
#[test]
fn both_forms_read_the_nested_net_at_the_operating_point() {
    let (local, rooted) = run(3.0);
    assert!((local - 3.0).abs() < 1e-9, "local form `u1.u2.w2` should read 3.0V, got {local}");
    assert!((rooted - 3.0).abs() < 1e-9, "`$root` form should read 3.0V, got {rooted}");
}

/// Discrimination (Wave-2 standing rule / AC14's own pattern applied to
/// AC15): changing the nested net's driven value must change what both
/// hierarchical reads observe — a resolution that accidentally grounded the
/// reference (or wired it to an unrelated constant net) would pass the
/// 3.0V case above by coincidence but fail this one.
#[test]
fn both_forms_track_a_different_nested_source_value() {
    let (local, rooted) = run(-1.25);
    assert!((local - -1.25).abs() < 1e-9, "local form should track the new value -1.25V, got {local}");
    assert!((rooted - -1.25).abs() < 1e-9, "`$root` form should track the new value -1.25V, got {rooted}");
}
