//! [`Waveform`] and the generic [`Trace`] container — every swept analysis
//! (transient, AC, DC sweep) returns a `Trace<T>` over its own axis; `.v`/`.i`
//! read out a `Waveform<T>` per net (HOST-13: one generic container folds the
//! former `AcTrace` — `Trace<ComplexWaveform>` — and `NoiseTrace` —
//! `Trace<NoiseSample>`, a zero-sized discriminator since noise has no
//! per-net `v`/`i`, only `psd`/`total`).

use std::collections::HashMap;
use std::marker::PhantomData;
use std::rc::Rc;

use piperine_codegen::device::CircuitBuildInfo;
use piperine_solver::abi::SolverStats;
use piperine_solver::prelude::{
    AcAnalysisResult, BranchIdentifier, DcAnalysisResult, NodeIdentifier, NoiseAnalysisResult,
    TransientAnalysisResult,
};

use crate::error::Error;
use crate::results::{NetLookup, NetRef};

/// A series of `(axis, value)` samples — one measured quantity over an
/// analysis axis. Points are assumed sorted by axis (true for every analysis
/// the session runs). `Waveform` (= `Waveform<f64>`) is the transient/DC-sweep
/// real surface; [`ComplexWaveform`] (= `Waveform<Complex64>`) is the AC
/// surface — one struct, two instantiations.
#[derive(Debug, Clone)]
pub struct Waveform<T = f64> {
    points: Vec<(f64, T)>,
}

/// `Waveform<Complex>`: the AC result samples. Scalar reductions live on the
/// `Real` projections returned by `mag`/`phase`/`db`.
pub type ComplexWaveform = Waveform<num_complex::Complex64>;

impl<T: Copy> Waveform<T> {
    pub fn new(points: Vec<(f64, T)>) -> Self {
        Self { points }
    }

    /// The raw `(axis, value)` samples — the numpy seam: hosts split this
    /// into two equal-length arrays.
    pub fn points(&self) -> &[(f64, T)] {
        &self.points
    }

    /// Number of samples.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// `true` when there are no samples.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
}

impl Waveform {
    /// Linear interpolation at `x`; clamps to the first/last sample outside
    /// the recorded range.
    pub fn at(&self, x: f64) -> f64 {
        if self.points.is_empty() {
            return 0.0;
        }
        if x <= self.points[0].0 {
            return self.points[0].1;
        }
        let last = self.points.len() - 1;
        if x >= self.points[last].0 {
            return self.points[last].1;
        }
        let i = self.points.partition_point(|(t, _)| *t <= x).saturating_sub(1).min(last - 1);
        let (t0, v0) = self.points[i];
        let (t1, v1) = self.points[i + 1];
        if t1 == t0 {
            v0
        } else {
            v0 + (v1 - v0) * (x - t0) / (t1 - t0)
        }
    }

    pub fn min(&self) -> f64 {
        self.points.iter().map(|(_, v)| *v).fold(f64::INFINITY, f64::min)
    }
    pub fn max(&self) -> f64 {
        self.points.iter().map(|(_, v)| *v).fold(f64::NEG_INFINITY, f64::max)
    }
    pub fn mean(&self) -> f64 {
        // Time-weighted (trapezoidal) mean over the recorded grid. The
        // transient is always adaptively sampled, so an unweighted average of
        // the sample values would bias toward regions where the stepper took
        // small `dt`. ∫v dt / ∫dt, both by the trapezoidal rule.
        let pts = &self.points;
        if pts.is_empty() {
            return 0.0;
        }
        if pts.len() < 2 {
            return pts[0].1;
        }
        let (mut integ, mut span) = (0.0_f64, 0.0_f64);
        for w in pts.windows(2) {
            let dt = w[1].0 - w[0].0;
            integ += dt * 0.5 * (w[0].1 + w[1].1);
            span += dt;
        }
        if span > 0.0 { integ / span } else { pts[0].1 }
    }
    pub fn rms(&self) -> f64 {
        // Time-weighted RMS: sqrt(∫v² dt / ∫dt), trapezoidal. See `mean` for
        // why the weighting matters on an adaptive grid.
        let pts = &self.points;
        if pts.is_empty() {
            return 0.0;
        }
        if pts.len() < 2 {
            return pts[0].1.abs();
        }
        let (mut integ, mut span) = (0.0_f64, 0.0_f64);
        for w in pts.windows(2) {
            let dt = w[1].0 - w[0].0;
            integ += dt * 0.5 * (w[0].1 * w[0].1 + w[1].1 * w[1].1);
            span += dt;
        }
        if span > 0.0 { (integ / span).sqrt() } else { pts[0].1.abs() }
    }
    pub fn peak_to_peak(&self) -> f64 {
        self.max() - self.min()
    }

    /// First axis value where the waveform crosses `level`, in direction
    /// `dir` (`"Rising"`/`"Falling"`/`"Either"`). `None` if it never does.
    pub fn cross(&self, level: f64, dir: &str) -> Option<f64> {
        for pair in self.points.windows(2) {
            let (t0, v0) = pair[0];
            let (t1, v1) = pair[1];
            let rising = v0 < level && v1 >= level;
            let falling = v0 > level && v1 <= level;
            let hit = match dir {
                "Rising" => rising,
                "Falling" => falling,
                _ => rising || falling,
            };
            if hit && v1 != v0 {
                return Some(t0 + (t1 - t0) * (level - v0) / (v1 - v0));
            }
        }
        None
    }
}

// ─── Real Waveform measurements (HOST-14) ──────────────────────────────────

/// Description of the step transition captured by a real [`Waveform`] — the
/// shared analysis backing every step measurement (`slew_rate`/`rise_time`/
/// `fall_time`/`overshoot`/`settling_time`). Private: the measurement methods
/// below are the public surface; this struct is the cached intermediate.
struct StepAnalysis {
    initial: f64,
    settled: f64,
    low_level: f64,
    high_level: f64,
    rising: bool,
    t_low: f64,
    t_high: f64,
}

impl Waveform {
    /// Analyze the step transition recorded in this waveform (first sample →
    /// last sample). Establishes the 10%/90% thresholds, the step direction,
    /// and the threshold-crossing times shared by every step measurement.
    /// Fails loud when the signal is flat (initial == settled) or the
    /// 10%/90% level is never crossed in the step direction.
    fn step_analysis(&self) -> Result<StepAnalysis, Error> {
        let points = self.points();
        if points.len() < 2 {
            return Err(Error::Measurement(format!(
                "step measurement requires at least 2 samples, got {}",
                points.len()
            )));
        }
        let initial = points[0].1;
        let settled = points[points.len() - 1].1;
        let swing = settled - initial;
        if swing.abs() <= f64::EPSILON {
            return Err(Error::Measurement(format!(
                "step measurement requires a non-flat signal: initial == settled == {initial}"
            )));
        }
        let rising = swing > 0.0;
        let low_level = initial + 0.1 * swing;
        let high_level = initial + 0.9 * swing;
        let dir = if rising { "Rising" } else { "Falling" };
        let t_low = self.cross(low_level, dir).ok_or_else(|| {
            Error::Measurement(format!(
                "step measurement: signal never crosses its 10% level ({low_level:.6e}) \
                 in the {dir} direction"
            ))
        })?;
        let t_high = self.cross(high_level, dir).ok_or_else(|| {
            Error::Measurement(format!(
                "step measurement: signal never crosses its 90% level ({high_level:.6e}) \
                 in the {dir} direction"
            ))
        })?;
        if t_high < t_low {
            return Err(Error::Measurement(format!(
                "step measurement: 90% crossing (t={t_high:.6e}) precedes 10% crossing \
                 (t={t_low:.6e}) — not a monotonic step"
            )));
        }
        Ok(StepAnalysis { initial, settled, low_level, high_level, rising, t_low, t_high })
    }

    /// Average slew rate between the 10% and 90% thresholds of the step
    /// (V/s, HOST-14). Sign-preserving: positive for a rising step, negative
    /// for a falling step. Fails loud on a flat signal or when the signal
    /// never crosses both thresholds in the step direction.
    pub fn slew_rate(&self) -> Result<f64, Error> {
        let s = self.step_analysis()?;
        Ok((s.high_level - s.low_level) / (s.t_high - s.t_low))
    }

    /// 10%→90% rise time of a rising step (seconds, HOST-14). A falling step
    /// fails loud — use [`fall_time`](Self::fall_time). Fails loud on a flat
    /// signal.
    pub fn rise_time(&self) -> Result<f64, Error> {
        let s = self.step_analysis()?;
        if !s.rising {
            return Err(Error::Measurement(
                "rise_time: this signal falls (initial > settled); use fall_time instead".into(),
            ));
        }
        Ok(s.t_high - s.t_low)
    }

    /// 90%→10% fall time of a falling step (seconds, HOST-14). A rising step
    /// fails loud — use [`rise_time`](Self::rise_time). Fails loud on a flat
    /// signal.
    pub fn fall_time(&self) -> Result<f64, Error> {
        let s = self.step_analysis()?;
        if s.rising {
            return Err(Error::Measurement(
                "fall_time: this signal rises (initial < settled); use rise_time instead".into(),
            ));
        }
        Ok(s.t_high - s.t_low)
    }

    /// Peak overshoot as a fraction of the step
    /// `(peak - settled) / |settled - initial|` (dimensionless, HOST-14):
    /// `0.0` for a critically/over-damped response, `~0.1..0.3` typical for
    /// an under-damped second-order step. Reported as a fraction (multiply
    /// by 100 for percent). Fails loud on a flat signal (no step magnitude).
    pub fn overshoot(&self) -> Result<f64, Error> {
        let s = self.step_analysis()?;
        let peak_beyond_settled = if s.rising { self.max() - s.settled } else { s.settled - self.min() };
        Ok(peak_beyond_settled / (s.settled - s.initial).abs())
    }

    /// Time at which the signal enters and remains within `tol` (absolute,
    /// same units as the signal) of the settled (last-sample) value
    /// (seconds, HOST-14). Returns the first sample strictly after the last
    /// out-of-band excursion, or the first sample time if the signal never
    /// leaves the band. Fails loud if the signal never settles within `tol`
    /// by the end of the recording, or if `tol < 0`.
    pub fn settling_time(&self, tol: f64) -> Result<f64, Error> {
        if tol < 0.0 {
            return Err(Error::Measurement(format!(
                "settling_time: tolerance must be non-negative, got {tol}"
            )));
        }
        let points = self.points();
        if points.is_empty() {
            return Err(Error::Measurement("settling_time: empty waveform".into()));
        }
        let settled = points[points.len() - 1].1;
        let last_outside = points
            .iter()
            .rev()
            .find(|(_, v)| (v - settled).abs() > tol)
            .map(|(t, _)| *t);
        match last_outside {
            None => Ok(points[0].0),
            Some(t_out) => points
                .iter()
                .find(|(t, _)| *t > t_out)
                .map(|(t, _)| *t)
                .ok_or_else(|| {
                    Error::Measurement(format!(
                        "settling_time: signal never settles within {tol} of {settled:.6e} \
                         (last out-of-band sample at t={t_out:.6e} is the final recorded sample)"
                    ))
                }),
        }
    }

    /// Propagation delay from this waveform to `other` at `level` (seconds,
    /// HOST-14): the time between this signal crossing `level` (either
    /// direction) and `other` crossing `level`. Positive when `other` lags
    /// this signal. Fails loud if either waveform never crosses `level`.
    pub fn delay(&self, other: &Waveform, level: f64) -> Result<f64, Error> {
        let t_self = self.cross(level, "Either").ok_or_else(|| {
            Error::Measurement(format!(
                "delay: this waveform never crosses level {level:.6e}"
            ))
        })?;
        let t_other = other.cross(level, "Either").ok_or_else(|| {
            Error::Measurement(format!(
                "delay: `other` waveform never crosses level {level:.6e}"
            ))
        })?;
        Ok(t_other - t_self)
    }
}

// ─── Real Waveform transforms (HOST-17 spec, HOST-15 task) ─────────────────
//
// Each transform returns a **new** `Waveform` — the source is never mutated
// (matches the measurement methods' read-only style).

impl Waveform {
    /// Resample onto an explicit `grid` of axis values via linear
    /// interpolation ([`Self::at`]) — the same interpolation the transient
    /// stepper's non-uniform grid already relies on elsewhere (`fourier`'s
    /// window resample, `Trace::at`). HOST-15.
    pub fn resample(&self, grid: &[f64]) -> Waveform {
        Waveform::new(grid.iter().map(|&x| (x, self.at(x))).collect())
    }

    /// First derivative `dv/dx` (central difference at interior samples,
    /// one-sided at the two endpoints) — same axis, one sample per input
    /// sample. HOST-15. Fails loud on fewer than 2 samples (no slope is
    /// defined).
    pub fn derivative(&self) -> Result<Waveform, Error> {
        let pts = self.points();
        let n = pts.len();
        if n < 2 {
            return Err(Error::Measurement(format!(
                "derivative: requires at least 2 samples, got {n}"
            )));
        }
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let d = if i == 0 {
                (pts[1].1 - pts[0].1) / (pts[1].0 - pts[0].0)
            } else if i == n - 1 {
                (pts[n - 1].1 - pts[n - 2].1) / (pts[n - 1].0 - pts[n - 2].0)
            } else {
                (pts[i + 1].1 - pts[i - 1].1) / (pts[i + 1].0 - pts[i - 1].0)
            };
            out.push((pts[i].0, d));
        }
        Ok(Waveform::new(out))
    }

    /// Cumulative trapezoidal integral `∫v dx`, `0` at the first sample's
    /// axis value — same axis, one running-sum sample per input sample.
    /// HOST-15. An empty waveform integrates to an empty waveform.
    pub fn integral(&self) -> Waveform {
        let pts = self.points();
        if pts.is_empty() {
            return Waveform::new(Vec::new());
        }
        let mut out = Vec::with_capacity(pts.len());
        let mut acc = 0.0_f64;
        out.push((pts[0].0, acc));
        for w in pts.windows(2) {
            let dt = w[1].0 - w[0].0;
            acc += dt * 0.5 * (w[0].1 + w[1].1);
            out.push((w[1].0, acc));
        }
        Waveform::new(out)
    }

    /// Clamp every sample value into `[lo, hi]` — same axis, values outside
    /// the band saturate at the nearer bound. HOST-15.
    pub fn clip(&self, lo: f64, hi: f64) -> Waveform {
        Waveform::new(self.points().iter().map(|&(t, v)| (t, v.clamp(lo, hi))).collect())
    }

    /// Discrete Fourier transform of the waveform as a full complex spectrum
    /// (HOST-15): resamples onto `n` uniform points over the recorded span
    /// (`n` = the input sample count; same inclusive-endpoint grid
    /// [`Self::resample`]/[`Self::derivative`] use, so the adaptive
    /// transient grid doesn't leak into the spectrum) and computes the
    /// direct DFT `X_k = (1/n)·Σ_m x_m·exp(−j·2π·k·m/n)`, `k = 0..n-1` — a
    /// full (not single-sided) spectrum, frequency axis `k·fs/n` with
    /// `fs = (n-1)/span`. A real single-tone input round-trips: the bin
    /// nearest the tone frequency carries magnitude ≈ amplitude/2 (energy
    /// split with its mirror bin at `n−k`), a mirror of [`Self::fourier`]'s
    /// harmonic-doubling convention. Fails loud on fewer than 2 samples or a
    /// non-positive span.
    pub fn fft(&self) -> Result<ComplexWaveform, Error> {
        let pts = self.points();
        let n = pts.len();
        if n < 2 {
            return Err(Error::Measurement(format!("fft: requires at least 2 samples, got {n}")));
        }
        let t0 = pts[0].0;
        let t_end = pts[n - 1].0;
        let span = t_end - t0;
        if span <= 0.0 {
            return Err(Error::Measurement(format!(
                "fft: waveform span must be positive, got {span:.6e}"
            )));
        }
        // `n` uniform samples over `n-1` intervals, `t0..t_end` inclusive —
        // the same grid `resample`/`derivative` use.
        let dt = span / (n as f64 - 1.0);
        let samples: Vec<f64> = (0..n).map(|i| self.at(t0 + dt * i as f64)).collect();
        let fs = 1.0 / dt;
        let mut out = Vec::with_capacity(n);
        for k in 0..n {
            let mut re = 0.0_f64;
            let mut im = 0.0_f64;
            for (m, &x) in samples.iter().enumerate() {
                let theta = -2.0 * std::f64::consts::PI * (k as f64) * (m as f64) / (n as f64);
                re += x * theta.cos();
                im += x * theta.sin();
            }
            re /= n as f64;
            im /= n as f64;
            let freq = k as f64 * fs / n as f64;
            out.push((freq, num_complex::Complex64::new(re, im)));
        }
        Ok(ComplexWaveform::new(out))
    }
}

// ─── Trace<T>: the generic swept-analysis container ────────────────────────

/// Zero-sized discriminator selecting the noise instantiation of
/// [`Trace<T>`]: noise has no per-net `v`/`i` (frequency-domain PSD only), so
/// it takes its own marker instead of a sample type — `Trace<NoiseSample>`
/// (aliased [`NoiseTrace`]) carries `psd`/`total` instead.
#[derive(Debug, Clone, Copy)]
pub struct NoiseSample;

/// The data actually backing a [`Trace`] — one variant per analysis kind that
/// produces a swept result. `Trace<T>`'s public methods are implemented per
/// `T` (below), each reading only the variant its `T` was constructed from.
enum TraceBackend {
    Transient { result: TransientAnalysisResult, info: Rc<CircuitBuildInfo> },
    Ac { result: AcAnalysisResult, info: Rc<CircuitBuildInfo> },
    /// A compile-once DC sweep (`Session::dc`, HOST-05/MD-18): one
    /// [`DcAnalysisResult`] + digital snapshot per swept axis value, over the
    /// same [`CircuitBuildInfo`] as a transient trace — `v`/`i` read exactly
    /// like [`crate::results::OpResult`], keyed by sweep index instead of
    /// simulated time.
    DcSweep {
        axis: Vec<f64>,
        points: Vec<DcAnalysisResult>,
        digital: Vec<HashMap<String, f64>>,
        info: Rc<CircuitBuildInfo>,
        stats: SolverStats,
    },
    Noise { result: NoiseAnalysisResult },
}

/// The result of a swept analysis: transient (`Trace<Waveform>`), DC sweep
/// (`Trace<Waveform>`), AC (`Trace<ComplexWaveform>`, formerly `AcTrace`), or
/// noise (`Trace<NoiseSample>`, formerly `NoiseTrace`) — one generic
/// container (HOST-13), `T` selecting which analysis constructed it and
/// which methods below apply.
pub struct Trace<T = Waveform> {
    backend: TraceBackend,
    _marker: PhantomData<T>,
}

impl<T> std::fmt::Debug for Trace<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Trace").finish_non_exhaustive()
    }
}

/// Resolve a host-visible net name to a solver node — shared by every
/// backend.
fn node_or_err(info: &CircuitBuildInfo, name: &str) -> Result<NodeIdentifier, Error> {
    info.net_node(name).ok_or_else(|| Error::Measurement(format!("net `{name}` is not addressable")))
}

// ─── Transient + DC sweep: `Trace<Waveform>` ───────────────────────────────

impl Trace<Waveform> {
    /// Build a transient trace (the pre-HOST-13 `Trace::new` shape, kept for
    /// existing call sites).
    pub fn new(result: TransientAnalysisResult, info: Rc<CircuitBuildInfo>) -> Self {
        Self { backend: TraceBackend::Transient { result, info }, _marker: PhantomData }
    }

    /// Build a DC-sweep trace (`Session::dc`, HOST-05): one point per swept
    /// axis value, produced on the same compiled circuit (MD-18 restamp).
    pub fn from_dc_sweep(
        axis: Vec<f64>,
        points: Vec<DcAnalysisResult>,
        digital: Vec<HashMap<String, f64>>,
        info: Rc<CircuitBuildInfo>,
        stats: SolverStats,
    ) -> Self {
        Self {
            backend: TraceBackend::DcSweep { axis, points, digital, info, stats },
            _marker: PhantomData,
        }
    }

    /// Per-analysis convergence + performance statistics.
    pub fn stats(&self) -> &SolverStats {
        match &self.backend {
            TraceBackend::Transient { result, .. } => &result.stats,
            TraceBackend::DcSweep { stats, .. } => stats,
            _ => unreachable!("Trace<Waveform> is only built from Transient/DcSweep data"),
        }
    }

    /// Net voltage `a` minus `b` (ground-referenced when `b` is `None`) over
    /// the trace's axis (simulated time for a transient, the swept value for
    /// a DC sweep). A digital net read returns its logic value (0/1, NaN for
    /// X/Z) on a transient trace; a DC sweep reads through the per-point
    /// digital snapshot the same way.
    pub fn v(&self, a: &NetRef, b: Option<&NetRef>) -> Result<Waveform, Error> {
        match &self.backend {
            TraceBackend::Transient { result, info } => Self::v_transient(result, info, a, b),
            TraceBackend::DcSweep { axis, points, digital, info, .. } => {
                Self::v_dc_sweep(axis, points, digital, info, a, b)
            }
            TraceBackend::Ac { .. } | TraceBackend::Noise { .. } => {
                unreachable!("Trace<Waveform> is only built from Transient/DcSweep data")
            }
        }
    }

    fn v_transient(
        result: &TransientAnalysisResult,
        info: &CircuitBuildInfo,
        a: &NetRef,
        b: Option<&NetRef>,
    ) -> Result<Waveform, Error> {
        if let Some(&idx) = info.digital_nets.get(&a.name) {
            use piperine_solver::prelude::LogicValue;
            let points = result
                .iter()
                .map(|step| {
                    let v = match step.digital(idx) {
                        Some(LogicValue::Zero) => 0.0,
                        Some(LogicValue::One) => 1.0,
                        _ => f64::NAN,
                    };
                    (step.time(), v)
                })
                .collect();
            return Ok(Waveform::new(points));
        }
        let node_a = node_or_err(info, &a.name)?;
        let node_b = match b {
            Some(nb) => Some(node_or_err(info, &nb.name)?),
            None => None,
        };
        let points = result
            .iter()
            .map(|step| {
                let va = if node_a == NodeIdentifier::Gnd { 0.0 } else { step.get_node(&node_a).unwrap_or(0.0) };
                let vb = match &node_b {
                    Some(nb) if *nb == NodeIdentifier::Gnd => 0.0,
                    Some(nb) => step.get_node(nb).unwrap_or(0.0),
                    None => 0.0,
                };
                (step.time(), va - vb)
            })
            .collect();
        Ok(Waveform::new(points))
    }

    fn v_dc_sweep(
        axis: &[f64],
        points: &[DcAnalysisResult],
        digital: &[HashMap<String, f64>],
        info: &CircuitBuildInfo,
        a: &NetRef,
        b: Option<&NetRef>,
    ) -> Result<Waveform, Error> {
        if info.digital_nets.contains_key(&a.name) {
            let out = axis
                .iter()
                .zip(digital)
                .map(|(&x, snap)| (x, snap.get(&a.name).copied().unwrap_or(f64::NAN)))
                .collect();
            return Ok(Waveform::new(out));
        }
        let node_a = node_or_err(info, &a.name)?;
        let node_b = match b {
            Some(nb) => Some(node_or_err(info, &nb.name)?),
            None => None,
        };
        let out = axis
            .iter()
            .zip(points)
            .map(|(&x, dc)| {
                let va = if node_a == NodeIdentifier::Gnd { 0.0 } else { dc.get_node(&node_a).unwrap_or(0.0) };
                let vb = match &node_b {
                    Some(nb) if *nb == NodeIdentifier::Gnd => 0.0,
                    Some(nb) => dc.get_node(nb).unwrap_or(0.0),
                    None => 0.0,
                };
                (x, va - vb)
            })
            .collect();
        Ok(Waveform::new(out))
    }

    /// A branch current over the trace's axis. See the pre-HOST-13
    /// `Trace::i` doc for the transient recompute recipe (ideal-source force
    /// branch vs kernel-recomputed resistive/reactive current). The DC-sweep
    /// variant reads the same way per point, without a reactive (`dQ/dt`)
    /// term — every point is its own independent operating point.
    pub fn i(&self, a: &NetRef, b: Option<&NetRef>) -> Result<Waveform, Error> {
        match &self.backend {
            TraceBackend::Transient { result, info } => Self::i_transient(result, info, a, b),
            TraceBackend::DcSweep { axis, points, info, .. } => Self::i_dc_sweep(axis, points, info, a, b),
            TraceBackend::Ac { .. } | TraceBackend::Noise { .. } => {
                unreachable!("Trace<Waveform> is only built from Transient/DcSweep data")
            }
        }
    }

    fn i_transient(
        result: &TransientAnalysisResult,
        info: &CircuitBuildInfo,
        a: &NetRef,
        b: Option<&NetRef>,
    ) -> Result<Waveform, Error> {
        let node_a = node_or_err(info, &a.name)?;
        let node_b = match b {
            Some(nb) => node_or_err(info, &nb.name)?,
            None => NodeIdentifier::Gnd,
        };
        let instance = crate::results::find_two_terminal_instance(info, node_a.clone(), node_b.clone())?;
        if instance.num_forces > 0 {
            let branch = BranchIdentifier::new(instance.label.clone(), "force0".to_string());
            let points = result
                .iter()
                .map(|step| (step.time(), step.get_branch(branch.clone()).unwrap_or(0.0)))
                .collect();
            return Ok(Waveform::new(points));
        }
        // Devices whose residual reads runtime state/vars need the opt-in
        // per-step recording (`run_tran(record_device_state = true)`);
        // without it the read fails loud. `ddt` is reactive (charge), not
        // state, so R/C/nonlinear devices pass; `idt`/`delay` read state.
        let (_, state_read, vars_read) = instance.kernel.read_bounds();
        let needs_banks = state_read > 0 || vars_read > 0;
        if needs_banks && !result.iter().all(|s| s.device_state(&instance.label).is_some()) {
            return Err(Error::Measurement(format!(
                "`i()` over time on `{}` is not recorded: the device reads runtime state/vars not captured per step (rerun with record_device_state = true)",
                instance.label
            )));
        }
        // Resistive current (terminal-0 reference) + terminal-0 charge, per
        // step. The reactive current a→b is `sign * dQ_0/dt`; the resistive
        // is `sign * residual[0]` (same convention as `OpResult::i`).
        let sign = if instance.terminals[0] == node_a { 1.0 } else { -1.0 };
        let sim = piperine_codegen::SimCtx::default();
        let n = result.len();
        let mut t_series = Vec::with_capacity(n);
        let mut i_res = Vec::with_capacity(n);
        let mut q0 = Vec::with_capacity(n);
        for step in result.iter() {
            let volts: Vec<f64> = instance
                .terminals
                .iter()
                .map(|t| if *t == NodeIdentifier::Gnd { 0.0 } else { step.get_node(t).unwrap_or(0.0) })
                .collect();
            let (state, vars): (&[f64], &[f64]) = step
                .device_state(&instance.label)
                .map(|(s, v)| (s.as_slice(), v.as_slice()))
                .unwrap_or((&[], &[]));
            let mut residual = vec![0.0; instance.terminals.len()];
            instance.kernel.eval_residual(&volts, &instance.params, state, vars, &sim, &mut residual);
            let mut charge = vec![0.0; instance.terminals.len()];
            instance.kernel.eval_charge(&volts, &instance.params, state, vars, &sim, &mut charge);
            i_res.push(residual[0]);
            q0.push(charge[0]);
            t_series.push(step.time());
        }
        let mut points = Vec::with_capacity(n);
        for k in 0..n {
            let dq_dt = if k == 0 {
                if n > 1 && (t_series[1] - t_series[0]) > 0.0 {
                    (q0[1] - q0[0]) / (t_series[1] - t_series[0])
                } else {
                    0.0
                }
            } else if (t_series[k] - t_series[k - 1]) > 0.0 {
                (q0[k] - q0[k - 1]) / (t_series[k] - t_series[k - 1])
            } else {
                0.0
            };
            points.push((t_series[k], sign * (i_res[k] + dq_dt)));
        }
        Ok(Waveform::new(points))
    }

    fn i_dc_sweep(
        axis: &[f64],
        points: &[DcAnalysisResult],
        info: &CircuitBuildInfo,
        a: &NetRef,
        b: Option<&NetRef>,
    ) -> Result<Waveform, Error> {
        let node_a = node_or_err(info, &a.name)?;
        let node_b = match b {
            Some(nb) => node_or_err(info, &nb.name)?,
            None => NodeIdentifier::Gnd,
        };
        let instance = crate::results::find_two_terminal_instance(info, node_a.clone(), node_b)?;
        let sim = piperine_codegen::SimCtx::default();
        let out = axis
            .iter()
            .zip(points)
            .map(|(&x, dc)| {
                let volts: Vec<f64> = instance
                    .terminals
                    .iter()
                    .map(|t| if *t == NodeIdentifier::Gnd { 0.0 } else { dc.get_node(t).unwrap_or(0.0) })
                    .collect();
                if instance.num_forces > 0 {
                    let branch = BranchIdentifier::new(instance.label.clone(), "force0".to_string());
                    return (x, dc.get_branch(branch).unwrap_or(0.0));
                }
                let mut residual = vec![0.0; instance.terminals.len()];
                instance.kernel.eval_residual(&volts, &instance.params, &[], &[], &sim, &mut residual);
                let current = if instance.terminals[0] == node_a { residual[0] } else { -residual[0] };
                (x, current)
            })
            .collect();
        Ok(Waveform::new(out))
    }

    /// A recorded device opvar over time (HOST-08): `path` is
    /// `"instance.opvar_name"` — the same instance label and (renamed, when
    /// `@name` is declared) opvar name `OpResult::instance(label).opvar(name)`
    /// reads at a single point. Requires the instance's observable to have
    /// been requested via `probe=` (or `record_device_state = true`) on the
    /// `tran` that produced this trace; a step with no recorded bank fails
    /// loud, mirroring [`Self::i`]'s state-reading-device error.
    pub fn opvar(&self, path: &str) -> Result<Waveform, Error> {
        match &self.backend {
            TraceBackend::Transient { result, info } => Self::opvar_transient(result, info, path),
            TraceBackend::DcSweep { .. } => Err(Error::Measurement(
                "opvar() over time is a transient-only feature; a DC sweep has no per-step \
                 device-state recording — read `OpResult::instance(label).opvar(name)` per point instead"
                    .into(),
            )),
            TraceBackend::Ac { .. } | TraceBackend::Noise { .. } => {
                unreachable!("Trace<Waveform> is only built from Transient/DcSweep data")
            }
        }
    }

    fn opvar_transient(
        result: &TransientAnalysisResult,
        info: &CircuitBuildInfo,
        path: &str,
    ) -> Result<Waveform, Error> {
        let (label, name) = crate::results::split_probe_path(path)?;
        let instance = info
            .instances
            .iter()
            .find(|i| i.label == label)
            .ok_or_else(|| Error::Measurement(format!("no element labeled `{label}`")))?;
        let j = instance.opvar_display_names.iter().position(|n| n == name).ok_or_else(|| {
            Error::Measurement(format!(
                "instance `{label}` has no opvar `{name}`; available: {}",
                instance.opvar_display_names.join(", ")
            ))
        })?;
        let sim = piperine_codegen::SimCtx::default();
        let mut points = Vec::with_capacity(result.len());
        for step in result.iter() {
            let volts: Vec<f64> = instance
                .terminals
                .iter()
                .map(|t| if *t == NodeIdentifier::Gnd { 0.0 } else { step.get_node(t).unwrap_or(0.0) })
                .collect();
            let (state, vars): (&[f64], &[f64]) = step.device_state(&instance.label).map(|(s, v)| (s.as_slice(), v.as_slice())).ok_or_else(|| {
                Error::Measurement(format!(
                    "opvar `{path}` is not recorded: rerun `tran` with `probe = [\"{path}\"]` \
                     (or `record_device_state = true`)"
                ))
            })?;
            let mut out = vec![0.0; instance.opvar_display_names.len()];
            instance.kernel.eval_opvars(&volts, &instance.params, state, vars, &sim, &mut out);
            points.push((step.time(), out[j]));
        }
        Ok(Waveform::new(points))
    }

    /// The trace's own axis (simulated time for a transient, the swept value
    /// for a DC sweep) as a real waveform (identity `(x, x)` pairs).
    pub fn axis(&self) -> Waveform {
        match &self.backend {
            TraceBackend::Transient { result, .. } => {
                Waveform::new(result.iter().map(|step| (step.time(), step.time())).collect())
            }
            TraceBackend::DcSweep { axis, .. } => Waveform::new(axis.iter().map(|&x| (x, x)).collect()),
            TraceBackend::Ac { .. } | TraceBackend::Noise { .. } => {
                unreachable!("Trace<Waveform> is only built from Transient/DcSweep data")
            }
        }
    }
}

/// The pre-HOST-13 transient-only name, kept as the concrete transient/DC
/// instantiation of the generic container.
pub type TranTrace = Trace<Waveform>;

// ─── AC: `Trace<ComplexWaveform>` (formerly `AcTrace`) ─────────────────────

impl ComplexWaveform {
    fn project(&self, f: impl Fn(&num_complex::Complex64) -> f64) -> Waveform {
        Waveform::new(self.points.iter().map(|(x, c)| (*x, f(c))).collect())
    }

    /// Magnitude projection `|c|` per sample.
    pub fn mag(&self) -> Waveform {
        self.project(|c| c.norm())
    }
    /// Phase projection `arg(c)` (radians) per sample.
    pub fn phase(&self) -> Waveform {
        self.project(|c| c.arg())
    }
    /// Decibel projection `20·log10|c|` per sample.
    pub fn db(&self) -> Waveform {
        self.project(|c| 20.0 * c.norm().log10())
    }
    /// Nearest sample to `x` (no complex interpolation).
    pub fn at(&self, x: f64) -> num_complex::Complex64 {
        self.points
            .iter()
            .min_by(|a, b| (a.0 - x).abs().total_cmp(&(b.0 - x).abs()))
            .map(|(_, c)| *c)
            .unwrap_or_default()
    }
}

// ─── ComplexWaveform margins/bandwidth (HOST-16) ────────────────────────────

impl ComplexWaveform {
    /// -3 dB bandwidth (HOST-16): the frequency where `|H(f)|` first falls
    /// to `1/√2` of the reference magnitude — `|H|` at the trace's first
    /// (lowest-frequency) sample, the conventional DC/low-frequency gain
    /// reference for a Bode magnitude plot. Fails loud when the reference
    /// magnitude is non-positive (no meaningful -3dB level) or the
    /// magnitude never falls to that level.
    pub fn bandwidth_3db(&self) -> Result<f64, Error> {
        let mag = self.mag();
        let ref_mag = mag
            .points()
            .first()
            .map(|&(_, m)| m)
            .ok_or_else(|| Error::Measurement("bandwidth_3db: empty AC trace".into()))?;
        if ref_mag <= 0.0 {
            return Err(Error::Measurement(format!(
                "bandwidth_3db: reference (first-sample) magnitude must be positive, got {ref_mag:.6e}"
            )));
        }
        let target = ref_mag / std::f64::consts::SQRT_2;
        mag.cross(target, "Falling").ok_or_else(|| {
            Error::Measurement(format!(
                "bandwidth_3db: |H(f)| never falls to -3dB of the reference magnitude ({target:.6e})"
            ))
        })
    }

    /// Unity-gain frequency (HOST-16): the frequency where `|H(f)|` first
    /// crosses `1` (`0 dB`). Fails loud when the magnitude never crosses
    /// unity.
    pub fn unity_gain_freq(&self) -> Result<f64, Error> {
        self.mag().cross(1.0, "Falling").ok_or_else(|| {
            Error::Measurement("unity_gain_freq: |H(f)| never crosses 1 (0 dB)".into())
        })
    }

    /// Phase margin in degrees (HOST-16): `180° + phase(f_ug)`, where `f_ug`
    /// is [`Self::unity_gain_freq`] and phase is read in the trace's own
    /// (wrapped, `arg()`-range) convention — the conventional definition for
    /// a well-behaved (single unity-gain crossing) loop-gain trace. Fails
    /// loud when there is no unity-gain crossing (propagates
    /// `unity_gain_freq`'s error).
    pub fn phase_margin(&self) -> Result<f64, Error> {
        let f_ug = self.unity_gain_freq()?;
        Ok(180.0 + self.phase().at(f_ug).to_degrees())
    }

    /// Gain margin in dB (HOST-16): `-20·log10(|H(f_180)|)`, where `f_180`
    /// is the frequency at which the **unwrapped** phase first crosses
    /// `-180°` — phase is unwrapped (accumulating ±360° jumps) before the
    /// crossing search since `arg()` is range-limited to `(-π, π]` and a
    /// multi-pole rolloff's phase legitimately passes through `-180°` after
    /// wrapping past `-180°`/`+180°` more than once. Fails loud when the
    /// (unwrapped) phase never reaches `-180°` (e.g. a single-pole rolloff,
    /// asymptotic to `-90°`) or the magnitude there is non-positive.
    pub fn gain_margin(&self) -> Result<f64, Error> {
        let unwrapped = self.unwrapped_phase_deg();
        let f180 = unwrapped.cross(-180.0, "Falling").ok_or_else(|| {
            Error::Measurement("gain_margin: (unwrapped) phase never crosses -180°".into())
        })?;
        let mag_at_180 = self.mag().at(f180);
        if mag_at_180 <= 0.0 {
            return Err(Error::Measurement(format!(
                "gain_margin: |H(f_180)| must be positive, got {mag_at_180:.6e}"
            )));
        }
        Ok(-20.0 * mag_at_180.log10())
    }

    /// Phase in degrees, unwrapped by accumulating a running ±360° offset
    /// whenever consecutive samples jump by more than 180° — undoes `arg()`
    /// range-wrapping so a monotonically-rolling-off phase (e.g. a 3-pole
    /// system crossing -180°, -270°, …) reads as a continuous curve instead
    /// of resetting into `(-180°, 180°]` every half-turn.
    fn unwrapped_phase_deg(&self) -> Waveform {
        let mut out = Vec::with_capacity(self.points.len());
        let mut offset = 0.0_f64;
        let mut prev: Option<f64> = None;
        for &(f, c) in &self.points {
            let mut p = c.arg().to_degrees() + offset;
            if let Some(prev_p) = prev {
                while p - prev_p > 180.0 {
                    offset -= 360.0;
                    p -= 360.0;
                }
                while p - prev_p < -180.0 {
                    offset += 360.0;
                    p += 360.0;
                }
            }
            out.push((f, p));
            prev = Some(p);
        }
        Waveform::new(out)
    }
}

impl Trace<ComplexWaveform> {
    /// Build an AC-sweep trace (the pre-HOST-13 `AcTrace::new` shape, kept
    /// for existing call sites).
    pub fn new(result: AcAnalysisResult, info: Rc<CircuitBuildInfo>) -> Self {
        Self { backend: TraceBackend::Ac { result, info }, _marker: PhantomData }
    }

    fn ac_parts(&self) -> (&AcAnalysisResult, &CircuitBuildInfo) {
        match &self.backend {
            TraceBackend::Ac { result, info } => (result, info),
            _ => unreachable!("Trace<ComplexWaveform> is only built from AC data"),
        }
    }

    /// Net voltage `a` minus `b` (ground-referenced when `b` is `None`) over
    /// the AC frequency sweep.
    pub fn v(&self, a: &NetRef, b: Option<&NetRef>) -> Result<ComplexWaveform, Error> {
        let (result, info) = self.ac_parts();
        let node_a = node_or_err(info, &a.name)?;
        let node_b = match b {
            Some(nb) => Some(node_or_err(info, &nb.name)?),
            None => None,
        };
        let zero = num_complex::Complex64::default();
        let points = result
            .iter()
            .map(|step| {
                let va = if node_a == NodeIdentifier::Gnd { zero } else { step.get_node(&node_a).copied().unwrap_or(zero) };
                let vb = match &node_b {
                    Some(nb) if *nb == NodeIdentifier::Gnd => zero,
                    Some(nb) => step.get_node(nb).copied().unwrap_or(zero),
                    None => zero,
                };
                (step.frequency, va - vb)
            })
            .collect();
        Ok(ComplexWaveform::new(points))
    }

    /// The frequency axis as a real waveform.
    pub fn axis(&self) -> Waveform {
        let (result, _) = self.ac_parts();
        let points = result.iter().map(|s| (s.frequency, s.frequency)).collect();
        Waveform::new(points)
    }
}

/// The pre-HOST-13 name for the AC-sweep instantiation of the generic
/// container (HOST-13: no separate `AcTrace` struct — same `Trace<T>`).
pub type AcTrace = Trace<ComplexWaveform>;

// ─── Noise: `Trace<NoiseSample>` (formerly `NoiseTrace`) ───────────────────

impl Trace<NoiseSample> {
    /// Build a noise trace (the pre-HOST-13 `NoiseTrace::new` shape, kept for
    /// existing call sites).
    pub fn new(result: NoiseAnalysisResult) -> Self {
        Self { backend: TraceBackend::Noise { result }, _marker: PhantomData }
    }

    fn noise(&self) -> &NoiseAnalysisResult {
        match &self.backend {
            TraceBackend::Noise { result } => result,
            _ => unreachable!("Trace<NoiseSample> is only built from noise data"),
        }
    }

    /// Output-referred noise PSD as `(frequency, v²/Hz)` samples.
    pub fn psd(&self) -> Waveform {
        let result = self.noise();
        Waveform::new(result.frequencies.iter().zip(&result.out_noise_sq).map(|(f, v)| (*f, *v)).collect())
    }

    /// The integrated total noise (RMS).
    pub fn total(&self) -> f64 {
        self.noise().integrated_noise
    }

    /// Per-source noise PSD as `Waveform`s (HOST-11): keyed
    /// `"element/source"` (e.g. `"r1/thermal"`, `"m1/flicker"`). Each
    /// waveform's points are `(frequency, v²/Hz)` — the PSD contribution
    /// of that source alone to the output noise. Sum of per-source PSDs
    /// at any frequency reconciles with [`psd`](Self::psd) (conservation).
    pub fn by_source(&self) -> std::collections::HashMap<String, Waveform> {
        let result = self.noise();
        result
            .contributions
            .iter()
            .map(|c| {
                let key = format!("{}/{}", c.element, c.source);
                let points = result.frequencies.iter().zip(&c.psd).map(|(f, v)| (*f, *v)).collect();
                (key, Waveform::new(points))
            })
            .collect()
    }

    /// The full per-source contribution catalog (HOST-11): each entry
    /// carries `element`/`source`/`kind`/`psd`/`integrated_sq` — beyond the
    /// scalar [`total`](Self::total). Sum of `integrated_sq` across all
    /// entries reconciles with `total()²` (conservation).
    pub fn contributions(&self) -> &[piperine_solver::abi::NoiseContribution] {
        &self.noise().contributions
    }
}

/// The pre-HOST-13 name for the noise instantiation of the generic container
/// (HOST-13: no separate `NoiseTrace` struct — same `Trace<T>`, discriminated
/// by the zero-sized [`NoiseSample`] marker since noise has no per-net
/// `v`/`i`).
pub type NoiseTrace = Trace<NoiseSample>;
