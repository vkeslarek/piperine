//! LRM-COVERS: 4.5.12, 4.5.14
//! LRMW2-32 (`lrm-conformance-wave2`, T40): `zi_*(expr, num, den, T, τ)`'s
//! `τ` argument was silently discarded — §4.5.12: "If the transition time
//! is specified and is non-zero, the timestep is controlled to accurately
//! resolve both the leading and trailing corner of the transition." Folded
//! through the identical instance-level LTE tolerance mechanism T33/T39
//! built for `ddt`/`laplace_*`'s own tolerance arguments
//! (`kernel/analog/compile.rs`'s `ddt_abstols`, now also draining
//! `StateKind::ZTransform`'s `tau`).
//!
//! Fixture: identical shape to `tests/laplace_abstol_lte.rs` — `laplace_nd`
//! swapped for `zi_nd(V(p,gnd), '{0,1}, '{-1,0,1}, 1e-6, tau)` (`zi_*`
//! lowers `V(p,gnd)` through the same `ddt`-shaped charge/resistive split
//! `laplace_*` does).

use piperine::{Session, SolverConfig};
use piperine_lang::SourceMap;

fn dut_source(tau: f64) -> String {
    format!(
        "
module Dut (inout electrical s, inout electrical p);
    parameter real tau = 1e-9;
    analog begin
        V(s, gnd) <+ 1e-6 * sin(6.283185307179586 * 1e3 * $abstime);
        I(s, p) <+ V(s, p) / 1e-6;
        I(p, gnd) <+ V(p, gnd) / 1e6 + zi_nd(V(p, gnd), '{{0,1}}, '{{-1,0,1}}, 1e-6, tau);
    end
endmodule

module Top ();
    electrical s, p;
    Dut #(.tau({tau})) d1 (s, p);
endmodule
"
    )
}

fn steps_accepted(tau: f64) -> usize {
    let src = dut_source(tau);
    let design = piperine_lang::parse_and_elaborate(&src, &SourceMap::dummy()).expect("elaborates");
    let mut session = Session::compile(&design, "Top").expect("session compiles");
    let trace = session
        .tran(3e-3, None, 0.0, &SolverConfig::default(), None, false, &[])
        .expect("transient runs");
    trace.stats().steps_accepted
}

/// AC14 discrimination: a tight `τ` (1e-9) must force materially more
/// accepted steps than a loose one (1e-2) over the identical circuit and
/// stop time — an ignored `τ` argument cannot reproduce this.
#[test]
fn a_tighter_zi_tau_forces_more_accepted_steps() {
    let tight = steps_accepted(1e-9);
    let loose = steps_accepted(1e-2);
    assert!(
        tight > loose,
        "tight tau=1e-9 ({tight} steps) must accept more steps than loose tau=1e-2 ({loose} steps)"
    );
}
