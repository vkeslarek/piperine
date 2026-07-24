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
}

/// The pre-HOST-13 name for the noise instantiation of the generic container
/// (HOST-13: no separate `NoiseTrace` struct — same `Trace<T>`, discriminated
/// by the zero-sized [`NoiseSample`] marker since noise has no per-net
/// `v`/`i`).
pub type NoiseTrace = Trace<NoiseSample>;
