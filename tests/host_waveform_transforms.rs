//! host-library T17 (HOST-15): `Waveform` transforms
//! (`fft`/`resample`/`derivative`/`integral`/`clip`) → new `Waveform`.
//!
//! All fixtures are synthetic (`Waveform::new`, no simulation) since these
//! are pure axis-domain transforms — spec-defined analytic outcomes:
//! - `resample(grid)`: linear interpolation at each grid point, same
//!   contract as `Waveform::at` (already spec'd/tested for HOST-14).
//! - `derivative`: exact for a linear signal (central difference is exact
//!   on a linear function).
//! - `integral`: exact for a constant signal (`∫c dt = c·t`).
//! - `clip(lo, hi)`: values outside `[lo, hi]` saturate at the nearer bound.
//! - `fft`: a single-tone sine sampled on an integer number of periods
//!   round-trips — the bin nearest the tone frequency carries magnitude
//!   `amplitude/2` (energy split with its Nyquist mirror), all other bins
//!   ≈ 0.

use piperine::Waveform;

/// HOST-15 AC2: `resample(grid)` linearly interpolates at each grid point —
/// same values `Waveform::at` returns pointwise.
#[test]
fn resample_interpolates_at_each_grid_point() {
    let wf = Waveform::new(vec![(0.0, 0.0), (1.0, 10.0), (2.0, 0.0)]);
    let grid = vec![0.0, 0.5, 1.0, 1.5, 2.0];
    let resampled = wf.resample(&grid);
    let values: Vec<f64> = resampled.points().iter().map(|(_, v)| *v).collect();
    let axis: Vec<f64> = resampled.points().iter().map(|(t, _)| *t).collect();
    assert_eq!(axis, grid);
    assert_eq!(values, vec![0.0, 5.0, 10.0, 5.0, 0.0]);
}

/// HOST-15 AC2: `derivative` is exact on a linear signal (`v = 3t + 2` has
/// constant slope `3` everywhere, including the one-sided endpoints).
#[test]
fn derivative_is_exact_on_a_linear_signal() {
    let wf = Waveform::new((0..10).map(|i| { let t = i as f64 * 0.1; (t, 3.0 * t + 2.0) }).collect());
    let d = wf.derivative().expect("derivative solves");
    for (_, v) in d.points() {
        assert!((v - 3.0).abs() < 1e-9, "expected slope 3.0, got {v}");
    }
}

/// HOST-15 edge: `derivative` fails loud on fewer than 2 samples (no slope
/// is defined).
#[test]
fn derivative_fails_loud_on_a_single_sample() {
    let wf = Waveform::new(vec![(0.0, 1.0)]);
    let err = wf.derivative().expect_err("single sample must fail loud");
    assert!(format!("{err}").contains("at least 2 samples"));
}

/// HOST-15 AC2: `integral` is exact on a constant signal
/// (`∫c dt = c·t`, `0` at the first sample).
#[test]
fn integral_is_exact_on_a_constant_signal() {
    let wf = Waveform::new((0..11).map(|i| (i as f64, 5.0)).collect());
    let integ = wf.integral();
    for (t, v) in integ.points() {
        assert!((v - 5.0 * t).abs() < 1e-9, "at t={t}: expected {}, got {v}", 5.0 * t);
    }
}

/// HOST-15 AC2: `clip(lo, hi)` saturates out-of-band values at the nearer
/// bound and leaves in-band values untouched.
#[test]
fn clip_saturates_out_of_band_values() {
    let wf = Waveform::new(vec![(0.0, -5.0), (1.0, 0.5), (2.0, 5.0), (3.0, 1.0)]);
    let clipped = wf.clip(-1.0, 1.0);
    let values: Vec<f64> = clipped.points().iter().map(|(_, v)| *v).collect();
    assert_eq!(values, vec![-1.0, 0.5, 1.0, 1.0]);
}

/// HOST-15 AC2 / spec "fft round-trips a known tone": `n` uniform samples
/// (`t0..t_end` inclusive, the same grid `fft` resamples onto internally) of
/// `amplitude·sin(2π·k·m/n)` — a pure tone at exactly bin index `k` of the
/// `n`-point DFT — round-trips through `fft()` with its peak at bin `k`,
/// magnitude `amplitude/2` (energy split with its mirror bin at `n−k`), and
/// every other bin ≈ 0.
#[test]
fn fft_round_trips_a_known_tone() {
    let n = 64usize;
    let k = 8usize; // the tone's bin index (< n/2, no Nyquist ambiguity)
    let amplitude = 2.0_f64;
    let dt = 1e-4_f64;
    let fs = 1.0 / dt;
    let f0 = k as f64 * fs / n as f64; // the frequency landing exactly on bin k
    let points: Vec<(f64, f64)> = (0..n)
        .map(|m| {
            let t = dt * m as f64;
            (t, amplitude * (2.0 * std::f64::consts::PI * f0 * t).sin())
        })
        .collect();
    let wf = Waveform::new(points);
    let spectrum = wf.fft().expect("fft solves");

    let (freq, c) = spectrum.points()[k];
    assert!((freq - f0).abs() < 1e-6, "tone bin frequency = {freq}, expected {f0}");
    assert!(
        (c.norm() - amplitude / 2.0).abs() < 1e-9,
        "tone bin magnitude = {}, expected {}",
        c.norm(),
        amplitude / 2.0
    );

    // DC bin (k=0) and an off-tone bin both carry ~0 — a pure single-tone
    // sine has no energy anywhere but bins `k` and `n-k`.
    let (_, dc) = spectrum.points()[0];
    assert!(dc.norm() < 1e-9, "DC bin should be ~0, got {}", dc.norm());
    let (_, off) = spectrum.points()[k + 5];
    assert!(off.norm() < 1e-9, "off-tone bin should be ~0, got {}", off.norm());
}

/// HOST-15 edge: `fft` fails loud on fewer than 2 samples.
#[test]
fn fft_fails_loud_on_a_single_sample() {
    let wf = Waveform::new(vec![(0.0, 1.0)]);
    let err = wf.fft().expect_err("single sample must fail loud");
    assert!(format!("{err}").contains("at least 2 samples"));
}

/// HOST-15 edge: `fft` fails loud on a zero-span waveform (all samples at
/// the same axis value).
#[test]
fn fft_fails_loud_on_zero_span() {
    let wf = Waveform::new(vec![(1.0, 0.0), (1.0, 1.0)]);
    let err = wf.fft().expect_err("zero span must fail loud");
    assert!(format!("{err}").contains("span must be positive"));
}
