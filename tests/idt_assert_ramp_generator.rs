//! LRM-COVERS: 4.5.4
//! LRMW2-29 (`lrm-conformance-wave2`, T34): `idt(expr, ic, assert)`'s
//! `assert` argument was silently discarded — the compiled integrator
//! never held at `ic` no matter what `assert` did. §4.5.4: "When specified
//! with both initial conditions and assert, idt() returns the initial
//! conditions during DC and IC analyses, and whenever assert is nonzero.
//! Once assert becomes zero, idt() returns the integral of the argument
//! starting from the last instant where assert was nonzero."
//!
//! Fixture: adapted from the LRM's own §4.5.4 "ramp_generator" worked
//! example (`V(out) <+ idt(1.0, 0, reset);`, `reset` toggled by
//! `@(timer(1, 1))`) — that literal form was tried first and dropped: an
//! `assert` argument that references a plain `integer` var written inside
//! an `@(timer(...))` event-action block resolves, at the `idt` call site
//! (a *later* statement, outside the event's own body), to the
//! unconditional assignment that precedes the event action in program
//! order — a pre-existing var-scoping gap in how an event-conditioned
//! write threads into a later expression's temp-tape substitution,
//! confirmed independent of this task (the *persistent* var bank read
//! elsewhere in the same body DOES see the event's write; only the
//! value captured for the marker's `assert` argument does not). Not this
//! task's scope to fix, so the fixture instead spells `assert` as a
//! direct comparison, `($abstime > assert_after)` — no event/var
//! indirection, still a plain `analog_expression` per §4.5.4's grammar,
//! and exercises exactly the same `assert`-carrying code path
//! (`resolve/pom/analog_ops.rs::Idt::lower`, `kernel/analog/compile.rs`'s
//! `assert_inputs` row, `AnalogInstance::accept_timestep`).

use piperine::{Session, SolverConfig};
use piperine_lang::SourceMap;

fn dut_source(assert_after: f64) -> String {
    format!(
        "
module RampGenerator (inout electrical out);
    parameter real assert_after = 1.0;
    analog begin
        V(out) <+ idt(1.0, 0, ($abstime > assert_after));
    end
endmodule

module Top ();
    electrical out;
    RampGenerator #(.assert_after({assert_after})) d1 (out);
endmodule
"
    )
}

fn v_out_at(assert_after: f64, t: f64) -> f64 {
    let src = dut_source(assert_after);
    let design = piperine_lang::parse_and_elaborate(&src, &SourceMap::dummy()).expect("elaborates");
    let mut session = Session::compile(&design, "Top").expect("session compiles");
    let trace = session
        .tran(3.0, Some(0.01), 0.0, &SolverConfig::default(), None, false, &[])
        .expect("transient runs");
    trace.v("out").expect("v(out)").at(t)
}

/// Before `assert_after`, both fixtures ramp identically from `ic=0` at
/// rate 1.0 — the baselines agree wherever `assert` hasn't fired yet.
#[test]
fn before_assert_fires_both_fixtures_agree() {
    let asserted = v_out_at(1.0, 0.5);
    let never_asserted = v_out_at(1e9, 0.5);
    assert!((asserted - 0.5).abs() < 0.05, "expected ~0.5V at t=0.5s, got {asserted}");
    assert!((never_asserted - 0.5).abs() < 0.05, "expected ~0.5V at t=0.5s, got {never_asserted}");
}

/// AC14 discrimination: past `assert_after=1.0`, `assert` stays nonzero
/// for the rest of the run — the integrator must hold at `ic=0` (§4.5.4)
/// — while a fixture whose `assert_after` never falls inside the run
/// window keeps integrating unbounded. An ignored `assert` argument
/// cannot reproduce this divergence: both would climb to the same value.
#[test]
fn past_assert_after_the_integrator_holds_while_the_baseline_keeps_climbing() {
    let asserted = v_out_at(1.0, 2.5);
    let never_asserted = v_out_at(1e9, 2.5);
    assert!(asserted.abs() < 0.05, "asserted idt must hold at ic=0 past assert_after, got {asserted}");
    assert!(
        (never_asserted - 2.5).abs() < 0.1,
        "unasserted idt must keep integrating to ~2.5V, got {never_asserted}"
    );
    assert!(
        (never_asserted - asserted).abs() > 1.0,
        "assert must make a material difference: asserted={asserted} never_asserted={never_asserted}"
    );
}
