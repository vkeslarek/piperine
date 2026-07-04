//! [`Waveform`] and [`Trace`] — the `$tran` result surface (piperine-bench/docs/SPEC.md
//! §6): a swept analysis returns a [`Trace`], and `.v`/`.i` on it read out
//! a [`Waveform`] over the analysis axis (time, for `$tran`).

use std::any::Any;
use std::rc::Rc;

use piperine_codegen::device::CircuitBuildInfo;
use piperine_lang::eval::{Closure, EvalError, Object, Value};
use piperine_solver::analog::{BranchIdentifier, NodeIdentifier};
use piperine_solver::analysis::transient::TransientAnalysisResult;

use crate::objects::NetRef;

/// A series of `(axis, value)` samples — one measured quantity over time
/// (piperine-bench/docs/SPEC.md §6.1). Points are assumed sorted by axis (true for every
/// analysis this crate runs).
#[derive(Debug, Clone)]
pub struct Waveform {
    points: Vec<(f64, f64)>,
}

impl Waveform {
    pub fn new(points: Vec<(f64, f64)>) -> Self {
        Self { points }
    }

    /// Linear interpolation at `x`; clamps to the first/last sample outside
    /// the recorded range.
    fn at(&self, x: f64) -> f64 {
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

    fn min(&self) -> f64 {
        self.points.iter().map(|(_, v)| *v).fold(f64::INFINITY, f64::min)
    }
    fn max(&self) -> f64 {
        self.points.iter().map(|(_, v)| *v).fold(f64::NEG_INFINITY, f64::max)
    }
    fn mean(&self) -> f64 {
        if self.points.is_empty() {
            return 0.0;
        }
        self.points.iter().map(|(_, v)| *v).sum::<f64>() / self.points.len() as f64
    }
    fn rms(&self) -> f64 {
        if self.points.is_empty() {
            return 0.0;
        }
        (self.points.iter().map(|(_, v)| v * v).sum::<f64>() / self.points.len() as f64).sqrt()
    }
    fn peak_to_peak(&self) -> f64 {
        self.max() - self.min()
    }

    /// Time for the waveform to rise from `lo` to `hi` (first rising
    /// crossings, `hi` after `lo`) — `None` if either never happens.
    fn rise_time(&self, lo: f64, hi: f64) -> Option<f64> {
        let t_lo = self.cross(lo, "Rising")?;
        let after: Vec<(f64, f64)> = self.points.iter().copied().filter(|(t, _)| *t >= t_lo).collect();
        let t_hi = Waveform::new(after).cross(hi, "Rising")?;
        Some(t_hi - t_lo)
    }

    /// Time for the waveform to fall from `hi` to `lo` (first falling
    /// crossings, `lo` after `hi`) — `None` if either never happens.
    fn fall_time(&self, hi: f64, lo: f64) -> Option<f64> {
        let t_hi = self.cross(hi, "Falling")?;
        let after: Vec<(f64, f64)> = self.points.iter().copied().filter(|(t, _)| *t >= t_hi).collect();
        let t_lo = Waveform::new(after).cross(lo, "Falling")?;
        Some(t_lo - t_hi)
    }

    /// Single-sided DFT (bench spec §6 `fft()`): resamples onto a uniform
    /// grid (adaptive transient steps are non-uniform), then a direct
    /// O(n²) transform — bins `k = 0..n/2` at `f_k = k / (n·dt)`. Fine for
    /// bench-sized traces; a windowed FFT is library work over `points()`.
    fn fft(&self) -> ComplexWaveform {
        let n = self.points.len();
        if n < 2 {
            return ComplexWaveform::new(vec![]);
        }
        let t0 = self.points[0].0;
        let t1 = self.points[n - 1].0;
        let dt = (t1 - t0) / (n - 1) as f64;
        let samples: Vec<f64> = (0..n).map(|i| self.at(t0 + i as f64 * dt)).collect();
        let mut bins = Vec::with_capacity(n / 2 + 1);
        for k in 0..=(n / 2) {
            let mut acc = num_complex::Complex64::default();
            for (i, &x) in samples.iter().enumerate() {
                let phi = -2.0 * std::f64::consts::PI * (k * i) as f64 / n as f64;
                acc += num_complex::Complex64::new(phi.cos(), phi.sin()) * x;
            }
            bins.push((k as f64 / (n as f64 * dt), acc / n as f64));
        }
        ComplexWaveform::new(bins)
    }

    /// First axis value where the waveform crosses `level`, in direction
    /// `dir` (`CrossDir::Rising`/`Falling`/`Either`). `None` if it never
    /// does.
    fn cross(&self, level: f64, dir: &str) -> Option<f64> {
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

impl Object for Waveform {
    fn type_name(&self) -> &str {
        "Waveform"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn call_method(&self, name: &str, args: Vec<Value>) -> Result<Value, EvalError> {
        match name {
            "at" => Ok(Value::Real(self.at(as_real(args.first())?))),
            "min" => Ok(Value::Real(self.min())),
            "max" => Ok(Value::Real(self.max())),
            "mean" => Ok(Value::Real(self.mean())),
            "rms" => Ok(Value::Real(self.rms())),
            "peak_to_peak" => Ok(Value::Real(self.peak_to_peak())),
            "len" => Ok(Value::Nat(self.points.len() as u64)),
            "points" => Ok(Value::List(Rc::new(std::cell::RefCell::new(
                self.points.iter().map(|(t, v)| Value::Tuple(vec![Value::Real(*t), Value::Real(*v)])).collect(),
            )))),
            "fft" => Ok(Value::Object(Rc::new(self.fft()))),
            "rise_time" => {
                let lo = as_real(args.first())?;
                let hi = as_real(args.get(1))?;
                Ok(Value::Option(self.rise_time(lo, hi).map(|t| Box::new(Value::Real(t)))))
            }
            "fall_time" => {
                let hi = as_real(args.first())?;
                let lo = as_real(args.get(1))?;
                Ok(Value::Option(self.fall_time(hi, lo).map(|t| Box::new(Value::Real(t)))))
            }
            "cross" => {
                let level = as_real(args.first())?;
                let dir = match args.get(1) {
                    Some(Value::EnumVariant(_, variant)) => variant.clone(),
                    Some(Value::Str(s)) => s.clone(),
                    _ => "Either".to_string(),
                };
                Ok(Value::Option(self.cross(level, &dir).map(|t| Box::new(Value::Real(t)))))
            }
            other => Err(EvalError::Undefined(format!("method `{other}` on Waveform"))),
        }
    }

    fn call_method_with(
        &self,
        name: &str,
        args: Vec<Value>,
        invoke: &mut dyn FnMut(&Closure, Vec<Value>) -> Result<Value, EvalError>,
    ) -> Result<Value, EvalError> {
        match name {
            // `Waveform<Real>::map(f: fn(Real) -> U) -> Waveform<U>` (piperine-bench/docs/SPEC.md
            // §6): apply `f` to each value, keeping the axis. A Real result
            // stays a `Waveform`; a Complex result widens to `ComplexWaveform`.
            "map" => {
                let f = one_closure(args)?;
                let mut reals = Vec::with_capacity(self.points.len());
                let mut complex = Vec::with_capacity(self.points.len());
                let mut all_real = true;
                for (t, v) in &self.points {
                    match invoke(&f, vec![Value::Real(*v)])? {
                        Value::Real(r) => {
                            reals.push((*t, r));
                            complex.push((*t, num_complex::Complex64::new(r, 0.0)));
                        }
                        Value::Complex(re, im) => {
                            all_real = false;
                            complex.push((*t, num_complex::Complex64::new(re, im)));
                        }
                        other => {
                            return Err(EvalError::TypeMismatch(format!(
                                "Waveform.map: closure must return Real or Complex, got {}",
                                other.type_name()
                            )));
                        }
                    }
                }
                if all_real {
                    Ok(Value::Object(Rc::new(Waveform::new(reals))))
                } else {
                    Ok(Value::Object(Rc::new(ComplexWaveform::new(complex))))
                }
            }
            _ => self.call_method(name, args),
        }
    }
}

/// Extract the single closure argument of a callback-taking method.
fn one_closure(mut args: Vec<Value>) -> Result<Rc<Closure>, EvalError> {
    if args.len() != 1 {
        return Err(EvalError::TypeMismatch(format!(
            "expected 1 argument, got {}",
            args.len()
        )));
    }
    match args.remove(0) {
        Value::Closure(c) => Ok(c),
        other => Err(EvalError::TypeMismatch(format!(
            "expected a closure, got {}",
            other.type_name()
        ))),
    }
}

fn as_real(v: Option<&Value>) -> Result<f64, EvalError> {
    match v {
        Some(Value::Real(r)) => Ok(*r),
        Some(Value::Nat(n)) => Ok(*n as f64),
        Some(Value::Int(n)) => Ok(*n as f64),
        _ => Err(EvalError::TypeMismatch("expected a Real argument".into())),
    }
}

/// The result of `$tran(stop, step)` (piperine-bench/docs/SPEC.md §5/§6): a swept
/// transient result, read by name into a [`Waveform`] per branch/node.
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

    fn resolve_node(&self, arg: &Value) -> Result<NodeIdentifier, EvalError> {
        match arg {
            Value::Object(obj) => {
                let net = obj
                    .as_any()
                    .downcast_ref::<NetRef>()
                    .ok_or_else(|| EvalError::TypeMismatch(format!("expected a Net, got {}", obj.type_name())))?;
                if net.name == "gnd" || net.name == "GND" || net.name == "vss" || net.name == "VSS" {
                    return Ok(NodeIdentifier::Gnd);
                }
                self.info
                    .nets
                    .get(&net.name)
                    .cloned()
                    .ok_or_else(|| EvalError::Undefined(format!("net `{}` is not addressable", net.name)))
            }
            other => Err(EvalError::TypeMismatch(format!("expected a Net, got {}", other.type_name()))),
        }
    }

    fn v(&self, args: &[Value]) -> Result<Value, EvalError> {
        let a = self.resolve_node(args.first().ok_or_else(|| EvalError::TypeMismatch("v() needs at least 1 argument".into()))?)?;
        let b = match args.get(1) {
            Some(v) => Some(self.resolve_node(v)?),
            None => None,
        };
        let points = self
            .result
            .iter()
            .map(|step| {
                let va = if a == NodeIdentifier::Gnd { 0.0 } else { step.get_node(&a).unwrap_or(0.0) };
                let vb = match &b {
                    Some(b) if *b == NodeIdentifier::Gnd => 0.0,
                    Some(b) => step.get_node(b).unwrap_or(0.0),
                    None => 0.0,
                };
                (step.time(), va - vb)
            })
            .collect();
        Ok(Value::Object(Rc::new(Waveform::new(points))))
    }

    /// A branch current over time (piperine-bench/docs/SPEC.md §4/§6). Ideal sources
    /// (`<-`, `num_forces > 0`) read the exact MNA branch unknown per step.
    /// Other two-terminal devices (resistors, capacitors, nonlinear) are
    /// recomputed per step from the solved terminal voltages: the resistive
    /// part via `eval_residual`, the reactive part via `dQ/dt` of
    /// `eval_charge` (backward-Euler differentiation, consistent with the
    /// solver's own companion). Devices whose residual reads runtime
    /// state/vars (not recorded per step) fail loud (G3 milestone split).
    fn i(&self, args: &[Value]) -> Result<Value, EvalError> {
        if args.is_empty() || args.len() > 2 {
            return Err(EvalError::TypeMismatch("i() takes 1 or 2 arguments".into()));
        }
        let a = self.resolve_node(&args[0])?;
        let b = match args.get(1) {
            Some(v) => self.resolve_node(v)?,
            None => NodeIdentifier::Gnd,
        };
        let instance = crate::objects::find_two_terminal_instance(&self.info, a.clone(), b.clone())?;
        if instance.num_forces > 0 {
            let branch = BranchIdentifier::new(instance.label.clone(), "force0".to_string());
            let points = self
                .result
                .iter()
                .map(|step| (step.time(), step.get_branch(branch.clone()).unwrap_or(0.0)))
                .collect();
            return Ok(Value::Object(Rc::new(Waveform::new(points))));
        }
        // Fail loud for devices whose residual reads runtime state/vars not
        // recorded per step (G3 milestone split). `ddt` is reactive (charge),
        // not state, so R/C/nonlinear devices pass; `idt`/`delay` read state.
        let (_, state_read, vars_read) = instance.kernel.read_bounds();
        if state_read > 0 || vars_read > 0 {
            return Err(EvalError::Host(format!(
                "`i()` over time on `{}` is not yet recorded: the device reads runtime state/vars not captured per step",
                instance.label
            )));
        }
        // Resistive current (terminal-0 reference) + terminal-0 charge, per
        // step. The reactive current a→b is `sign * dQ_0/dt`; the resistive
        // is `sign * residual[0]` (same convention as `OpResult::i`).
        let sign = if instance.terminals[0] == a { 1.0 } else { -1.0 };
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
        Ok(Value::Object(Rc::new(Waveform::new(points))))
    }

    fn axis(&self) -> Value {
        let points = self.result.iter().map(|step| (step.time(), step.time())).collect();
        Value::Object(Rc::new(Waveform::new(points)))
    }
}

impl Object for Trace {
    fn type_name(&self) -> &str {
        "Trace"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn call_method(&self, name: &str, args: Vec<Value>) -> Result<Value, EvalError> {
        match name {
            "v" => self.v(&args),
            "i" => self.i(&args),
            "axis" => Ok(self.axis()),
            other => Err(EvalError::Undefined(format!("method `{other}` on Trace"))),
        }
    }
}

// ─── AC: complex waveforms ─────────────────────────────────────────────────────

/// A series of `(frequency, Complex)` samples — bench spec §6
/// `Waveform<Complex>`. Scalar reductions live on the `Real` projections
/// returned by [`mag`](Self)/[`phase`](Self)/[`db`](Self).
#[derive(Debug, Clone)]
pub struct ComplexWaveform {
    points: Vec<(f64, num_complex::Complex64)>,
}

impl ComplexWaveform {
    pub fn new(points: Vec<(f64, num_complex::Complex64)>) -> Self {
        Self { points }
    }

    fn project(&self, f: impl Fn(&num_complex::Complex64) -> f64) -> Waveform {
        Waveform::new(self.points.iter().map(|(x, c)| (*x, f(c))).collect())
    }
}

impl Object for ComplexWaveform {
    fn type_name(&self) -> &str {
        "Waveform"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn call_method(&self, name: &str, args: Vec<Value>) -> Result<Value, EvalError> {
        match name {
            "mag" => Ok(Value::Object(Rc::new(self.project(|c| c.norm())))),
            "phase" => Ok(Value::Object(Rc::new(self.project(|c| c.arg())))),
            "db" => Ok(Value::Object(Rc::new(self.project(|c| 20.0 * c.norm().log10())))),
            "len" => Ok(Value::Nat(self.points.len() as u64)),
            "at" => {
                // Nearest sample (no complex interpolation): SPEC §6 `at`
                // on a Complex waveform.
                let x = as_real(args.first())?;
                let c = self
                    .points
                    .iter()
                    .min_by(|a, b| (a.0 - x).abs().total_cmp(&(b.0 - x).abs()))
                    .map(|(_, c)| *c)
                    .unwrap_or_default();
                Ok(Value::Complex(c.re, c.im))
            }
            "points" => Ok(Value::List(Rc::new(std::cell::RefCell::new(
                self.points
                    .iter()
                    .map(|(x, c)| Value::Tuple(vec![Value::Real(*x), Value::Complex(c.re, c.im)]))
                    .collect(),
            )))),
            other => Err(EvalError::Undefined(format!("method `{other}` on Waveform<Complex>"))),
        }
    }

    fn call_method_with(
        &self,
        name: &str,
        args: Vec<Value>,
        invoke: &mut dyn FnMut(&Closure, Vec<Value>) -> Result<Value, EvalError>,
    ) -> Result<Value, EvalError> {
        match name {
            // `Waveform<Complex>::map(f: fn(Complex) -> U) -> Waveform<U>`
            // (piperine-bench/docs/SPEC.md §6): apply `f` to each complex value, keeping
            // the axis. A Real result projects to a `Waveform`; a Complex
            // result stays a `ComplexWaveform`.
            "map" => {
                let f = one_closure(args)?;
                let mut reals = Vec::with_capacity(self.points.len());
                let mut complex = Vec::with_capacity(self.points.len());
                let mut all_real = true;
                for (t, c) in &self.points {
                    match invoke(&f, vec![Value::Complex(c.re, c.im)])? {
                        Value::Real(r) => {
                            reals.push((*t, r));
                            complex.push((*t, num_complex::Complex64::new(r, 0.0)));
                        }
                        Value::Complex(re, im) => {
                            all_real = false;
                            complex.push((*t, num_complex::Complex64::new(re, im)));
                        }
                        other => {
                            return Err(EvalError::TypeMismatch(format!(
                                "Waveform.map: closure must return Real or Complex, got {}",
                                other.type_name()
                            )));
                        }
                    }
                }
                if all_real {
                    Ok(Value::Object(Rc::new(Waveform::new(reals))))
                } else {
                    Ok(Value::Object(Rc::new(ComplexWaveform::new(complex))))
                }
            }
            _ => self.call_method(name, args),
        }
    }
}

/// The result of `$ac(cfg)` (bench spec §5/§6): a frequency sweep whose
/// `.v`/`.i` read out complex waveforms.
pub struct AcTrace {
    result: piperine_solver::analysis::ac::AcAnalysisResult,
    info: Rc<CircuitBuildInfo>,
}

impl std::fmt::Debug for AcTrace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AcTrace").finish_non_exhaustive()
    }
}

impl AcTrace {
    pub fn new(result: piperine_solver::analysis::ac::AcAnalysisResult, info: Rc<CircuitBuildInfo>) -> Self {
        Self { result, info }
    }

    fn resolve_node(&self, arg: &Value) -> Result<NodeIdentifier, EvalError> {
        match arg {
            Value::Object(obj) => {
                let net = obj
                    .as_any()
                    .downcast_ref::<NetRef>()
                    .ok_or_else(|| EvalError::TypeMismatch(format!("expected a Net, got {}", obj.type_name())))?;
                if net.name == "gnd" {
                    return Ok(NodeIdentifier::Gnd);
                }
                self.info
                    .nets
                    .get(&net.name)
                    .cloned()
                    .ok_or_else(|| EvalError::Undefined(format!("net `{}` is not addressable", net.name)))
            }
            other => Err(EvalError::TypeMismatch(format!("expected a Net, got {}", other.type_name()))),
        }
    }

    fn v(&self, args: &[Value]) -> Result<Value, EvalError> {
        let a = self.resolve_node(
            args.first().ok_or_else(|| EvalError::TypeMismatch("v() needs at least 1 argument".into()))?,
        )?;
        let b = match args.get(1) {
            Some(v) => Some(self.resolve_node(v)?),
            None => None,
        };
        let zero = num_complex::Complex64::default();
        let points = self
            .result
            .iter()
            .map(|step| {
                let va = if a == NodeIdentifier::Gnd { zero } else { step.get_node(&a).copied().unwrap_or(zero) };
                let vb = match &b {
                    Some(b) if *b == NodeIdentifier::Gnd => zero,
                    Some(b) => step.get_node(b).copied().unwrap_or(zero),
                    None => zero,
                };
                (step.frequency, va - vb)
            })
            .collect();
        Ok(Value::Object(Rc::new(ComplexWaveform::new(points))))
    }

    fn axis(&self) -> Value {
        let points = self.result.iter().map(|s| (s.frequency, s.frequency)).collect();
        Value::Object(Rc::new(Waveform::new(points)))
    }
}

impl Object for AcTrace {
    fn type_name(&self) -> &str {
        "Trace"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn call_method(&self, name: &str, args: Vec<Value>) -> Result<Value, EvalError> {
        match name {
            "v" => self.v(&args),
            "axis" => Ok(self.axis()),
            other => Err(EvalError::Undefined(format!("method `{other}` on an AC Trace"))),
        }
    }
}

// ─── Noise ─────────────────────────────────────────────────────────────────────

/// The result of `$noise(out, cfg)` (bench spec §5/§6): output-referred
/// noise PSD over frequency plus the integrated total.
pub struct NoiseTrace {
    result: piperine_solver::analysis::noise::NoiseAnalysisResult,
}

impl std::fmt::Debug for NoiseTrace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NoiseTrace").finish_non_exhaustive()
    }
}

impl NoiseTrace {
    pub fn new(result: piperine_solver::analysis::noise::NoiseAnalysisResult) -> Self {
        Self { result }
    }
}

impl Object for NoiseTrace {
    fn type_name(&self) -> &str {
        "NoiseTrace"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn call_method(&self, name: &str, _args: Vec<Value>) -> Result<Value, EvalError> {
        match name {
            "psd" => Ok(Value::Object(Rc::new(Waveform::new(
                self.result
                    .frequencies
                    .iter()
                    .zip(&self.result.out_noise_sq)
                    .map(|(f, v)| (*f, *v))
                    .collect(),
            )))),
            "total" => Ok(Value::Real(self.result.integrated_noise)),
            other => Err(EvalError::Undefined(format!("method `{other}` on NoiseTrace"))),
        }
    }
}
