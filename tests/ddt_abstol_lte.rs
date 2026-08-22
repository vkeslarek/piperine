//! LRM-COVERS: 4.5.3
//! LRMW2-37 (`lrm-conformance-wave2`, T33): `ddt(expr, abstol)`'s second
//! argument was silently discarded — the compiled state carried no
//! tolerance at all, and the LTE-driven adaptive timestep
//! (`AnalogInstance::suggest_transient_step`) always used the run's global
//! `Tolerances::abstol` (1e-12) regardless of what the model asked for.
//! §4.5.3: "The absolute tolerance, abstol or derived from nature, applies
//! to the output of the ddt operator and is the largest signal level that
//! is considered negligible" — a *tight* `abstol` must force the stepper
//! to take smaller strides (more accepted steps over a fixed-length run)
//! than a *loose* one, for the same physical circuit.
//!
//! Fixture: a small sinusoid (`1e-6`V, 1kHz) drives node `p` through a
//! near-zero resistor (negligible RC filtering) with a large discharge
//! resistor to ground (DC uniqueness for the otherwise-floating
//! "capacitor" `ddt(V(p,gnd))`). At this amplitude the LTE tolerance
//! formula's `reltol*|q|` term (`device/analog/instance.rs::
//! suggest_transient_step`) is small enough that a `ddt`-declared `abstol`
//! in the `1e-9..1e-2` range is what actually decides the accepted-step
//! count (verified empirically: identical step counts before the fix,
//! `git stash` reproduces that).

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
        I(p, gnd) <+ V(p, gnd) / 1e6 + ddt(V(p, gnd), abstol);
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

/// AC14 discrimination: a tight `abstol` (1e-9) must force materially more
/// accepted steps than a loose one (1e-2) over the identical circuit and
/// stop time — an ignored `abstol` argument cannot reproduce this (both
/// runs would silently share the run's global 1e-12 tolerance and land on
/// the same step count).
#[test]
fn a_tighter_ddt_abstol_forces_more_accepted_steps() {
    let tight = steps_accepted(1e-9);
    let loose = steps_accepted(1e-2);
    assert!(
        tight > loose,
        "tight abstol=1e-9 ({tight} steps) must accept more steps than loose abstol=1e-2 ({loose} steps)"
    );
}
