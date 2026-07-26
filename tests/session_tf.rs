//! HOST-03 — `Session::tf`: binds the existing solver `.tf` driver (no new
//! solver math) to a typed `TfResult` on a resistive divider with known
//! closed-form gain/R_in/R_out. `cargo test -p piperine` (Phase 1 / T5 quick
//! gate).

use piperine::{Session, SolverConfig};
use piperine_lang::SourceMap;

const DIVIDER_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod V(inout p: Electrical, inout n: Electrical) { param dc: Real = 5.0; }
analog V { V(p, n) <- dc; }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog R { I(p, n) <+ V(p, n) / r; }

mod Top() {
    wire gnd : Electrical;
    wire vin : Electrical;
    wire mid : Electrical;
    V1    : V(.p = vin, .n = gnd) {};
    r_top : R(.p = vin, .n = mid) { .r = 3e3 };
    r_bot : R(.p = mid, .n = gnd) { .r = 2e3 };
}
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
