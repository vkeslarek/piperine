//! `.pz` host surface (PZ-07): `SimSession::run_pz` — an RC low-pass's
//! single real pole and a series RLC's complex-conjugate pole pair, driven
//! entirely through `piperine::` (no bench crate, no interpreter).

use piperine::{SimSession, SolverConfig};
use piperine_lang::SourceMap;

const RC_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod V(inout p: Electrical, inout n: Electrical) { param dc: Real = 1.0; }
analog V { V(p, n) <- dc; }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog R { I(p, n) <+ V(p, n) / r; }

mod C(inout p: Electrical, inout n: Electrical) { param c: Real = 1e-6; }
analog C { I(p, n) <+ c * ddt(V(p, n)); }

mod Top() {
    wire gnd  : Electrical;
    wire vin  : Electrical;
    wire vout : Electrical;
    v1 : V(.p = vin, .n = gnd) {};
    r1 : R(.p = vin, .n = vout) { .r = 1e3 };
    c1 : C(.p = vout, .n = gnd) { .c = 1e-6 };
}
";

const RLC_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod V(inout p: Electrical, inout n: Electrical) { param dc: Real = 1.0; }
analog V { V(p, n) <- dc; }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog R { I(p, n) <+ V(p, n) / r; }

mod L(inout p: Electrical, inout n: Electrical) { param l: Real = 1e-3; }
analog L { V(p, n) <- l * ddt(I(p, n)); }

mod C(inout p: Electrical, inout n: Electrical) { param c: Real = 1e-6; }
analog C { I(p, n) <+ c * ddt(V(p, n)); }

mod Top() {
    wire gnd : Electrical;
    wire vin : Electrical;
    wire a   : Electrical;
    wire b   : Electrical;
    v1 : V(.p = vin, .n = gnd) {};
    r1 : R(.p = vin, .n = a) { .r = 10.0 };
    l1 : L(.p = a, .n = b) { .l = 1e-3 };
    c1 : C(.p = b, .n = gnd) { .c = 1e-6 };
}
";

#[test]
fn rc_low_pass_has_one_real_pole_at_minus_one_over_rc() {
    let design = piperine_lang::parse_and_elaborate(RC_PHDL, &SourceMap::dummy()).expect("RC elaborates");
    let session = SimSession::new(design, "Top".to_string());
    let result = session
        .run_pz("v1", "vout", None, &SolverConfig::default())
        .expect(".pz solves on the RC low-pass");

    assert_eq!(result.poles.len(), 1, "{:?}", result.poles);
    assert!(result.zeros.is_empty(), "RC low-pass has no finite zero: {:?}", result.zeros);

    let expected = -1.0 / (1e3 * 1e-6);
    let pole = result.poles[0];
    assert_eq!(pole.im, 0.0, "pole should be real: {pole:?}");
    assert!((pole.re - expected).abs() / expected.abs() < 1e-6, "pole = {pole:?}, expected {expected}");
}

#[test]
fn series_rlc_has_the_analytic_complex_conjugate_pole_pair() {
    let design = piperine_lang::parse_and_elaborate(RLC_PHDL, &SourceMap::dummy()).expect("RLC elaborates");
    let session = SimSession::new(design, "Top".to_string());
    let result = session
        .run_pz("v1", "b", None, &SolverConfig::default())
        .expect(".pz solves on the series RLC");

    assert_eq!(result.poles.len(), 2, "{:?}", result.poles);

    let (r, l, c) = (10.0_f64, 1e-3_f64, 1e-6_f64);
    let sigma = -r / (2.0 * l);
    let omega_d = (1.0 / (l * c) - (r / (2.0 * l)).powi(2)).sqrt();

    let mut poles = result.poles.clone();
    poles.sort_by(|a, b| a.im.partial_cmp(&b.im).unwrap());
    let (p_minus, p_plus) = (poles[0], poles[1]);
    assert!(p_minus.im < 0.0 && p_plus.im > 0.0, "expected a conjugate pair: {poles:?}");
    assert!((p_minus.re - sigma).abs() / sigma.abs() < 1e-6, "Re = {}, expected {sigma}", p_minus.re);
    assert!((p_plus.re - sigma).abs() / sigma.abs() < 1e-6, "Re = {}, expected {sigma}", p_plus.re);
    assert!((p_plus.im - omega_d).abs() / omega_d < 1e-6, "Im = {}, expected {omega_d}", p_plus.im);
    assert!((p_minus.im + omega_d).abs() / omega_d < 1e-6, "conjugate mismatch: {poles:?}");
}

#[test]
fn unknown_input_source_fails_loud() {
    let design = piperine_lang::parse_and_elaborate(RC_PHDL, &SourceMap::dummy()).expect("RC elaborates");
    let session = SimSession::new(design, "Top".to_string());
    let result = session.run_pz("no_such_source", "vout", None, &SolverConfig::default());
    assert!(result.is_err(), "an unknown input source must fail loud");
}
