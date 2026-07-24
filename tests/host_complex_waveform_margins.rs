//! host-library T18 (HOST-16): `ComplexWaveform` margins/bandwidth
//! (`bandwidth_3db`/`gain_margin`/`phase_margin`/`unity_gain_freq`).
//!
//! Fixture: a synthetic 3-pole loop-gain transfer function
//! `H(f) = A0 / [(1+jf/f1)(1+jf/f2)(1+jf/f3)]` — analytically known
//! magnitude `|H(f)| = A0 / sqrt(prod(1+(f/fi)^2))` and phase
//! `arg H(f) = -sum(atan(f/fi))`. A single dominant pole (`f1 << f2, f3`)
//! gives a well-defined -3dB corner near `f1`; three real poles give the
//! phase enough rolloff (asymptote -270°) to actually cross -180° at a
//! finite frequency, unlike a 1- or 2-pole system (asymptote -90°/-180°,
//! never reached) — needed so `gain_margin` has a real answer to check.
//!
//! Every expected value below is derived independently of the
//! implementation: closed-form magnitude/phase plus a bisection root-find
//! over those closed forms, not by reading `waveform.rs`.

use num_complex::Complex64;
use piperine::ComplexWaveform;

const A0: f64 = 1000.0; // 60 dB DC/low-freq gain
const F1: f64 = 100.0; // dominant pole
const F2: f64 = 1.0e6;
const F3: f64 = 1.0e7;

fn mag_analytic(f: f64) -> f64 {
    A0 / ((1.0 + (f / F1).powi(2)) * (1.0 + (f / F2).powi(2)) * (1.0 + (f / F3).powi(2))).sqrt()
}

fn phase_deg_analytic(f: f64) -> f64 {
    -((f / F1).atan() + (f / F2).atan() + (f / F3).atan()).to_degrees()
}

fn h(f: f64) -> Complex64 {
    let denom = Complex64::new(1.0, f / F1) * Complex64::new(1.0, f / F2) * Complex64::new(1.0, f / F3);
    Complex64::new(A0, 0.0) / denom
}

/// Bisect a monotonically decreasing `f(x)` for the root of `f(x) - target`
/// over `[lo, hi]` (a ground-truth root-finder independent of `Waveform`).
fn bisect_decreasing(f: impl Fn(f64) -> f64, target: f64, mut lo: f64, mut hi: f64) -> f64 {
    assert!(f(lo) > target && f(hi) < target, "bracket must straddle the root");
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if f(mid) > target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Bisect a monotonically increasing `f(x)` for the root of `f(x) - target`
/// over `[lo, hi]`.
fn bisect_increasing(f: impl Fn(f64) -> f64, target: f64, mut lo: f64, mut hi: f64) -> f64 {
    assert!(f(lo) < target && f(hi) > target, "bracket must straddle the root");
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if f(mid) < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Dense log-spaced `ComplexWaveform` fixture over the 3-pole `H(f)`.
fn fixture() -> ComplexWaveform {
    let (fstart, fstop, n) = (0.01_f64, 1.0e8_f64, 50_000usize);
    let ratio = (fstop / fstart).powf(1.0 / (n as f64 - 1.0));
    let points = (0..n)
        .map(|i| {
            let f = fstart * ratio.powi(i as i32);
            (f, h(f))
        })
        .collect();
    ComplexWaveform::new(points)
}

/// HOST-16 AC3: `bandwidth_3db` matches the analytic -3dB corner (bisected
/// over the closed-form magnitude, starting from the reference at the first
/// sample — `fstart = 0.01 Hz`, indistinguishable from DC given `f1=100Hz`).
#[test]
fn bandwidth_3db_matches_analytic_corner() {
    let trace = fixture();
    let ref_mag = mag_analytic(0.01);
    let target = ref_mag / std::f64::consts::SQRT_2;
    let expected = bisect_decreasing(mag_analytic, target, 0.01, 1.0e8);

    let bw = trace.bandwidth_3db().expect("bandwidth_3db solves");
    let rel_err = (bw - expected).abs() / expected;
    assert!(rel_err < 1e-3, "bandwidth_3db = {bw:.6e}, expected ≈ {expected:.6e} (rel {rel_err:.3e})");
}

/// HOST-16 AC3: `unity_gain_freq` matches the analytic `|H(f)| = 1`
/// crossing.
#[test]
fn unity_gain_freq_matches_analytic_crossing() {
    let trace = fixture();
    let expected = bisect_decreasing(mag_analytic, 1.0, 0.01, 1.0e8);

    let f_ug = trace.unity_gain_freq().expect("unity_gain_freq solves");
    let rel_err = (f_ug - expected).abs() / expected;
    assert!(rel_err < 1e-3, "unity_gain_freq = {f_ug:.6e}, expected ≈ {expected:.6e} (rel {rel_err:.3e})");
}

/// HOST-16 AC3: `phase_margin` matches `180° + phase(f_ug)` computed from
/// the closed-form phase at the analytic unity-gain frequency.
#[test]
fn phase_margin_matches_analytic_value() {
    let trace = fixture();
    let f_ug = bisect_decreasing(mag_analytic, 1.0, 0.01, 1.0e8);
    let expected = 180.0 + phase_deg_analytic(f_ug);

    let pm = trace.phase_margin().expect("phase_margin solves");
    assert!((pm - expected).abs() < 0.5, "phase_margin = {pm:.4}°, expected ≈ {expected:.4}°");
}

/// HOST-16 AC3: `gain_margin` matches `-20·log10(|H(f_180)|)` at the
/// analytic (unwrapped) phase = -180° crossing.
#[test]
fn gain_margin_matches_analytic_value() {
    let trace = fixture();
    // -phase_deg_analytic is monotonically increasing from 0 toward 270°.
    let f180 = bisect_increasing(|f| -phase_deg_analytic(f), 180.0, 0.01, 1.0e8);
    let expected = -20.0 * mag_analytic(f180).log10();

    let gm = trace.gain_margin().expect("gain_margin solves");
    assert!((gm - expected).abs() < 0.5, "gain_margin = {gm:.4} dB, expected ≈ {expected:.4} dB");
}

/// HOST-16 edge: `gain_margin` fails loud when the (unwrapped) phase never
/// reaches -180° — a single-pole rolloff's phase asymptotes to -90°.
#[test]
fn gain_margin_fails_loud_when_phase_never_reaches_180() {
    let points: Vec<(f64, Complex64)> = (0..1000)
        .map(|i| {
            let f = 1.0 * (1.0e6_f64 / 1.0).powf(i as f64 / 999.0);
            (f, Complex64::new(A0, 0.0) / Complex64::new(1.0, f / F1))
        })
        .collect();
    let trace = ComplexWaveform::new(points);
    let err = trace.gain_margin().expect_err("single-pole phase never reaches -180°");
    assert!(format!("{err}").contains("-180"));
}

/// HOST-16 edge: `unity_gain_freq` fails loud when the magnitude never
/// crosses 1 (a trace that stays well above 0dB throughout).
#[test]
fn unity_gain_freq_fails_loud_when_never_crossed() {
    let points: Vec<(f64, Complex64)> = vec![(1.0, Complex64::new(100.0, 0.0)), (2.0, Complex64::new(50.0, 0.0))];
    let trace = ComplexWaveform::new(points);
    let err = trace.unity_gain_freq().expect_err("magnitude never crosses 1");
    assert!(format!("{err}").contains("never crosses"));
}

/// HOST-16 edge: `bandwidth_3db` fails loud on a non-positive reference
/// magnitude (first sample is 0).
#[test]
fn bandwidth_3db_fails_loud_on_zero_reference() {
    let points: Vec<(f64, Complex64)> = vec![(1.0, Complex64::new(0.0, 0.0)), (2.0, Complex64::new(1.0, 0.0))];
    let trace = ComplexWaveform::new(points);
    let err = trace.bandwidth_3db().expect_err("zero reference magnitude must fail loud");
    assert!(format!("{err}").contains("positive"));
}
