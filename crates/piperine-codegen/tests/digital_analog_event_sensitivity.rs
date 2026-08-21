//! LRM-COVERS: 7.3.5
//!
//! LRM §7.3.5 "Detecting continuous events in a discrete context": a digital
//! (`initial`/`always`) block may be sensitive to an analog event function
//! (`cross`/`above`/`timer`/`absdelta`), and its body runs when that event
//! FIRES, not on every digital evaluation pass. `kernel/digital/compile.rs`'s
//! classification loop used to fold every non-`posedge`/`negedge` event
//! control into the plain combinational statement list — `comb` already
//! re-evaluates every pass, so `@(cross(V(x)-2.5,1))` ran on every step
//! instead of once per crossing (SPEC_DEVIATION.md finding 1.2, re-verified
//! reproducing at HEAD before this fix).
//!
//! The fix routes `cross`/`above` through the SAME watch-term/`fired`
//! mechanism a genuine clocked (`posedge`/`negedge`) block already uses
//! (`rewrite_analog_watch_events`, `compile.rs`) — proven here by counting
//! how many times a `$display`d body actually runs against a real transient
//! that ramps an analog node linearly through the crossing threshold.

use std::rc::Rc;

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::prelude::{BufferSink, Context, TransientAnalysisOptions};

/// `vramp`: an ideal, well-posed (voltage-source branch, never singular)
/// linear ramp `V(p,n) = slope · $abstime`. `dut`: a digital block
/// sensitive to an analog crossing event on that ramp.
const FIXTURE: &str = "
    discipline logic;
        domain discrete;
    enddiscipline

    module vramp ( inout electrical p, inout electrical n );
        parameter real slope = 5.0;
        analog V(p, n) <+ slope * $abstime;
    endmodule

    module dut ( input electrical x, input electrical gnd );
        always @(cross(V(x, gnd) - 2.5, 1))
            $display(\"crossed\");
    endmodule

    module top ();
        electrical x_net, gnd;
        vramp v1 ( x_net, gnd );
        dut d1 ( x_net, gnd );
    endmodule
";

/// A regression fixture with the SAME analog ramp but a plain level
/// comparison (`V(x,gnd) > 2.5`, NOT wrapped in `cross`/`above`) — must
/// keep its current (correct, pre-existing) combinational-fold behavior:
/// re-run on every digital evaluation pass tied to the analog ramp's
/// accepted timesteps, same as before this task. Only `cross`/`above`
/// CALLS are rewritten into watch terms; a bare comparison expression is
/// a distinct, still-open gap (LRMW2-12/T16 — `@(expr)` level events),
/// out of this task's scope.
const LEVEL_SENSITIVE_FIXTURE: &str = "
    discipline logic;
        domain discrete;
    enddiscipline

    module vramp ( inout electrical p, inout electrical n );
        parameter real slope = 5.0;
        analog V(p, n) <+ slope * $abstime;
    endmodule

    module dut ( input electrical x, input electrical gnd );
        always @(V(x, gnd) > 2.5)
            $display(\"level\");
    endmodule

    module top ();
        electrical x_net, gnd;
        vramp v1 ( x_net, gnd );
        dut d1 ( x_net, gnd );
    endmodule
";

fn run_and_capture(src: &str, stop: f64, dt: f64) -> String {
    let design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("fixture elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, _info) = compiler.build_circuit_mapped("top").expect("compiles");

    let sink = Rc::new(BufferSink::new());
    let mut ctx = Context::default();
    ctx.io = sink.clone();

    let mut tran = circuit.transient(TransientAnalysisOptions::new(stop, dt), ctx).unwrap();
    tran.solve().expect("transient run completes");
    sink.stdout()
}

/// AC5: the crossing-sensitive body fires once per crossing, not once per
/// timestep. `V(x,gnd)` ramps linearly from 0 to 5 V over 1 s (`slope = 5`),
/// crossing the 2.5 V threshold exactly once (rising, matching `cross(..,
/// 1)`'s direction argument) at `t = 0.5 s`. Over a run with ~200 digital
/// evaluation passes (`dt = 0.005`), the pre-fix "fold into comb" behavior
/// fires on essentially every pass once the ramp is near/above the
/// threshold — this bound (`1..=5`) is impossible for that bug to satisfy
/// but is exactly what "fires on the event" gives (the scheduler may
/// legitimately re-evaluate a couple of times around the exact crossing
/// step, same disclosed characteristic `vams_digital_events_in_analog_e2e.rs`
/// documents for other event-driven digital paths).
#[test]
fn crossing_sensitive_digital_block_fires_once_per_crossing_not_every_step() {
    let out = run_and_capture(FIXTURE, 1.0, 0.005);
    let count = out.matches("crossed\n").count();
    assert!(
        (1..=5).contains(&count),
        "must fire a SMALL, bounded number of times around the one crossing \
         at t=0.5s, not once per digital evaluation pass (~200 passes over \
         this run): got {count} firings. Captured stdout: {out:?}"
    );
}

/// Regression: a plain level-sensitive `@(sig)` digital control (no analog
/// event function) keeps folding into the combinational path — it must
/// still run on (essentially) every digital evaluation pass, exactly as
/// before this task. If this test's count collapsed to a handful too, the
/// fix would have wrongly widened its net beyond analog-event controls.
#[test]
fn plain_level_sensitive_control_still_runs_every_pass() {
    let out = run_and_capture(LEVEL_SENSITIVE_FIXTURE, 1.0, 0.005);
    let count = out.matches("level\n").count();
    assert!(
        count >= 50,
        "a bare comparison (not a `cross`/`above` call) must still be \
         re-evaluated every digital pass tied to the analog ramp's accepted \
         steps (unaffected regression) — got only {count} firings over \
         ~200 steps. Captured stdout: {out:?}"
    );
}
