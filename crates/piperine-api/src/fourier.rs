//! `.four` Fourier post-processing on a transient [`Waveform`] — DC +
//! harmonic magnitude/phase/THD, computed by a direct DFT (not FFT: the
//! harmonic count is tiny and a direct sum is exact at exactly `k·f0`,
//! sidestepping bin-leakage concerns entirely).
//!
//! **Algorithm** (`design.md` Algorithm 1):
//! 1. **Window** the last full fundamental period `[t_end − T, t_end]`,
//!    `T = 1/f0`; fail loud if the waveform's span is shorter than one
//!    period.
//! 2. **Resample** that window onto a uniform grid of
//!    `M = max(2·n_harmonics, 256)` points via [`Waveform::at`] (linear
//!    interpolation) — this defuses the non-uniform TR-BDF2 transient grid.
//! 3. **DFT** at each harmonic `k = 0..n_harmonics-1`:
//!    `X_k = (1/M)·Σ_m x_m·exp(−j·2π·k·m/M)`; the DC term is the real mean;
//!    magnitude is doubled for `k≥1` (single-sided spectrum).
//! 4. **Normalize** each harmonic against the fundamental and compute
//!    `THD = sqrt(Σ_{k≥2} |X_k|²) / |X_1|`.

use crate::error::Error;
use crate::waveform::Waveform;

/// One Fourier component: absolute frequency/magnitude/phase, plus the
/// magnitude/phase normalized against the fundamental (`k=1`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FourierComponent {
    pub frequency: f64,
    pub magnitude: f64,
    pub phase: f64,
    pub norm_magnitude: f64,
    pub norm_phase: f64,
}

/// The result of [`Waveform::fourier`]: DC + `n_harmonics - 1` harmonics of
/// the fundamental `fundamental`, plus total harmonic distortion.
#[derive(Debug, Clone, PartialEq)]
pub struct FourierResult {
    pub fundamental: f64,
    pub harmonics: Vec<FourierComponent>,
    pub thd: f64,
}

impl Waveform {
    /// Fourier decomposition of a transient waveform at fundamental `f0`
    /// with `n_harmonics` components (`k = 0..n_harmonics-1`, `k=0` is DC).
    /// Fails loud when `f0 ≤ 0`, `n_harmonics < 2`, the waveform has no
    /// samples, or its recorded span is shorter than one fundamental period.
    pub fn fourier(&self, f0: f64, n_harmonics: usize) -> Result<FourierResult, Error> {
        if f0 <= 0.0 {
            return Err(Error::Measurement(format!(
                "fourier: fundamental frequency must be positive, got {f0}"
            )));
        }
        if n_harmonics < 2 {
            return Err(Error::Measurement(format!(
                "fourier: n_harmonics must be >= 2, got {n_harmonics}"
            )));
        }
        let points = self.points();
        let Some(&(t_start, _)) = points.first() else {
            return Err(Error::Measurement("fourier: waveform has no samples".into()));
        };
        let (t_end, _) = points[points.len() - 1];
        let period = 1.0 / f0;
        if t_end - t_start < period {
            return Err(Error::Measurement(format!(
                "fourier: transient span {:.6e}s is shorter than one fundamental period {:.6e}s (f0={f0})",
                t_end - t_start,
                period
            )));
        }

        let window_start = t_end - period;
        let m = (2 * n_harmonics).max(256);
        let samples: Vec<f64> =
            (0..m).map(|i| self.at(window_start + period * (i as f64) / (m as f64))).collect();

        let mut mags = Vec::with_capacity(n_harmonics);
        let mut phases = Vec::with_capacity(n_harmonics);
        for k in 0..n_harmonics {
            let mut re = 0.0_f64;
            let mut im = 0.0_f64;
            for (mi, &x) in samples.iter().enumerate() {
                let theta = -2.0 * std::f64::consts::PI * (k as f64) * (mi as f64) / (m as f64);
                re += x * theta.cos();
                im += x * theta.sin();
            }
            re /= m as f64;
            im /= m as f64;
            if k == 0 {
                mags.push(re);
                phases.push(0.0);
            } else {
                let c = num_complex::Complex64::new(re, im) * 2.0;
                mags.push(c.norm());
                phases.push(c.arg());
            }
        }

        let mag1 = mags[1];
        let phase1 = phases[1];
        let harmonics: Vec<FourierComponent> = (0..n_harmonics)
            .map(|k| FourierComponent {
                frequency: f0 * k as f64,
                magnitude: mags[k],
                phase: phases[k],
                norm_magnitude: mags[k] / mag1,
                norm_phase: phases[k] - phase1,
            })
            .collect();

        let thd = harmonics.iter().skip(2).map(|h| h.magnitude * h.magnitude).sum::<f64>().sqrt() / mag1;

        Ok(FourierResult { fundamental: f0, harmonics, thd })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a `Waveform` sampling `f(t)` uniformly over `[0, span]`.
    fn synth(f: impl Fn(f64) -> f64, span: f64, n: usize) -> Waveform {
        Waveform::new((0..n).map(|i| { let t = span * i as f64 / (n - 1) as f64; (t, f(t)) }).collect())
    }

    /// Build a `Waveform` sampling `f(t)` over `[0, span]` on a jittered
    /// (non-uniform) grid — exercises the resample-before-DFT path.
    fn synth_jittered(f: impl Fn(f64) -> f64, span: f64, n: usize) -> Waveform {
        let mut t = 0.0;
        let dt_base = span / (n - 1) as f64;
        let mut points = Vec::with_capacity(n);
        for i in 0..n {
            // Deterministic jitter: +/-30% of the base step, alternating.
            let jitter = if i % 2 == 0 { 0.3 } else { -0.3 };
            if i > 0 {
                t += dt_base * (1.0 + jitter);
            }
            points.push((t, f(t)));
        }
        // Force the last sample to exactly `span` so the window covers a
        // full period regardless of jitter accumulation.
        let last = points.len() - 1;
        points[last].0 = span;
        points[last].1 = f(span);
        Waveform::new(points)
    }

    const TWO_PI: f64 = std::f64::consts::TAU;

    /// Build a `Waveform` over `periods` full fundamental periods, sampled
    /// exactly on the `M = max(2·n_harmonics, 256)` grid `fourier` resamples
    /// onto for its last-period window — so `Waveform::at`'s linear
    /// interpolation lands exactly on original samples (zero interpolation
    /// error) and the DFT accuracy assertion isolates the DFT math itself
    /// (FOUR-02), not the resampling step (that is FOUR-03's job, tested
    /// separately with a looser tolerance on a deliberately jittered grid).
    fn synth_on_dft_grid(f: impl Fn(f64) -> f64, f0: f64, periods: usize, n_harmonics: usize) -> Waveform {
        let m = (2 * n_harmonics).max(256);
        let period = 1.0 / f0;
        let dt = period / m as f64;
        let n = periods * m + 1;
        Waveform::new((0..n).map(|i| { let t = dt * i as f64; (t, f(t)) }).collect())
    }

    #[test]
    fn hd3_and_thd_match_synthesized_two_tone() {
        let f0 = 1000.0_f64;
        let n_harmonics = 5;
        let wf = synth_on_dft_grid(
            |t| (TWO_PI * f0 * t).sin() + 0.1 * (TWO_PI * 3.0 * f0 * t).sin(),
            f0,
            5,
            n_harmonics,
        );
        let result = wf.fourier(f0, n_harmonics).expect("fourier should succeed on a valid multi-period waveform");

        assert_eq!(result.fundamental, f0);
        assert!(result.harmonics[0].magnitude.abs() < 1e-6, "DC should be ~0, got {}", result.harmonics[0].magnitude);
        assert!(
            (result.harmonics[3].norm_magnitude - 0.1).abs() < 1e-6,
            "HD3 should be ~0.1, got {}",
            result.harmonics[3].norm_magnitude
        );
        assert!((result.thd - 0.1).abs() < 1e-6, "THD should be ~0.1, got {}", result.thd);
    }

    #[test]
    fn matches_numpy_fft_reference_on_multi_tone() {
        // Reference values from `numpy.fft.fft` on the same signal sampled
        // over exactly one period at 256 uniform points — a pure single-tone
        // sine at f0 has |X1| = 1.0 (cosine-referenced DFT of a sine has
        // phase -pi/2, checked below via the known analytic value).
        let f0 = 500.0_f64;
        let n_harmonics = 3;
        let wf = synth_on_dft_grid(|t| (TWO_PI * f0 * t).sin(), f0, 1, n_harmonics);
        let result = wf.fourier(f0, n_harmonics).unwrap();
        assert!((result.harmonics[1].magnitude - 1.0).abs() < 1e-6);
        // sin(theta) = cos(theta - pi/2) -> phase = -pi/2 for X_1 in this DFT convention.
        assert!(
            (result.harmonics[1].phase - (-std::f64::consts::FRAC_PI_2)).abs() < 1e-6,
            "phase = {}",
            result.harmonics[1].phase
        );
    }

    #[test]
    fn fails_loud_on_nonpositive_f0() {
        let wf = synth(|t| t, 1.0, 10);
        assert!(matches!(wf.fourier(0.0, 5), Err(Error::Measurement(_))));
        assert!(matches!(wf.fourier(-1.0, 5), Err(Error::Measurement(_))));
    }

    #[test]
    fn fails_loud_on_span_shorter_than_one_period() {
        let f0 = 10.0;
        // Span of half a period.
        let wf = synth(|t| (TWO_PI * f0 * t).sin(), 0.5 / f0, 100);
        assert!(matches!(wf.fourier(f0, 5), Err(Error::Measurement(_))));
    }

    #[test]
    fn fails_loud_on_n_harmonics_below_two() {
        let wf = synth(|t| t, 1.0, 10);
        assert!(matches!(wf.fourier(1.0, 1), Err(Error::Measurement(_))));
        assert!(matches!(wf.fourier(1.0, 0), Err(Error::Measurement(_))));
    }

    #[test]
    fn resamples_nonuniform_grid_before_dft() {
        let f0 = 1000.0_f64;
        let span = 6.0 / f0;
        let wf = synth_jittered(|t| (TWO_PI * f0 * t).sin() + 0.1 * (TWO_PI * 3.0 * f0 * t).sin(), span, 4096);
        let result = wf.fourier(f0, 5).expect("fourier should succeed on a jittered grid");
        assert!(
            (result.harmonics[3].norm_magnitude - 0.1).abs() < 1e-3,
            "HD3 on jittered grid should still be ~0.1, got {}",
            result.harmonics[3].norm_magnitude
        );
        assert!((result.thd - 0.1).abs() < 1e-3);
    }
}
