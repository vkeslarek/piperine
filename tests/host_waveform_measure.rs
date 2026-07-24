//! host-library T16 (HOST-14): real-`Waveform` measurements
//! (`slew_rate`/`rise_time`/`fall_time`/`overshoot`/`settling_time`/`delay`).
//!
//! Fixture: a first-order RC charge (`τ = R·C = 1 ms`) driven by a 5 V step.
//! The first-order step response `v(t) = V·(1 − exp(−t/τ))` is analytic, so
//! every measurement has a spec-defined expected value:
//! - `rise_time` = `τ·(ln(10) − ln(10/9))` = `τ·ln(9)` ≈ `2.197·τ` ≈ `2.20 ms`;
//! - `slew_rate` = `0.8·V / rise_time` ≈ `1820 V/s`;
//! - `overshoot` = `0` (first-order → monotonic, no overshoot);
//! - `settling_time(0.25)` (`5%` of `V=5`) = `τ·ln(1/0.05)` ≈ `3.00 ms`;
//! - `delay(vin, vout, 2.5)` = `τ·ln(2)` ≈ `0.693 ms` (`vout` reaches `V/2`).
//!
//! Fail-loud edge cases: a flat signal rejects every step measurement with a
//! clear `Measurement` error; `delay` rejects a level neither waveform crosses.

use std::collections::HashMap;

use piperine::{NetRef, Session, SolverConfig};
use piperine_lang::SourceMap;

const RC_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod V(inout p: Electrical, inout n: Electrical) { param dc: Real = 0.0; }
analog V { V(p, n) <- dc; }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog R { I(p, n) <+ V(p, n) / r; }

mod C(inout p: Electrical, inout n: Electrical) { param c: Real = 1e-6; }
analog C { I(p, n) <+ c * ddt(V(p, n)); }

mod Top() {
    wire gnd : Electrical;
    wire vin : Electrical;
    wire out : Electrical;
    v1 : V(.p = vin, .n = gnd) { .dc = 5.0 };
    r1 : R(.p = vin, .n = out) { .r = 1e3 };
    c1 : C(.p = out, .n = gnd) { .c = 1e-6 };
}
";

fn design() -> piperine_lang::Design {
    piperine_lang::parse_and_elaborate(RC_PHDL, &SourceMap::dummy()).expect("RC fixture elaborates")
}

/// The RC step-response `Waveform` over `v(out)`, sampled over `10·τ` so the
/// signal fully settles. `ic = {out: 0}` starts the capacitor uncharged (the
/// DC operating point would otherwise leave it at 5 V from t=0 — no step).
/// The constant `dt` keeps the analysis grid uniform (the analytic expected
/// values above are unaffected by the sampler).
fn rc_step_response() -> piperine::Waveform {
    let mut session = Session::compile(&design(), "Top").expect("session compiles");
    let tau = 1e-3;
    let ic = HashMap::from([("out".to_string(), 0.0)]);
    let trace = session
        .tran(10.0 * tau, Some(tau / 50.0), 0.0, &SolverConfig::default(), Some(&ic), false, &[])
        .expect("tran solves");
    let out = NetRef { name: "out".into() };
    trace.v(&out, None).expect("v(out)")
}

/// HOST-14 AC1: `rise_time` on the RC step matches the analytic
/// `τ·ln(9)` first-order value within the integration tolerance.
#[test]
fn rise_time_matches_first_order_rc_analytic_value() {
    let v_out = rc_step_response();
    let rt = v_out.rise_time().expect("rise_time solves");
    let expected = 1e-3 * (9.0f64).ln();
    let rel_err = (rt - expected).abs() / expected;
    assert!(rel_err < 0.02, "rise_time = {rt:.6e}s, expected ≈ {expected:.6e}s (rel {rel_err:.3e})");
}

/// HOST-14 AC1: `slew_rate` is positive and matches `0.8·V / rise_time`
/// within the integration tolerance on the settled value.
#[test]
fn slew_rate_matches_v_over_rise_time() {
    let v_out = rc_step_response();
    let sr = v_out.slew_rate().expect("slew_rate solves");
    assert!(sr > 0.0, "rising step → positive slew rate, got {sr}");
    let rt = v_out.rise_time().expect("rise_time solves");
    let expected = 0.8 * 5.0 / rt;
    let rel_err = (sr - expected).abs() / expected;
    assert!(rel_err < 1e-3, "slew_rate = {sr:.6e} V/s, expected {expected:.6e} (rel {rel_err:.3e})");
}

/// HOST-14 AC1: `overshoot` is ≈ 0 for the first-order RC step (monotonic).
#[test]
fn overshoot_is_zero_for_first_order_rc() {
    let v_out = rc_step_response();
    let os = v_out.overshoot().expect("overshoot solves");
    assert!(os.abs() < 1e-3, "first-order RC is monotonic → overshoot ≈ 0, got {os}");
}

/// HOST-14 AC1: `settling_time(0.25)` (5% of the 5 V swing) matches the
/// analytic `τ·ln(1/0.05)` ≈ `3.00 ms`.
#[test]
fn settling_time_matches_first_order_rc_analytic_value() {
    let v_out = rc_step_response();
    let st = v_out.settling_time(0.25).expect("settling_time solves");
    let expected = 1e-3 * (1.0 / 0.05f64).ln();
    let rel_err = (st - expected).abs() / expected;
    assert!(rel_err < 0.02, "settling_time = {st:.6e}s, expected ≈ {expected:.6e}s (rel {rel_err:.3e})");
}

/// HOST-14 AC1: `delay(input → output, 2.5)` matches the analytic
/// `τ·ln(2)` (when the RC output reaches `V/2`). `self.delay(other)` returns
/// `t_other − t_self`, so `input.delay(output)` is the positive propagation
/// delay. The input is a synthetic edge at t=0 (the source is ideal and held
/// at 5 V — not a transition itself), so a unit step Waveform stands in for
/// the drive edge.
#[test]
fn delay_matches_v_over_two_crossing() {
    let v_out = rc_step_response();
    let v_in = piperine::Waveform::new(vec![(0.0, 0.0), (1e-9, 5.0), (1e-2, 5.0)]);
    let d = v_in.delay(&v_out, 2.5).expect("delay solves");
    let expected = 1e-3 * (2.0f64).ln();
    let rel_err = (d - expected).abs() / expected;
    assert!(rel_err < 0.02, "delay = {d:.6e}s, expected ≈ {expected:.6e}s (rel {rel_err:.3e})");
}

/// HOST-14 AC1 / edge: `fall_time` on a *rising* signal fails loud with a
/// clear pointer at `rise_time` — the user picked the wrong measurement, not
/// a malformed waveform.
#[test]
fn fall_time_rejects_a_rising_step() {
    let v_out = rc_step_response();
    let err = v_out.fall_time().expect_err("rising step must reject fall_time");
    let msg = format!("{err}");
    assert!(msg.contains("fall_time") && msg.contains("rise_time"), "got: {msg}");
}

/// HOST-14 edge: a flat signal (initial == settled) fails loud on every
/// *step* measurement — never a silent `0.0` / `NaN`. (`settling_time` is
/// excluded: a flat signal is trivially settled at the first sample, which
/// is a legitimate `Ok` answer, not a failure.)
#[test]
fn flat_signal_fails_loud_on_every_step_measurement() {
    let flat = piperine::Waveform::new(vec![(0.0, 1.0), (0.5, 1.0), (1.0, 1.0)]);
    for name in ["slew_rate", "rise_time", "fall_time", "overshoot"] {
        let result = match name {
            "slew_rate" => flat.slew_rate(),
            "rise_time" => flat.rise_time(),
            "fall_time" => flat.fall_time(),
            "overshoot" => flat.overshoot(),
            _ => unreachable!(),
        };
        let err = result.expect_err("flat signal must fail {name} loud");
        let msg = format!("{err}");
        assert!(
            msg.to_lowercase().contains("flat"),
            "{name}: error must say 'flat', got: {msg}"
        );
    }
}

/// HOST-14 edge: `delay` fails loud when neither waveform crosses `level`.
#[test]
fn delay_fails_loud_when_level_is_never_crossed() {
    let v_out = rc_step_response();
    let err = v_out.delay(&v_out, 100.0).expect_err("level out of range must fail loud");
    let msg = format!("{err}");
    assert!(msg.contains("never crosses"), "got: {msg}");
}

/// HOST-14 edge: `settling_time` with `tol < 0` fails loud (bad argument).
#[test]
fn settling_time_rejects_negative_tolerance() {
    let v_out = rc_step_response();
    let err = v_out.settling_time(-1.0).expect_err("negative tol must fail loud");
    assert!(format!("{err}").contains("non-negative"), "got: {err}");
}
