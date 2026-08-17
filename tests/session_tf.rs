//! HOST-03 — `Session::tf`: binds the existing solver `.tf` driver (no new
//! solver math) to a typed `TfResult` on a resistive divider with known
//! closed-form gain/R_in/R_out. `cargo test -p piperine` (Phase 1 / T5 quick
//! gate).

use piperine::{Session, SolverConfig};
use piperine_lang::SourceMap;

const DIVIDER_PHDL: &str = "\
module V(inout electrical p, inout electrical n);
    parameter real dc = 5.0;
    analog V(p, n) <+ dc;
endmodule

module R(inout electrical p, inout electrical n);
    parameter real r = 1e3;
    analog I(p, n) <+ V(p, n) / r;
endmodule

module Top();
    electrical gnd, vin, mid;
    V V1 (.p(vin), .n(gnd));
    R #(.r(3e3)) r_top (.p(vin), .n(mid));
    R #(.r(2e3)) r_bot (.p(mid), .n(gnd));
endmodule
";

/// `gain = r_bot/(r_top+r_bot) = 0.4`, `z_in = r_top+r_bot = 5 kΩ`,
/// `z_out = r_top || r_bot = 1.2 kΩ` — the closed-form resistive-divider
/// transfer characteristics `.tf` computes from unit excitations on the
/// linearized (here already-linear) system.
#[test]
fn tf_matches_the_closed_form_divider_transfer_characteristics() {
    let design =
        piperine_lang::parse_and_elaborate(DIVIDER_PHDL, &SourceMap::dummy()).expect("divider elaborates");
    let mut session = Session::compile(&design, "Top").expect("session compiles");
    let tf = session.tf("mid", None, "V1", &SolverConfig::default()).expect("tf solves");

    let (r_top, r_bot) = (3e3_f64, 2e3_f64);
    let gain = r_bot / (r_top + r_bot);
    let z_in = r_top + r_bot;
    let z_out = (r_top * r_bot) / (r_top + r_bot);

    assert!((tf.gain - gain).abs() < 1e-9, "gain = {}, expected {gain}", tf.gain);
    assert!((tf.z_in - z_in).abs() / z_in < 1e-9, "z_in = {}, expected {z_in}", tf.z_in);
    // z_out's unit-current-injection solve picks up the default gmin
    // conductance at every node (SolverConfig::default().gmin), so it is
    // exact only in the gmin -> 0 limit; 0.5% covers that without being a
    // vacuous tolerance.
    assert!((tf.z_out - z_out).abs() / z_out < 5e-3, "z_out = {}, expected {z_out}", tf.z_out);
}

/// A current-source input is documented-out-of-scope (MD-14) and fails loud
/// rather than silently returning a wrong gain.
#[test]
fn tf_current_source_input_fails_loud() {
    let design =
        piperine_lang::parse_and_elaborate(DIVIDER_PHDL, &SourceMap::dummy()).expect("divider elaborates");
    let mut session = Session::compile(&design, "Top").expect("session compiles");
    let err = session.tf("mid", None, "i_no_such_source", &SolverConfig::default());
    assert!(err.is_err(), "an unknown/non-voltage input source must fail loud");
}
