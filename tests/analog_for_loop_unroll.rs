//! LRM-COVERS: 5.9, 5.9.3
//! LRMW2-20 (`lrm-conformance-wave2`, T32): §5.9's `for`/`while`/`repeat`
//! with elaboration-constant bounds actually simulate, not merely
//! elaborate — the LRM's own §5.9.3 worked example sums a contribution
//! across a `for`-unrolled loop; this fixture keeps that shape (a `for`
//! loop summing into a real variable, then contributed) with a literal
//! bound instead of the LRM's array-net/`parameter` combination (see
//! `crates/piperine-codegen/tests/analog_loop_unroll.rs`'s header for the
//! full disclosed adaptation).
//!
//! The DC operating point makes the summed result observable end to end:
//! `I(a, gnd) <+ V(a) + tmp;` is a unit resistor to ground (`V(a)`) plus
//! the loop-summed constant current source (`tmp`) on the same branch —
//! KCL at `a` forces `V(a) = -tmp`.

use piperine::{Session, SolverConfig};
use piperine_lang::SourceMap;

fn dut_source(loop_body: &str) -> String {
    format!(
        "
module Dut (inout electrical a);
    integer k;
    real tmp;
    analog begin
        tmp = 0.0;
        {loop_body}
        I(a, gnd) <+ V(a) + tmp;
    end
endmodule

module Top ();
    electrical a;
    Dut d1 (a);
endmodule
"
    )
}

fn op_v_a(src: &str) -> f64 {
    let design = piperine_lang::parse_and_elaborate(src, &SourceMap::dummy()).expect("elaborates");
    let mut session = Session::compile(&design, "Top").expect("session compiles");
    let op = session.op(&SolverConfig::default(), None).expect("op solves");
    op.v("a").expect("v(a)")
}

/// AC15-style (P4's own bar, reused here): the LRM's own for-based
/// contribution-summing example must *simulate*, not merely elaborate —
/// `for (k = 0; k < 4; k = k + 1) tmp = tmp + k;` sums `0+1+2+3 = 6`,
/// giving `V(a) = -6.0` at the DC operating point.
#[test]
fn for_loop_sum_reaches_the_dc_operating_point() {
    let src = dut_source("for (k = 0; k < 4; k = k + 1) begin tmp = tmp + k; end");
    let v = op_v_a(&src);
    assert!((v - -6.0).abs() < 1e-6, "for-loop sum 0+1+2+3=6 must give V(a)=-6.0, got {v}");
}

/// Discriminating: a different literal bound changes the simulated
/// result (`< 6` sums `0+1+2+3+4+5 = 15`) — an ignored or
/// wrongly-unrolled loop can't reproduce both operating points.
#[test]
fn a_different_bound_gives_a_different_operating_point() {
    let src = dut_source("for (k = 0; k < 6; k = k + 1) begin tmp = tmp + k; end");
    let v = op_v_a(&src);
    assert!((v - -15.0).abs() < 1e-6, "for-loop sum 0+1+2+3+4+5=15 must give V(a)=-15.0, got {v}");
}

/// `repeat(N)` also reaches the DC operating point: `repeat(3) tmp = tmp
/// + 2.0;` sums 6.0, the same target as the `for`-loop test above (cross-
/// checking the two loop kinds against the same expected physical value).
#[test]
fn repeat_sum_reaches_the_dc_operating_point() {
    let src = dut_source("repeat (3) begin tmp = tmp + 2.0; end");
    let v = op_v_a(&src);
    assert!((v - -6.0).abs() < 1e-6, "repeat(3) of tmp+=2.0 must sum to 6.0, giving V(a)=-6.0, got {v}");
}
