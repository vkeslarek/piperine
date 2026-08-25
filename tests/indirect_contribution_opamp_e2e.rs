//! LRM-COVERS: 5.6.7, 5.6.7.1
//!
//! LRMW2-41 (`lrm-conformance-wave2`, T46): the AC15 end-to-end proof —
//! the LRM's own §5.6.7 ideal-opamp example (`V(out):V(pin,nin) == 0;`,
//! "a complete description of an ideal opamp") wired into a
//! voltage-follower feedback loop and DC-solved, proving the indirect
//! contribution's implicit equation actually solves to the LRM-stated
//! behavior (`out` tracks `pin` because the feedback drives `pin - nin`
//! to `0`) — not merely that the module elaborates/compiles
//! (`crates/piperine-codegen/tests/indirect_contribution.rs` already
//! covers that, plus the disclosed flow-nature/conditional scope cuts).

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::prelude::Context;

/// LRM §5.6.7's own opamp, in a voltage-follower feedback loop: `nin` is
/// tied to `out`, so the implicit equation `V(pin,nin) == 0` forces
/// `V(out) == V(pin)`, and `pin` is driven by an independent source —
/// `out` tracks whatever `pin` is set to.
fn follower_output(vin: f64) -> f64 {
    let src = format!(
        r#"
        module vsrc (inout electrical p, inout electrical n);
            parameter real dc = 1.0;
            analog V(p, n) <+ dc;
        endmodule

        module opamp (inout electrical out, pin, nin);
            analog V(out):V(pin,nin) == 0;
        endmodule

        module top ();
            electrical gnd, vin, vout;
            vsrc #(.dc({vin})) vs (.p(vin), .n(gnd));
            opamp op1 (vout, vin, vout);
        endmodule
        "#
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy())
        .unwrap_or_else(|e| panic!("fixture elaborates: {e:?}"));
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) =
        compiler.build_circuit_mapped("top").unwrap_or_else(|e| panic!("top builds: {e:?}"));
    let dc = circuit.dc(Context::default()).unwrap().solve().expect("dc solve");
    let vout = info.nets.get("vout").expect("net `vout`");
    dc.get_node(vout).expect("V(vout) at the operating point")
}

/// AC15: the indirect contribution's implicit equation actually solves —
/// `out` tracks `pin` (the LRM-stated ideal-opamp/nullor behavior), not a
/// dropped or default-zero value.
#[test]
fn opamp_follower_output_tracks_the_input() {
    let out = follower_output(1.5);
    assert!((out - 1.5).abs() < 1e-6, "expected V(out) == V(pin) == 1.5, got {out}");
}

/// Discrimination: a different input drives a different (tracked) output
/// — an inliner that dropped the equation or pinned `out` at a constant
/// would fail this.
#[test]
fn opamp_follower_output_tracks_a_different_input() {
    let out = follower_output(2.7);
    assert!((out - 2.7).abs() < 1e-6, "expected V(out) == V(pin) == 2.7, got {out}");
}

/// §5.6.7.1 ("Multiple indirect contributions"): two indirect
/// contribution statements form a genuine 2x2 linear system —
/// `V(x): V(x) == 3.0 - V(y);` and `V(y): V(y) == V(x) - 1.0;`
/// (equivalent to x+y=3, x-y=1) — solved together (x = 2, y = 1), not
/// independently or in isolation. Proves the LRM's own claim that "the
/// targets frequently can be paired with any equation" without affecting
/// the result: each equation's target (`V(x)`/`V(y)`) is not the
/// variable the equation is "about" in an isolated, per-statement sense
/// — the two must be solved jointly. (Grammar note: the indirect
/// statement's own LHS-of-`==` must be a bare probe — Syntax 5-6's
/// `indirect_expression` — so the coupling term lives on the RHS, unlike
/// the informal `x+y=3`/`x-y=1` framing.)
#[test]
fn two_indirect_contributions_solve_as_a_joint_linear_system() {
    let src = r#"
        module solver2x2 (inout electrical x, inout electrical y);
            analog begin
                V(x):V(x) == 3.0 - V(y);
                V(y):V(y) == V(x) - 1.0;
            end
        endmodule

        module top ();
            electrical gnd, nx, ny;
            ground gnd;
            solver2x2 s1 (nx, ny);
        endmodule
    "#;
    let design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy())
        .unwrap_or_else(|e| panic!("fixture elaborates: {e:?}"));
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) =
        compiler.build_circuit_mapped("top").unwrap_or_else(|e| panic!("top builds: {e:?}"));
    let dc = circuit.dc(Context::default()).unwrap().solve().expect("dc solve");
    let x = info.nets.get("nx").expect("net `nx`");
    let y = info.nets.get("ny").expect("net `ny`");
    let vx = dc.get_node(x).expect("V(nx) at the operating point");
    let vy = dc.get_node(y).expect("V(ny) at the operating point");
    assert!((vx - 2.0).abs() < 1e-6, "expected V(x) == 2.0 (x+y=3, x-y=1), got {vx}");
    assert!((vy - 1.0).abs() < 1e-6, "expected V(y) == 1.0, got {vy}");
}
