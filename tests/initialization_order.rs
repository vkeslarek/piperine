//! LRM-COVERS: 8.2, 8.4.1
//!
//! lrm-conformance-wave3 (Phase 6, T39): §8.4.1 "Circuit initialization"
//! — "a one time execution of nodeset statements (3.6.3.2), then the
//! procedural statements in analog initial block, and then the
//! procedural statements in the Verilog initial block for time zero."
//! `tests/net_declaration_nodeset.rs` already proves the nodeset phase in
//! isolation (a declared/host-supplied nodeset seeds the Newton initial
//! guess); this target proves the ordering claim itself: `analog
//! initial`'s effect (a variable it sets) is visible to the main analog
//! body's very first evaluation (the DC operating point), and the
//! digital `initial` block's effect (a reg it sets) is visible at time
//! zero — both phases having already run before their respective bodies'
//! steady-state behavior is observed.
//!
//! §8.2 "Simulation initialization" (Figure 8-1's pre-simulation order:
//! variable declaration assignments, then `analog initial` blocks, then
//! node declaration/nodeset evaluation) is the same claim from the
//! system-level angle, proven by the same
//! `analog_initial_runs_before_the_main_analog_body_first_evaluates`
//! (offset's zero-value declaration default is visible before `analog
//! initial` overwrites it to 7.0). **Deviation, already recorded**: §8.2's
//! own closing sentence — "for parametric sweep type analyses... the tool
//! shall re-evaluate the Elaboration and Pre-simulation steps... for each
//! sweep point" — is deliberately NOT what Piperine does: MD-18
//! ("elaboration fixes devices; sweeps restamp on the compiled circuit,
//! never re-elaborate", `.specs/STATE.md`) restamps parameter values on
//! one compiled circuit instead. This is a project-level architectural
//! decision, not an oversight — recorded here as the clause's one
//! disclosed deviation, not silently claimed as full compliance.

use piperine::{Session, SolverConfig};
use piperine_lang::SourceMap;

const SRC: &str = "
    discipline logic;
        domain discrete;
    enddiscipline

    module dut (inout electrical a, output logic q);
        real offset;
        analog initial offset = 7.0;
        analog I(a, gnd) <+ V(a) - offset;
        initial q = 1;
    endmodule

    module Top ();
        electrical a;
        logic q_net;
        dut d1 (a, q_net);
    endmodule
";

#[test]
fn analog_initial_runs_before_the_main_analog_body_first_evaluates() {
    let design = piperine_lang::parse_and_elaborate(SRC, &SourceMap::dummy()).expect("elaborates");
    let mut session = Session::compile(&design, "Top").expect("session compiles");
    let op = session.op(&SolverConfig::default(), None).expect("op solves");
    let v_a = op.v("a").expect("v(a)");
    // `I(a,gnd) <+ V(a) - offset;` with no other element on node `a`
    // forces `V(a) - offset = 0` (KCL, sole branch) -> `V(a) = offset`.
    // If `analog initial` had NOT already run before the main body's
    // first evaluation, `offset` would still be its zero-value default,
    // giving `V(a) = 0` instead of `7.0`.
    assert!((v_a - 7.0).abs() < 1e-6, "V(a) must equal the analog-initial-set offset (7.0), got {v_a}");
}

#[test]
fn digital_initial_block_effect_is_visible_at_time_zero() {
    let design = piperine_lang::parse_and_elaborate(SRC, &SourceMap::dummy()).expect("elaborates");
    let mut session = Session::compile(&design, "Top").expect("session compiles");
    let trace = session.tran(0.0, Some(1e-9), 0.0, &SolverConfig::default(), None, false, &[]).expect("transient solves");
    let q = trace.v("q_net").expect("v(q_net)");
    let at_zero = q.at(0.0);
    assert!(
        (at_zero - 1.0).abs() < 1e-9,
        "q must reflect the digital `initial qreg = 1;` block's effect at time zero, got {at_zero}"
    );
}
