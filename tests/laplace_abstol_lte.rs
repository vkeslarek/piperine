//! LRM-COVERS: 4.5.11, 4.5.14
//! LRMW2-35 (`lrm-conformance-wave2`, T39): `laplace_*(expr, num, den, ε)`'s
//! trailing `ε`/`abstol_expression` was silently discarded — the compiled
//! state carried no tolerance at all, and the LTE-driven adaptive timestep
//! (`AnalogInstance::suggest_transient_step`) always used the run's global
//! `Tolerances::abstol` regardless of what the model declared. §4.5.11.1-
//! 4.5.11.4 define it directly, summarized as `abstol` in §4.5.14's Table
//! 4-20 — `laplace_*`'s own trailing argument, the same
//! role `ddt`'s own second argument plays (§4.5.3) — folded through the
//! identical instance-level LTE tolerance mechanism T33 built for `ddt`
//! (`kernel/analog/compile.rs`'s `ddt_abstols`).
//!
//! Fixture: identical shape to `tests/ddt_abstol_lte.rs`, `ddt(V(p,gnd),
//! abstol)` swapped for `laplace_nd(V(p,gnd), '{0,1}, '{-1,0,1}, abstol)` —
//! `laplace_*` lowers `V(p,gnd)` through the same `ddt`-shaped
//! charge/resistive split (`flatten/analog.rs`'s `has_marker`/
//! `substitute_marker` treat `__laplace` identically to `__ddt`), so the
//! same amplitude/resistor values put the `ddt`-abstol-equivalent LTE term
//! back in charge of the accepted-step count.

use piperine::{Session, SolverConfig};
use piperine_lang::SourceMap;

fn dut_source(abstol: f64) -> String {
    format!(
        "
module Dut (inout electrical s, inout electrical p);
    parameter real abstol = 1e-9;
    analog begin
        V(s, gnd) <+ 1e-6 * sin(6.283185307179586 * 1e3 * $abstime);
        I(s, p) <+ V(s, p) / 1e-6;
        I(p, gnd) <+ V(p, gnd) / 1e6 + laplace_nd(V(p, gnd), '{{0,1}}, '{{-1,0,1}}, abstol);
    end
endmodule

module Top ();
    electrical s, p;
    Dut #(.abstol({abstol})) d1 (s, p);
endmodule
"
    )
}

fn steps_accepted(abstol: f64) -> usize {
    let src = dut_source(abstol);
    let design = piperine_lang::parse_and_elaborate(&src, &SourceMap::dummy()).expect("elaborates");
    let mut session = Session::compile(&design, "Top").expect("session compiles");
    let trace = session
        .tran(3e-3, None, 0.0, &SolverConfig::default(), None, false, &[])
        .expect("transient runs");
    trace.stats().steps_accepted
}

/// AC14 discrimination: a tight `ε` (1e-9) must force materially more
/// accepted steps than a loose one (1e-2) over the identical circuit and
/// stop time — an ignored `ε` argument cannot reproduce this (both runs
/// would silently share the run's global tolerance and land on the same
/// step count).
#[test]
fn a_tighter_laplace_abstol_forces_more_accepted_steps() {
    let tight = steps_accepted(1e-9);
    let loose = steps_accepted(1e-2);
    assert!(
        tight > loose,
        "tight abstol=1e-9 ({tight} steps) must accept more steps than loose abstol=1e-2 ({loose} steps)"
    );
}
