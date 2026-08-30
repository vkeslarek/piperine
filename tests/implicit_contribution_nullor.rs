//! LRM-COVERS: 5.6.6
//!
//! lrm-conformance-wave3 (Phase 6, T36): §5.6.6 "Implicit Contributions" —
//! "the value of the target may be expressed in terms of itself... The
//! underlying implementation of the simulator will find the value of
//! [the target] that equals the sum of the contributions made to it, even
//! if the contributions are a function of [the target] itself." The
//! section's own example is `I(diode) <+ is*(limexp((V(diode) -
//! r*I(diode))/$vt) - 1);`; §5.6.7 (introducing indirect contributions)
//! reuses the identical self-referential shape for `V(out) <+ V(out) +
//! V(in);` (the ideal-opamp/nullor constitutive equation, "subtracting
//! V(out) from both sides" is the LRM's own algebra reducing it to
//! `V(in) = 0`) — this target uses that second form because a potential
//! contribution's implicit self-reference resolves through the same
//! auxiliary-force-branch machinery every ideal voltage source already
//! uses (`crates/piperine-codegen/src/device/analog/forces.rs`), making
//! the expected numeric answer independently derivable (a textbook
//! inverting amplifier), unlike the flow-only diode form whose fixed
//! point depends on exactly how many other branches share the node.
//!
//! Circuit: `Vsrc(0.3V) -> Rin(1k) -> in`, `Opamp(in, out)` implementing
//! `V(out) <+ V(out) + V(in);`, `Rf(2k)` feeding back from `out` to
//! `in`. If the self-referential contribution were resolved wrong (e.g.
//! `V(out)` on the RHS silently read as `0.0` instead of the same
//! auxiliary unknown the LHS constrains), the circuit would not reduce to
//! `V(in) = 0` and the classic `V(out) = -(Rf/Rin)*Vin` inverting-gain
//! relation would not hold.

use piperine::{Session, SolverConfig};
use piperine_lang::SourceMap;

const SRC: &str = "
module R (inout electrical p, inout electrical n);
    parameter real r = 1.0e3;
    analog I(p, n) <+ V(p, n) / r;
endmodule

module Vsrc (inout electrical p, inout electrical n);
    parameter real dc = 1.0;
    analog V(p, n) <+ dc;
endmodule

module Opamp (inout electrical in, inout electrical out);
    analog V(out) <+ V(out) + V(in);
endmodule

module Top ();
    electrical vin_node, in, out;
    Vsrc vsrc (.p(vin_node), .n(gnd));
    R rin (.p(vin_node), .n(in));
    R rf (.p(out), .n(in));
    Opamp u1 (.in(in), .out(out));
endmodule
";

#[test]
fn self_referential_potential_contribution_enforces_the_nullor_constraint() {
    let design = piperine_lang::parse_and_elaborate(SRC, &SourceMap::dummy()).expect("elaborates");
    let mut session = Session::builder(&design, "Top")
        .stage("vsrc", "dc", piperine_lang::Value::Real(0.3))
        .stage("rin", "r", piperine_lang::Value::Real(1e3))
        .stage("rf", "r", piperine_lang::Value::Real(2e3))
        .compile()
        .expect("session compiles");
    let op = session.op(&SolverConfig::default(), None).expect("op solves");
    let v_in = op.v("in").expect("v(in)");
    let v_out = op.v("out").expect("v(out)");

    // §5.6.6/§5.6.7's own algebra: subtracting V(out) from both sides of
    // `V(out) <+ V(out) + V(in);` leaves `V(in) = 0` — the "virtual
    // ground" node of an inverting amplifier.
    assert!(v_in.abs() < 1e-6, "the self-referential contribution must force V(in) = 0 (LRM's own reduction), got {v_in}");

    // The classic inverting-amplifier gain, independently derivable from
    // KCL at `in` once V(in) = 0: Vin/Rin = -Vout/Rf.
    let expected_out = -(2.0e3 / 1.0e3) * 0.3;
    assert!(
        (v_out - expected_out).abs() < 1e-6,
        "V(out) must equal -(Rf/Rin)*Vin = {expected_out} once the self-referential constraint holds, got {v_out}"
    );
}
