//! LRM-COVERS: 5.10.3.1
//! LRMW2-33 (`lrm-conformance-wave2`, T41): `cross(expr, dir, time_tol)`'s
//! `time_tol` argument was silently discarded — §5.10.3.1: "In addition,
//! cross() controls the timestep to accurately resolve the crossing" and
//! "time_tol ... represent[s] the maximum allowable error between the true
//! crossing point and when the event triggers." Folded into the same
//! per-instance LTE tolerance mechanism T33/T39/T40 built for `ddt`/
//! `laplace_*`/`zi_*`'s own tolerance arguments (`kernel/analog/
//! compile.rs`'s `ddt_abstols`, now also draining `cross`/`above`'s
//! `time_tol`/`expr_tol`) — the LRM's own "absolute tolerance" framing for
//! both matches `ddt`'s `abstol` almost verbatim. Disclosed simplification:
//! `Element::bound_step_hint`/`$bound_step` (`piperine-solver/src/
//! analyses/events.rs`'s own `EventEntry::step_hint` doc comment) turned
//! out to be a dead lever — no solver call site anywhere in `crates/`
//! actually reads it (a first pass at this task wired `time_tol` into
//! `bound_step_hint` and got an *undiscriminating* test back — a tight and
//! a loose `time_tol` produced the identical step count — which is what
//! led to re-deriving this instead) — so this reuses the proven-live LTE
//! mechanism, not a per-crossing localized cap around the actual crossing
//! instant (the tier-2 `predict_crossing`/`refine_crossing` machinery in
//! `piperine-ir::events` exists but isn't wired into a live re-sampling
//! loop; that remains a separate, larger, out-of-scope feature).
//!
//! Fixture: mirrors `tests/laplace_abstol_lte.rs`'s shape (a `ddt` reactive
//! term is required for the LTE stepper to engage at all — a purely
//! resistive circuit has no charge history to compute a truncation error
//! from, so `time_tol` alone had nothing to bite into in a first draft of
//! this fixture) — the same sinusoid-through-a-near-zero-RC-filter circuit,
//! plus a `@(cross(...))` on the filtered node watching zero crossings with
//! a parameterized `time_tol`, and the `ddt` call itself left with no
//! explicit `abstol` of its own, so `time_tol` alone is what varies the LTE
//! tolerance between the two runs.

use piperine::{Session, SolverConfig};
use piperine_lang::SourceMap;

fn dut_source(time_tol: f64) -> String {
    format!(
        "
module Dut (inout electrical s, inout electrical p);
    parameter real time_tol = 1e-9;
    real count;
    analog begin
        V(s, gnd) <+ 1e-6 * sin(6.283185307179586 * 1e3 * $abstime);
        I(s, p) <+ V(s, p) / 1e-6;
        I(p, gnd) <+ V(p, gnd) / 1e6 + ddt(V(p, gnd));
        @(cross(V(p, gnd), 0, time_tol))
            count = count + 1.0;
    end
endmodule

module Top ();
    electrical s, p;
    Dut #(.time_tol({time_tol})) d1 (s, p);
endmodule
"
    )
}

fn steps_accepted(time_tol: f64) -> usize {
    let src = dut_source(time_tol);
    let design = piperine_lang::parse_and_elaborate(&src, &SourceMap::dummy()).expect("elaborates");
    let mut session = Session::compile(&design, "Top").expect("session compiles");
    let trace = session
        .tran(3e-3, None, 0.0, &SolverConfig::default(), None, false, &[])
        .expect("transient runs");
    trace.stats().steps_accepted
}

/// AC14 discrimination: a tight `time_tol` (1e-9) must force materially
/// more accepted steps than a loose one (1e-2) over the identical circuit
/// and stop time — an ignored `time_tol` argument cannot reproduce this.
#[test]
fn a_tighter_cross_time_tol_forces_more_accepted_steps() {
    let tight = steps_accepted(1e-9);
    let loose = steps_accepted(1e-2);
    assert!(
        tight > loose,
        "tight time_tol=1e-9 ({tight} steps) must accept more steps than loose time_tol=1e-2 ({loose} steps)"
    );
}
