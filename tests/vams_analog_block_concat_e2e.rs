//! LRM-COVERS: 6.9.1
//!
//! T8 (LRMW2-04): LRM §6.9.1 "Concatenation of analog blocks" — a module
//! with more than one `analog` block behaves as if they were combined into
//! one, in source order. `crates/piperine-codegen/tests/analog_multiple_blocks.rs`
//! proves the operating-point-level fix (sum, order-independence, statement
//! order across blocks) directly against `AnalogKernel::eval_residual`; this
//! file proves the one piece that genuinely needs a real *transient* run and
//! the full host surface: two `analog` blocks in the SAME module, each
//! allocating its own `ddt` state slot, must not share state after
//! concatenation. If `resolve/pom/body.rs`'s merge collided the two blocks'
//! `StateId`s, one integrator's history would corrupt the other's and the
//! two nodes below would NOT track their independent closed-form ramps.

use std::collections::HashMap;

use piperine::{Session, SolverConfig};
use piperine_lang::SourceMap;

/// Two nodes, each driven by its own `analog` block: a constant current
/// `ia`/`ib` into a capacitor `ca`/`cb` on its own branch to `gnd`, plus a
/// huge leak resistor `rleak` (needed only so the branch has a resistive
/// term and the DC/transient Jacobian is non-singular — `rleak` is chosen
/// large enough that its current is ~9 orders of magnitude below `ia`/`ib`
/// at the voltages this test reaches, so it does not perturb the expected
/// values below). With nothing else attached to `a`/`b`, the branch's KCL
/// gives `ia + V(a,gnd)/rleak + ca·dV(a)/dt = 0`, i.e. (to excellent
/// approximation) the linear ramp `V(a,t) ≈ V(a,0) − (ia/ca)·t` — same for
/// `b` with its own `ib`/`cb`. `ia` and `ib` differ, so the two ramps have
/// different, independently verifiable slopes.
const TWO_CAPS_VAMS: &str = "\
module TwoCaps(inout electrical a, inout electrical b, inout electrical gnd);
    parameter real ia = 1.0e-3;
    parameter real ib = 2.0e-3;
    parameter real ca = 1.0e-9;
    parameter real cb = 1.0e-9;
    parameter real rleak = 1.0e9;
    analog begin
        I(a, gnd) <+ ia;
        I(a, gnd) <+ V(a, gnd) / rleak;
        I(a, gnd) <+ ca * ddt(V(a, gnd));
    end
    analog begin
        I(b, gnd) <+ ib;
        I(b, gnd) <+ V(b, gnd) / rleak;
        I(b, gnd) <+ cb * ddt(V(b, gnd));
    end
endmodule

module Top();
    electrical a, b, gnd;
    TwoCaps t1 (.a(a), .b(b), .gnd(gnd));
endmodule
";

fn design() -> piperine_lang::Design {
    piperine_lang::parse_and_elaborate(TWO_CAPS_VAMS, &SourceMap::dummy())
        .expect("TwoCaps fixture elaborates")
}

/// AC (T8 Done-when #2): two `analog` blocks each using `ddt` keep
/// independent state — proven as a transient assertion, not an elaboration
/// shape check. Each node's voltage at `t = 1 µs` must match ITS OWN
/// closed-form ramp; a state-id collision would corrupt at least one of the
/// two (equal, swapped, or garbled slopes), which the two distinct expected
/// values below would catch.
#[test]
fn two_ddt_blocks_keep_independent_integrator_state() {
    let mut session = Session::compile(&design(), "Top").expect("session compiles");
    let ic = HashMap::from([("a".to_string(), 0.0), ("b".to_string(), 0.0)]);
    let t_stop = 1.0e-6;
    let dt = t_stop / 200.0;
    let trace = session
        .tran(t_stop, Some(dt), 0.0, &SolverConfig::default(), Some(&ic), false, &[])
        .expect("transient solves");

    let v_a = trace.v("a").expect("v(a)").at(t_stop);
    let v_b = trace.v("b").expect("v(b)").at(t_stop);

    // ia = 1e-3, ca = 1e-9 -> slope = -1e6 V/s -> V(a, 1us) = -1.0 V
    let expected_a = -1.0;
    // ib = 2e-3, cb = 1e-9 -> slope = -2e6 V/s -> V(b, 1us) = -2.0 V
    let expected_b = -2.0;

    assert!(
        (v_a - expected_a).abs() < 0.02,
        "block 1's ddt state must track ITS OWN branch (ia/ca ramp): v(a, 1us) = {v_a}, expected ~{expected_a}"
    );
    assert!(
        (v_b - expected_b).abs() < 0.02,
        "block 2's ddt state must track ITS OWN branch (ib/cb ramp), independent of \
         block 1's: v(b, 1us) = {v_b}, expected ~{expected_b}"
    );
    assert!(
        (v_a - v_b).abs() > 0.5,
        "the two blocks' ramps must differ (ia != ib) — equal values would mean \
         the two `ddt` state slots collided and now share history: v(a)={v_a}, v(b)={v_b}"
    );
}
