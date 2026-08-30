//! LRM-COVERS: 5.4.4
//!
//! lrm-conformance-wave3 (Phase 6, T36): §5.4.4 "Unassigned sources" — "If
//! a value is not assigned to a branch, and it is not a probe branch, the
//! branch flow is set to zero (0)." The LRM's own illustration is a
//! one-armed `if (closed) V(p,n) <+ 0;`, equivalent to the two-armed
//! `if (closed) V(p,n) <+ 0; else I(p,n) <+ 0;`.
//!
//! `crates/piperine-codegen/src/flatten/analog.rs`'s `add_contribution`
//! already wraps every conditional contribution as
//! `select(guard, resolved, lit(0.0))` regardless of switch-branch
//! promotion — this target proves that wrapping is real, observable
//! behavior at a DC operating point, not just a code-reading claim: the
//! unit-resistor-plus-source trick from
//! `tests/analog_for_loop_unroll.rs` (`I(a, gnd) <+ V(a) + tmp;` forces
//! `V(a) = -tmp` at DC) with `tmp`'s contribution split into its own
//! `if (closed)`-guarded statement with no `else` arm.

use piperine::{Session, SolverConfig};
use piperine_lang::SourceMap;

const DUT: &str = "
module Dut (inout electrical a);
    parameter integer closed = 0;
    real tmp;
    analog begin
        tmp = 1.0;
        I(a, gnd) <+ V(a);
        if (closed) I(a, gnd) <+ tmp;
    end
endmodule

module Top ();
    electrical a;
    Dut d1 (a);
endmodule
";

fn op_v_a(closed: i64) -> f64 {
    let design = piperine_lang::parse_and_elaborate(DUT, &SourceMap::dummy()).expect("elaborates");
    let mut session = Session::builder(&design, "Top")
        .stage("d1", "closed", piperine_lang::Value::Int(closed))
        .compile()
        .expect("session compiles");
    let op = session.op(&SolverConfig::default(), None).expect("op solves");
    op.v("a").expect("v(a)")
}

/// The `if`'s condition is false and there is no `else` — §5.4.4 requires
/// the branch flow to be zero, exactly as if `else I(a, gnd) <+ 0;` had
/// been written. If the guard's default were anything else (leftover
/// garbage, an unresolved-name placeholder, a silently skipped statement
/// that kept the branch out of the KCL sum entirely), `V(a)` would not be
/// `0.0`.
#[test]
fn unassigned_conditional_flow_defaults_to_zero() {
    let v = op_v_a(0);
    assert!((v - 0.0).abs() < 1e-9, "closed=0: V(a) must be 0.0 (unassigned flow = 0, §5.4.4), got {v}");
}

/// The condition is true: the guarded contribution applies for real,
/// proving the `0.0` above is the guard's *default*, not the case where
/// the whole conditional contribution is a no-op regardless of the guard.
#[test]
fn guarded_flow_applies_when_the_condition_is_true() {
    let v = op_v_a(1);
    assert!((v - (-1.0)).abs() < 1e-9, "closed=1: V(a) must be -1.0 (I(a,gnd) <+ V(a) + 1.0 = 0), got {v}");
}
