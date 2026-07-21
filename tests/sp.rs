//! `.sp` host surface (SP-04, SP-06): `SimSession::run_sp` — a shunt-C
//! low-pass declared with `@rfport(num, z0)` ports, driven entirely through
//! `piperine::` (no bench crate, no interpreter).

use piperine::{SimSession, SolverConfig};
use piperine_lang::SourceMap;
use num_complex::Complex64;

/// `p1 --Rs-- p2`, shunt `C` from `p2` to gnd, plus a huge (1 GΩ) DC-only
/// bias resistor at each port node (real `.sp` fixtures always have *some*
/// DC continuity to ground; `.sp` itself adds only the `z0` termination at
/// analysis-time — see design.md's "Port primitive" section — so a purely
/// floating passive network has no operating point without one).
const SHUNT_C_LOWPASS_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1.0; }
analog R { I(p, n) <+ V(p, n) / r; }

mod C(inout p: Electrical, inout n: Electrical) { param c: Real = 1e-9; }
analog C { I(p, n) <+ c * ddt(V(p, n)); }

mod Top() {
    wire gnd : Electrical;
    @rfport(num = 1, z0 = 50) wire p1 : Electrical;
    @rfport(num = 2, z0 = 50) wire p2 : Electrical;
    rs  : R(.p = p1, .n = p2) { .r = 1.0 };
    c1  : C(.p = p2, .n = gnd) { .c = 1e-9 };
    rb1 : R(.p = p1, .n = gnd) { .r = 1e9 };
    rb2 : R(.p = p2, .n = gnd) { .r = 1e9 };
}
";

#[test]
fn shunt_c_lowpass_s21_matches_closed_form_rolloff() {
    let design = piperine_lang::parse_and_elaborate(SHUNT_C_LOWPASS_PHDL, &SourceMap::dummy())
        .expect("shunt-C low-pass elaborates");
    let session = SimSession::new(design, "Top".to_string());
    let result = session
        .run_sp(1e3, 1e9, 5, true, &SolverConfig::default())
        .expect(".sp solves on the shunt-C low-pass");

    assert_eq!(result.n_ports, 2);
    assert_eq!(result.frequencies.len(), 5);
    assert_eq!(result.z0, vec![50.0, 50.0]);

    // Closed form (ABCD -> S, series Rs then shunt Y=jωC, equal z0 both
    // ports): A=1+Rs*Y, B=Rs, C=Y, D=1;
    // S21 = 2 / (A + B/z0 + C*z0 + D).
    let (rs, cap, z0) = (1.0_f64, 1e-9_f64, 50.0_f64);
    for (k, &f) in result.frequencies.iter().enumerate() {
        let omega = 2.0 * std::f64::consts::PI * f;
        let y = Complex64::new(0.0, omega * cap);
        let a = Complex64::new(1.0, 0.0) + rs * y;
        let b = Complex64::new(rs, 0.0);
        let c = y;
        let d = Complex64::new(1.0, 0.0);
        let denom = a + b / z0 + c * z0 + d;
        let expected_s21 = Complex64::new(2.0, 0.0) / denom;
        let got_s21 = result.s[k][[1, 0]];
        assert!(
            (got_s21 - expected_s21).norm() < 1e-6,
            "S21 at f={f}: got {got_s21:?}, expected {expected_s21:?}"
        );
    }

    // Roll-off: |S21| must be non-increasing with frequency.
    let mags: Vec<f64> = result.s.iter().map(|s| s[[1, 0]].norm()).collect();
    for w in mags.windows(2) {
        assert!(w[1] <= w[0] + 1e-9, "|S21| should be non-increasing with frequency: {mags:?}");
    }
}

#[test]
fn no_rfport_declared_fails_loud() {
    let src = "\
discipline Electrical { potential v: Real; flow i: Real; }
mod Top() { wire gnd : Electrical; }
";
    let design = piperine_lang::parse_and_elaborate(src, &SourceMap::dummy()).expect("elaborates");
    let session = SimSession::new(design, "Top".to_string());
    let result = session.run_sp(1e3, 1e9, 5, true, &SolverConfig::default());
    assert!(result.is_err(), "a module with no @rfport ports must fail loud (SP-05)");
}
