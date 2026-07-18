//! [`Waveform`] and the swept-analysis traces — a transient/AC/noise
//! analysis returns a trace, and `.v`/`.i` on it read out a [`Waveform`]
//! over the analysis axis (time or frequency).

use std::rc::Rc;

use piperine_codegen::device::CircuitBuildInfo;
use piperine_solver::prelude::{BranchIdentifier, NodeIdentifier, TransientAnalysisResult};

use crate::error::Error;
use crate::results::{NetLookup, NetRef};

/// A series of `(axis, value)` samples — one measured quantity over an
/// analysis axis. Points are assumed sorted by axis (true for every analysis
/// the session runs). `Waveform` (= `Waveform<f64>`) is the transient real
/// surface; [`ComplexWaveform`] (= `Waveform<Complex64>`) is the AC surface —
/// one struct, two instantiations.
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

/// The result of a transient analysis: a swept result, read by name into a
/// [`Waveform`] per branch/node.
pub struct Trace {
    result: TransientAnalysisResult,
    info: Rc<CircuitBuildInfo>,
}

impl std::fmt::Debug for Trace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Trace").finish_non_exhaustive()
    }
}

impl Trace {
    pub fn new(result: TransientAnalysisResult, info: Rc<CircuitBuildInfo>) -> Self {
        Self { result, info }
    }

    /// Per-analysis convergence + performance statistics.
    pub fn stats(&self) -> &piperine_solver::abi::SolverStats {
        &self.result.stats
    }

    /// Resolve a host-visible net name to a solver node.
    fn node_or_err(&self, name: &str) -> Result<NodeIdentifier, Error> {
        self.info
            .net_node(name)
            .ok_or_else(|| Error::Measurement(format!("net `{name}` is not addressable")))
    }

    /// Net voltage `a` minus `b` (ground-referenced when `b` is `None`) over
    /// time. A digital net read returns its logic value (0/1, NaN for X/Z) —
    /// the transient records a digital snapshot per step, so sequential
    /// logic is observable where a stateless operating point cannot.
    pub fn v(&self, a: &NetRef, b: Option<&NetRef>) -> Result<Waveform, Error> {
        if let Some(&idx) = self.info.digital_nets.get(&a.name) {
            use piperine_solver::prelude::LogicValue;
            let points = self
                .result
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
        let node_a = self.node_or_err(&a.name)?;
        let node_b = match b {
            Some(nb) => Some(self.node_or_err(&nb.name)?),
            None => None,
        };
        let points = self
            .result
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

    /// A branch current over time. Ideal sources (`<-`, `num_forces > 0`)
    /// read the exact MNA branch unknown per step. Other two-terminal
    /// devices (resistors, capacitors, nonlinear) are recomputed per step
    /// from the solved terminal voltages: the resistive part via
    /// `eval_residual`, the reactive part via `dQ/dt` of `eval_charge`
    /// (backward-Euler differentiation, consistent with the solver's own
    /// companion). Devices whose residual reads runtime state/vars (not
    /// recorded per step) fail loud.
    pub fn i(&self, a: &NetRef, b: Option<&NetRef>) -> Result<Waveform, Error> {
        let node_a = self.node_or_err(&a.name)?;
        let node_b = match b {
            Some(nb) => self.node_or_err(&nb.name)?,
            None => NodeIdentifier::Gnd,
        };
        let instance = crate::results::find_two_terminal_instance(&self.info, node_a.clone(), node_b.clone())?;
        if instance.num_forces > 0 {
            let branch = BranchIdentifier::new(instance.label.clone(), "force0".to_string());
            let points = self
                .result
                .iter()
                .map(|step| (step.time(), step.get_branch(branch.clone()).unwrap_or(0.0)))
                .collect();
            return Ok(Waveform::new(points));
        }
        // Fail loud for devices whose residual reads runtime state/vars not
        // recorded per step. `ddt` is reactive (charge), not state, so
        // R/C/nonlinear devices pass; `idt`/`delay` read state.
        let (_, state_read, vars_read) = instance.kernel.read_bounds();
        if state_read > 0 || vars_read > 0 {
            return Err(Error::Measurement(format!(
                "`i()` over time on `{}` is not yet recorded: the device reads runtime state/vars not captured per step",
                instance.label
            )));
        }
        // Resistive current (terminal-0 reference) + terminal-0 charge, per
        // step. The reactive current a→b is `sign * dQ_0/dt`; the resistive
        // is `sign * residual[0]` (same convention as `OpResult::i`).
        let sign = if instance.terminals[0] == node_a { 1.0 } else { -1.0 };
        let sim = piperine_codegen::SimCtx::default();
        let n = self.result.len();
        let mut t_series = Vec::with_capacity(n);
        let mut i_res = Vec::with_capacity(n);
        let mut q0 = Vec::with_capacity(n);
        for step in self.result.iter() {
            let volts: Vec<f64> = instance
                .terminals
                .iter()
                .map(|t| if *t == NodeIdentifier::Gnd { 0.0 } else { step.get_node(t).unwrap_or(0.0) })
                .collect();
            let mut residual = vec![0.0; instance.terminals.len()];
            instance
                .kernel
                .eval_residual(&volts, &instance.params, &[], &[], &sim, &mut residual);
            let mut charge = vec![0.0; instance.terminals.len()];
            instance
                .kernel
                .eval_charge(&volts, &instance.params, &[], &[], &sim, &mut charge);
            i_res.push(residual[0]);
            q0.push(charge[0]);
            t_series.push(step.time());
        }
        let mut points = Vec::with_capacity(n);
        for k in 0..n {
            // Backward-Euler dQ_0/dt; the first sample has no predecessor —
            // reuse the forward difference (or 0 for a single-step trace).
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

    /// The time axis as a real waveform.
    pub fn axis(&self) -> Waveform {
        let points = self.result.iter().map(|step| (step.time(), step.time())).collect();
        Waveform::new(points)
    }
}

// ─── AC: complex waveforms ─────────────────────────────────────────────────────

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

/// The result of an AC small-signal sweep: a frequency sweep whose `.v`
/// reads out complex waveforms.
pub struct AcTrace {
    result: piperine_solver::prelude::AcAnalysisResult,
    info: Rc<CircuitBuildInfo>,
}

impl std::fmt::Debug for AcTrace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AcTrace").finish_non_exhaustive()
    }
}

impl AcTrace {
    pub fn new(result: piperine_solver::prelude::AcAnalysisResult, info: Rc<CircuitBuildInfo>) -> Self {
        Self { result, info }
    }

    /// Resolve a host-visible net name to a solver node.
    fn node_or_err(&self, name: &str) -> Result<NodeIdentifier, Error> {
        self.info
            .net_node(name)
            .ok_or_else(|| Error::Measurement(format!("net `{name}` is not addressable")))
    }

    /// Net voltage `a` minus `b` (ground-referenced when `b` is `None`) over
    /// the AC frequency sweep.
    pub fn v(&self, a: &NetRef, b: Option<&NetRef>) -> Result<ComplexWaveform, Error> {
        let node_a = self.node_or_err(&a.name)?;
        let node_b = match b {
            Some(nb) => Some(self.node_or_err(&nb.name)?),
            None => None,
        };
        let zero = num_complex::Complex64::default();
        let points = self
            .result
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
        let points = self.result.iter().map(|s| (s.frequency, s.frequency)).collect();
        Waveform::new(points)
    }
}

// ─── Noise ─────────────────────────────────────────────────────────────────────

/// The result of an output-referred noise analysis: noise PSD over frequency
/// plus the integrated total.
pub struct NoiseTrace {
    result: piperine_solver::prelude::NoiseAnalysisResult,
}

impl std::fmt::Debug for NoiseTrace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NoiseTrace").finish_non_exhaustive()
    }
}

impl NoiseTrace {
    pub fn new(result: piperine_solver::prelude::NoiseAnalysisResult) -> Self {
        Self { result }
    }

    /// Output-referred noise PSD as `(frequency, v²/Hz)` samples.
    pub fn psd(&self) -> Waveform {
        Waveform::new(
            self.result
                .frequencies
                .iter()
                .zip(&self.result.out_noise_sq)
                .map(|(f, v)| (*f, *v))
                .collect(),
        )
    }

    /// The integrated total noise (RMS).
    pub fn total(&self) -> f64 {
        self.result.integrated_noise
    }
}
