//! LRM-COVERS: 4.5.8
//! LRMW2-36 (`lrm-conformance-wave2`, T38): `transition(expr, td, rise,
//! fall, time_tol)`'s fifth argument was silently discarded — the
//! doc comment on `kernel/analog/kernel.rs`'s `RuntimeState::Transition`
//! said so outright ("ttol ... is intentionally not carried here").
//! §4.5.8: "transition() returns a real number which describes a
//! piecewise linear function over time. The transition function causes
//! the simulator to place time-points at both corners of a transition. If
//! time_tol is not specified, the transition function causes the
//! simulator to assure each transition is adequately resolved."
//!
//! Implementation: the ramp's two corners were already forced breakpoints
//! (`piperine_ir::ops::Operator::pending_edges`) regardless of `time_tol`;
//! this task adds intermediate breakpoints through the ramp's interior,
//! spaced no farther apart than `time_tol` when it is given — a tighter
//! `time_tol` forces more forced landing points, hence more accepted
//! steps, over the identical ramp.

use piperine::{Session, SolverConfig};
use piperine_lang::SourceMap;

fn dut_source(time_tol: f64) -> String {
    format!(
        "
module Dut (inout electrical out);
    parameter real time_tol = 0.0;
    analog begin
        V(out) <+ transition(($abstime > 1.0) ? 5.0 : 0.0, 0.0, 4.0, 4.0, time_tol);
    end
endmodule

module Top ();
    electrical out;
    Dut #(.time_tol({time_tol})) d1 (out);
endmodule
"
    )
}

fn steps_accepted(time_tol: f64) -> usize {
    let src = dut_source(time_tol);
    let design = piperine_lang::parse_and_elaborate(&src, &SourceMap::dummy()).expect("elaborates");
    let mut session = Session::compile(&design, "Top").expect("session compiles");
    let trace = session
        .tran(6.0, Some(1.0), 0.0, &SolverConfig::default(), None, false, &[])
        .expect("transient runs");
    trace.stats().steps_accepted
}

/// AC14 discrimination: a tight `time_tol` (0.2, over a 4.0-long ramp)
/// forces materially more accepted steps than a loose one (0.0, the
/// pre-existing corners-only behavior) — an ignored `time_tol` argument
/// cannot reproduce this (both would land on the same two corners).
#[test]
fn a_tighter_time_tol_forces_more_accepted_steps_through_the_ramp() {
    let tight = steps_accepted(0.2);
    let loose = steps_accepted(0.0);
    assert!(
        tight > loose,
        "tight time_tol=0.2 ({tight} steps) must accept more steps than time_tol=0.0 ({loose} steps)"
    );
}

/// The ramp itself still reaches its target value regardless of
/// `time_tol` — the intermediate breakpoints only add resolution, they
/// never change what the transition converges to.
#[test]
fn the_ramp_still_settles_at_its_target_with_a_tight_time_tol() {
    let src = dut_source(0.2);
    let design = piperine_lang::parse_and_elaborate(&src, &SourceMap::dummy()).expect("elaborates");
    let mut session = Session::compile(&design, "Top").expect("session compiles");
    let trace = session
        .tran(6.0, Some(1.0), 0.0, &SolverConfig::default(), None, false, &[])
        .expect("transient runs");
    let v = trace.v("out").expect("v(out)").at(6.0);
    assert!((v - 5.0).abs() < 0.1, "expected the ramp settled at 5.0V by t=6s, got {v}");
}
