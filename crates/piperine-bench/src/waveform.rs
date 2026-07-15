//! [`Waveform`] and [`Trace`] — the `$tran` result surface (piperine-bench/docs/SPEC.md
//! §6): a swept analysis returns a [`Trace`], and `.v`/`.i` on it read out
//! a [`Waveform`] over the analysis axis (time, for `$tran`).

use std::any::Any;
use std::rc::Rc;

use piperine_codegen::device::CircuitBuildInfo;
use piperine_lang::eval::{Closure, EvalError, Object, Value};
use piperine_solver::analog::{BranchIdentifier, NodeIdentifier};
use piperine_solver::analysis::transient::TransientAnalysisResult;

use crate::objects::{NetLookup, NetRef};

/// A series of `(axis, value)` samples — one measured quantity over an
/// analysis axis (piperine-bench/docs/SPEC.md §6.1). Points are assumed sorted by axis
/// (true for every analysis this crate runs). `Waveform` (= `Waveform<f64>`)
/// is the `$tran` real surface; [`ComplexWaveform`] (= `Waveform<Complex64>`)
/// is the `$ac` surface — one struct, two instantiations.
#[derive(Debug, Clone)]
pub struct Waveform<T = f64> {
    points: Vec<(f64, T)>,
}

/// `Waveform<Complex>` (bench spec §6): the `$ac` result samples. Scalar
/// reductions live on the `Real` projections returned by `mag`/`phase`/`db`.
pub type ComplexWaveform = Waveform<num_complex::Complex64>;

impl<T: Copy> Waveform<T> {
    pub fn new(points: Vec<(f64, T)>) -> Self {
        Self { points }
    }

    /// The raw `(axis, value)` samples — the numpy seam (PY-08): the Python
    /// binding splits this into two `np.ndarray`s of equal length.
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

    /// `points()` as a bench `List` of `(axis, value)` tuples.
    fn points_value(&self, to_value: impl Fn(T) -> Value) -> Value {
        Value::List(Rc::new(std::cell::RefCell::new(
            self.points
                .iter()
                .map(|&(x, v)| Value::Tuple(vec![Value::Real(x), to_value(v)]))
                .collect(),
        )))
    }

    /// `map(f)` (bench spec §6): apply `f` to each value, keeping the axis.
    /// An all-`Real` result is a `Waveform`; any `Complex` result widens the
    /// whole series to a `ComplexWaveform`. Shared by both instantiations —
    /// only the argument conversion (`to_value`) differs.
    fn map_with(
        &self,
        to_value: impl Fn(T) -> Value,
        invoke: &mut piperine_lang::eval::InvokeClosure<'_>,
        args: Vec<Value>,
    ) -> Result<Value, EvalError> {
        let f = args.only_closure()?;
        let mut reals = Vec::with_capacity(self.points.len());
        let mut complex = Vec::with_capacity(self.points.len());
        let mut all_real = true;
        for &(x, v) in &self.points {
            match invoke(&f, vec![to_value(v)])? {
                Value::Real(r) => {
                    reals.push((x, r));
                    complex.push((x, num_complex::Complex64::new(r, 0.0)));
                }
                Value::Complex(re, im) => {
                    all_real = false;
                    complex.push((x, num_complex::Complex64::new(re, im)));
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
}

/// Fixed-width sample table for `$display` (`Object::render`): a header,
/// one row per sample, long series elided in the middle. Shared by
/// [`Waveform`] and [`ComplexWaveform`].
pub(crate) struct SampleTable {
    header: [&'static str; 2],
    rows: Vec<(f64, String)>,
}

impl SampleTable {
    const HEAD: usize = 12;
    const TAIL: usize = 4;

    pub(crate) fn new(header: [&'static str; 2], rows: Vec<(f64, String)>) -> Self {
        Self { header, rows }
    }

    pub(crate) fn render(&self) -> String {
        let mut out = format!("\n{:>16}  {:>16}\n", self.header[0], self.header[1]);
        let elide = self.rows.len() > Self::HEAD + Self::TAIL + 1;
        for (i, (x, v)) in self.rows.iter().enumerate() {
            if elide && i == Self::HEAD {
                out.push_str(&format!(
                    "{:>16}  {:>16}\n",
                    "...",
                    format!("({} rows)", self.rows.len() - Self::HEAD - Self::TAIL)
                ));
            }
            if elide && i >= Self::HEAD && i < self.rows.len() - Self::TAIL {
                continue;
            }
            out.push_str(&format!("{:>16.6e}  {:>16}\n", x, v));
        }
        out
    }
}

impl Waveform {
    /// Linear interpolation at `x`; clamps to the first/last sample outside
    /// the recorded range. Public typed seam for the Python binding (PY-08).
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

impl Object for Waveform {
    fn type_name(&self) -> &str {
        "Waveform"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn render(&self) -> String {
        let rows = self
            .points
            .iter()
            .map(|&(x, v)| (x, format!("{v:.6e}")))
            .collect();
        SampleTable::new(["axis", "value"], rows).render()
    }
    fn call_method(&self, name: &str, args: Vec<Value>) -> Result<Value, EvalError> {
        match name {
            "at" => Ok(Value::Real(self.at(args.real(0)?))),
            "min" => Ok(Value::Real(self.min())),
            "max" => Ok(Value::Real(self.max())),
            "mean" => Ok(Value::Real(self.mean())),
            "rms" => Ok(Value::Real(self.rms())),
            "peak_to_peak" => Ok(Value::Real(self.peak_to_peak())),
            "len" => Ok(Value::Nat(self.len() as u64)),
            "points" => Ok(self.points_value(Value::Real)),
            "fft" => Ok(Value::Object(Rc::new(self.fft()))),
            "rise_time" => {
                let lo = args.real(0)?;
                let hi = args.real(1)?;
                Ok(Value::Option(self.rise_time(lo, hi).map(|t| Box::new(Value::Real(t)))))
            }
            "fall_time" => {
                let hi = args.real(0)?;
                let lo = args.real(1)?;
                Ok(Value::Option(self.fall_time(hi, lo).map(|t| Box::new(Value::Real(t)))))
            }
            "cross" => {
                let level = args.real(0)?;
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
        invoke: &mut piperine_lang::eval::InvokeClosure<'_>,
    ) -> Result<Value, EvalError> {
        match name {
            "map" => self.map_with(Value::Real, invoke, args),
            _ => self.call_method(name, args),
        }
    }
}

/// Argument-list accessors shared by the `call_method` impls in this module
/// — the owner of what were loose `as_real`/`one_closure` helpers.
trait ValueArgs {
    /// The `i`-th argument coerced to `Real`.
    fn real(&self, i: usize) -> Result<f64, EvalError>;
    /// Exactly one argument, which must be a closure.
    fn only_closure(&self) -> Result<Rc<Closure>, EvalError>;
}

impl ValueArgs for [Value] {
    fn real(&self, i: usize) -> Result<f64, EvalError> {
        self.get(i)
            .ok_or_else(|| EvalError::TypeMismatch("expected a Real argument".into()))?
            .coerce_real()
    }

    fn only_closure(&self) -> Result<Rc<Closure>, EvalError> {
        match self {
            [Value::Closure(c)] => Ok(c.clone()),
            [other] => Err(EvalError::TypeMismatch(format!(
                "expected a closure, got {}",
                other.type_name()
            ))),
            _ => Err(EvalError::TypeMismatch(format!(
                "expected 1 argument, got {}",
                self.len()
            ))),
        }
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

    /// Resolve a bench-visible net name to a solver node — the typed form of
    /// the value-layer `NetLookup::node_arg`.
    fn node_or_err(&self, name: &str) -> Result<NodeIdentifier, EvalError> {
        self.info
            .net_node(name)
            .ok_or_else(|| EvalError::Undefined(format!("net `{name}` is not addressable")))
    }

    /// Net voltage `a` minus `b` (ground-referenced when `b` is `None`) over
    /// time. A digital net read returns its logic value (0/1, NaN for X/Z) —
    /// the transient records a digital snapshot per step, so sequential logic
    /// is observable through `$tran` where `$op` (stateless) cannot. Public
    /// typed seam for the Python binding (PY-07).
    pub fn v(&self, a: &NetRef, b: Option<&NetRef>) -> Result<Waveform, EvalError> {
        if let Some(&idx) = self.info.digital_nets.get(&a.name) {
            use piperine_solver::digital::LogicValue;
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

    /// Value-layer dispatch wrapper kept for `impl Object`.
    fn v_value(&self, args: &[Value]) -> Result<Value, EvalError> {
        let first = args.first().ok_or_else(|| EvalError::TypeMismatch("v() needs at least 1 argument".into()))?;
        let a = NetRef::from_value(first).ok_or_else(|| EvalError::TypeMismatch("expected a Net".into()))?;
        let b = match args.get(1) {
            Some(v) => Some(NetRef::from_value(v).ok_or_else(|| EvalError::TypeMismatch("expected a Net".into()))?),
            None => None,
        };
        self.v(a, b).map(|w| Value::Object(Rc::new(w)))
    }

    /// A branch current over time (piperine-bench/docs/SPEC.md §4/§6). Ideal sources
    /// (`<-`, `num_forces > 0`) read the exact MNA branch unknown per step.
    /// Other two-terminal devices (resistors, capacitors, nonlinear) are
    /// recomputed per step from the solved terminal voltages: the resistive
    /// part via `eval_residual`, the reactive part via `dQ/dt` of
    /// `eval_charge` (backward-Euler differentiation, consistent with the
    /// solver's own companion). Devices whose residual reads runtime
    /// state/vars (not recorded per step) fail loud (G3 milestone split).
    /// Public typed seam for the Python binding (PY-07).
    pub fn i(&self, a: &NetRef, b: Option<&NetRef>) -> Result<Waveform, EvalError> {
        let node_a = self.node_or_err(&a.name)?;
        let node_b = match b {
            Some(nb) => self.node_or_err(&nb.name)?,
            None => NodeIdentifier::Gnd,
        };
        let instance = crate::objects::find_two_terminal_instance(&self.info, node_a.clone(), node_b.clone())?;
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

    /// Value-layer dispatch wrapper kept for `impl Object`.
    fn i_value(&self, args: &[Value]) -> Result<Value, EvalError> {
        if args.is_empty() || args.len() > 2 {
            return Err(EvalError::TypeMismatch("i() takes 1 or 2 arguments".into()));
        }
        let a = NetRef::from_value(&args[0]).ok_or_else(|| EvalError::TypeMismatch("expected a Net".into()))?;
        let b = match args.get(1) {
            Some(v) => Some(NetRef::from_value(v).ok_or_else(|| EvalError::TypeMismatch("expected a Net".into()))?),
            None => None,
        };
        self.i(a, b).map(|w| Value::Object(Rc::new(w)))
    }

    /// The time axis as a real waveform (the `.axis` bench method). Public
    /// typed seam for the Python binding (PY-07).
    pub fn axis(&self) -> Waveform {
        let points = self.result.iter().map(|step| (step.time(), step.time())).collect();
        Waveform::new(points)
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
            "v" => self.v_value(&args),
            "i" => self.i_value(&args),
            "axis" => Ok(Value::Object(Rc::new(self.axis()))),
            other => Err(EvalError::Undefined(format!("method `{other}` on Trace"))),
        }
    }
}

// ─── AC: complex waveforms ─────────────────────────────────────────────────────

impl ComplexWaveform {
    fn project(&self, f: impl Fn(&num_complex::Complex64) -> f64) -> Waveform {
        Waveform::new(self.points.iter().map(|(x, c)| (*x, f(c))).collect())
    }

    /// Magnitude projection `|c|` per sample. Public typed seam (PY-09).
    pub fn mag(&self) -> Waveform {
        self.project(|c| c.norm())
    }
    /// Phase projection `arg(c)` (radians) per sample. Public typed seam (PY-09).
    pub fn phase(&self) -> Waveform {
        self.project(|c| c.arg())
    }
    /// Decibel projection `20·log10|c|` per sample. Public typed seam (PY-09).
    pub fn db(&self) -> Waveform {
        self.project(|c| 20.0 * c.norm().log10())
    }
    /// Nearest sample to `x` (no complex interpolation). Public typed seam (PY-09).
    pub fn at(&self, x: f64) -> num_complex::Complex64 {
        self.points
            .iter()
            .min_by(|a, b| (a.0 - x).abs().total_cmp(&(b.0 - x).abs()))
            .map(|(_, c)| *c)
            .unwrap_or_default()
    }
}

impl Object for ComplexWaveform {
    fn type_name(&self) -> &str {
        "Waveform"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn render(&self) -> String {
        let rows = self
            .points
            .iter()
            .map(|&(x, c)| (x, format!("{:.6e} {:+.6e}j", c.re, c.im)))
            .collect();
        SampleTable::new(["axis", "value"], rows).render()
    }
    fn call_method(&self, name: &str, args: Vec<Value>) -> Result<Value, EvalError> {
        match name {
            "mag" => Ok(Value::Object(Rc::new(self.mag()))),
            "phase" => Ok(Value::Object(Rc::new(self.phase()))),
            "db" => Ok(Value::Object(Rc::new(self.db()))),
            "len" => Ok(Value::Nat(self.len() as u64)),
            "at" => {
                // Nearest sample (no complex interpolation): SPEC §6 `at`
                // on a Complex waveform.
                let x = args.real(0)?;
                let c = self.at(x);
                Ok(Value::Complex(c.re, c.im))
            }
            "points" => Ok(self.points_value(|c| Value::Complex(c.re, c.im))),
            other => Err(EvalError::Undefined(format!("method `{other}` on Waveform<Complex>"))),
        }
    }

    fn call_method_with(
        &self,
        name: &str,
        args: Vec<Value>,
        invoke: &mut piperine_lang::eval::InvokeClosure<'_>,
    ) -> Result<Value, EvalError> {
        match name {
            "map" => self.map_with(|c| Value::Complex(c.re, c.im), invoke, args),
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

    /// Resolve a bench-visible net name to a solver node — the typed form of
    /// the value-layer `NetLookup::node_arg`.
    fn node_or_err(&self, name: &str) -> Result<NodeIdentifier, EvalError> {
        self.info
            .net_node(name)
            .ok_or_else(|| EvalError::Undefined(format!("net `{name}` is not addressable")))
    }

    /// Net voltage `a` minus `b` (ground-referenced when `b` is `None`) over
    /// the AC frequency sweep. Public typed seam for the Python binding (PY-09).
    pub fn v(&self, a: &NetRef, b: Option<&NetRef>) -> Result<ComplexWaveform, EvalError> {
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

    /// Value-layer dispatch wrapper kept for `impl Object`.
    fn v_value(&self, args: &[Value]) -> Result<Value, EvalError> {
        let first = args.first().ok_or_else(|| EvalError::TypeMismatch("v() needs at least 1 argument".into()))?;
        let a = NetRef::from_value(first).ok_or_else(|| EvalError::TypeMismatch("expected a Net".into()))?;
        let b = match args.get(1) {
            Some(v) => Some(NetRef::from_value(v).ok_or_else(|| EvalError::TypeMismatch("expected a Net".into()))?),
            None => None,
        };
        self.v(a, b).map(|w| Value::Object(Rc::new(w)))
    }

    /// The frequency axis as a real waveform. Public typed seam (PY-09).
    pub fn axis(&self) -> Waveform {
        let points = self.result.iter().map(|s| (s.frequency, s.frequency)).collect();
        Waveform::new(points)
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
            "v" => self.v_value(&args),
            "axis" => Ok(Value::Object(Rc::new(self.axis()))),
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

    /// Output-referred noise PSD as `(frequency, v²/Hz)` samples. Public typed
    /// seam for the Python binding (PY-10).
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

    /// The integrated total noise (RMS). Public typed seam (PY-10).
    pub fn total(&self) -> f64 {
        self.result.integrated_noise
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
            "psd" => Ok(Value::Object(Rc::new(self.psd()))),
            "total" => Ok(Value::Real(self.total())),
            other => Err(EvalError::Undefined(format!("method `{other}` on NoiseTrace"))),
        }
    }
}
