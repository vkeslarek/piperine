//! LRM-COVERS: 4.5.9
//! LRMW2-34 (`lrm-conformance-wave2`, T37): `slew(expr, max_pos_slew_rate)`
//! with no third argument used to default `max_neg_slew_rate` to a bare
//! `0.0` instead of the LRM's "opposite of max_pos_slew_rate" — and `0.0`
//! is not merely "no fall limiting", it is worse: `device/analog/
//! operators.rs`/`piperine_ir::ops::Operator::slew`'s falling-edge branch
//! is `delta.max(-fall*dt)`, so `fall=0.0` clamps every negative `delta`
//! to exactly `0.0` — the output never falls at all, frozen at whatever
//! value it last rose to. §4.5.9: "If the max_neg_slew_rate is not
//! specified, it defaults to the opposite of the max_pos_slew_rate."
//!
//! Fixture: a one-argument `slew(x, rate=5.0)` fed a falling step
//! (5V -> 0V at t=3s, well after the rise settles at t=1s). Before the
//! fix, `V(out)` never leaves 5V once it gets there. After the fix, it
//! slews down at `rate` exactly as it slews up (1s to cross 5V either
//! way).

use piperine::{Session, SolverConfig};
use piperine_lang::SourceMap;

const SRC: &str = "
module Dut (inout electrical out);
    parameter real rate = 5.0;
    analog begin
        V(out) <+ slew(($abstime > 3.0) ? 0.0 : 5.0, rate);
    end
endmodule

module Top ();
    electrical out;
    Dut d1 (out);
endmodule
";

fn v_out_at(t: f64) -> f64 {
    let design = piperine_lang::parse_and_elaborate(SRC, &SourceMap::dummy()).expect("elaborates");
    let mut session = Session::compile(&design, "Top").expect("session compiles");
    let trace = session
        .tran(10.0, Some(0.02), 0.0, &SolverConfig::default(), None, false, &[])
        .expect("transient runs");
    trace.v("out").expect("v(out)").at(t)
}

/// Before the step (t=2s, 1s after the rise settles at rate=5V/s), the
/// output holds at the high level — a regression guard, not the
/// discriminating point (the rising slew already worked pre-fix).
#[test]
fn before_the_step_the_output_holds_at_the_high_level() {
    let v = v_out_at(2.0);
    assert!((v - 5.0).abs() < 0.1, "expected ~5V before the step, got {v}");
}

/// AC14 discrimination: well after the falling step (t=6s, 3s after the
/// step at t=3s — plenty of time to slew down 5V at rate=5V/s, which
/// takes 1s), the output must have reached ~0V. Pre-fix it would still
/// read ~5V (frozen — the falling edge never moves at all).
#[test]
fn well_after_the_falling_step_the_output_has_slewed_down_to_zero() {
    let v = v_out_at(6.0);
    assert!(v.abs() < 0.2, "expected the output to have slewed down to ~0V by t=6s, got {v}");
}
