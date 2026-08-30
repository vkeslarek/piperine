//! LRM-COVERS: 5.6.7.2
//!
//! T32 (`lrm-conformance-wave3`, LRMW3-16): "once a branch is assigned
//! indirectly [...], it shall not also be driven by a direct contribution
//! statement" — a stricter rule than the ordinary §5.4.2 potential/flow
//! kind conflict, and a *different* axis from it: two contributions
//! (direct or indirect) of the *same* nature kind never conflict under
//! §5.4.2, but an indirect assignment and a direct contribution to the
//! same branch (or a parallel branch between the same node pair) always
//! conflict under §5.6.7.2, regardless of nature kind.
//!
//! Wave-2's T46 left this deliberately unenforced (`elab/lower/vams.rs`'s
//! `resolve_contribution_branches` doc comment: "mixing the two forms on
//! the same branch degrades to whatever the ordinary kind-conflict check
//! does today"): `add_indirect_contrib` (`flatten/analog.rs`) desugars an
//! indirect assignment into an ordinary `add_contrib` call, so a direct
//! contribution to the identical node pair simply accumulated into the
//! same `potential_acc` entry — no error at all. Fixed here with two new
//! `AnalogFlattener` fields (`direct_targets`/`indirect_targets`, node
//! pairs order-normalized) checked in a new `add_contrib_inner`
//! (`add_contrib`'s new body, plus a `via_indirect` flag `add_indirect_contrib`
//! passes through its own desugared call).
//!
//! Fixture note: a `top` module with **no instances at all** never gets
//! its own `AnalogKernel` compiled by `CircuitCompiler::build_circuit_mapped`
//! (a pre-existing, unrelated quirk of Piperine's per-module-type kernel
//! compilation) — every fixture below puts its analog body in a `leaf`
//! module `top` instantiates, so the flatten path this task changes
//! actually runs.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;

fn build(src: &str) -> Result<(), String> {
    let mut design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy())
        .map_err(|e| format!("elaborates: {e:?}"))?;
    design.set_top("top");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).map_err(|e| format!("lowers: {e:?}"))?;
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    compiler.build_circuit_mapped("top").map(|_| ()).map_err(|e| format!("{e:?}"))
}

/// AC (Done-when bullet 1): the conflict fails loud citing §5.6.7.2, on
/// the *same* branch (`out`/ground, once via `V(out):...`, once via a
/// direct `V(out, gnd) <+ ...`).
#[test]
fn mixed_indirect_and_direct_on_the_same_branch_fails_loud_citing_5_6_7_2() {
    let src = r#"
        module leaf(inout electrical out, inout electrical pin, inout electrical nin, inout electrical gnd);
            analog begin
                V(out):V(pin, nin) == 0;
                V(out, gnd) <+ 1.0;
            end
        endmodule
        module top();
            electrical out, pin, nin, gnd;
            leaf u1(out, pin, nin, gnd);
        endmodule
    "#;
    let err = build(src).expect_err("indirect + direct on the same branch must fail loud");
    assert!(err.contains("§5.6.7.2"), "expected §5.6.7.2 cited, got: {err}");
    // Both conflicting statements are labelled by byte offset (Done-when
    // bullet 1's own wording) — two distinct "start..end" pairs, not just
    // one.
    assert!(
        err.matches("..").count() >= 2,
        "expected two labelled byte-offset ranges (one per statement), got: {err}"
    );
}

/// AC (Done-when bullet 2): a *parallel* branch — a different access
/// function (`I` instead of `V`) between the identical node pair — is
/// caught too, not only a literal re-use of the same access-function call.
#[test]
fn a_parallel_branch_between_the_same_node_pair_is_caught_too() {
    let src = r#"
        module leaf(inout electrical out, inout electrical pin, inout electrical nin, inout electrical gnd);
            analog begin
                V(out):V(pin, nin) == 0;
                I(out, gnd) <+ 1.0;
            end
        endmodule
        module top();
            electrical out, pin, nin, gnd;
            leaf u1(out, pin, nin, gnd);
        endmodule
    "#;
    let err = build(src).expect_err("a parallel (out, gnd) flow contribution must also conflict");
    assert!(err.contains("§5.6.7.2"), "expected §5.6.7.2 cited, got: {err}");
}

/// AC (Done-when bullet 3, half 1): a legal indirect-only body still
/// compiles.
#[test]
fn a_legal_indirect_only_body_still_compiles() {
    let src = r#"
        module leaf(inout electrical out, inout electrical pin, inout electrical nin, inout electrical gnd);
            analog V(out):V(pin, nin) == 0;
        endmodule
        module top();
            electrical out, pin, nin, gnd;
            leaf u1(out, pin, nin, gnd);
        endmodule
    "#;
    build(src).expect("an indirect-only body must build cleanly");
}

/// AC (Done-when bullet 3, half 2): a legal direct-only body still
/// compiles.
#[test]
fn a_legal_direct_only_body_still_compiles() {
    let src = r#"
        module leaf(inout electrical out, inout electrical gnd);
            analog V(out, gnd) <+ 1.0;
        endmodule
        module top();
            electrical out, gnd;
            leaf u1(out, gnd);
        endmodule
    "#;
    build(src).expect("a direct-only body must build cleanly");
}

/// Two direct contributions to the *same* branch do not conflict with
/// EACH OTHER under §5.6.7.2 (ordinary accumulation, unaffected by this
/// task) — proves the new check is not over-broad.
#[test]
fn two_direct_contributions_to_the_same_branch_do_not_conflict() {
    let src = r#"
        module leaf(inout electrical out, inout electrical gnd);
            parameter real r = 1.0;
            analog begin
                V(out, gnd) <+ 1.0;
                V(out, gnd) <+ r;
            end
        endmodule
        module top();
            electrical out, gnd;
            leaf u1(out, gnd);
        endmodule
    "#;
    build(src).expect("two direct contributions to the same branch must not conflict under §5.6.7.2");
}
