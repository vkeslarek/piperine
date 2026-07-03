//! [`Waveform`] and [`Trace`] — the `$tran` result surface (SPEC_BENCH.md
//! §6): a swept analysis returns a [`Trace`], and `.v`/`.i` on it read out
//! a [`Waveform`] over the analysis axis (time, for `$tran`).

use std::any::Any;
use std::rc::Rc;

use piperine_codegen::device::CircuitBuildInfo;
use piperine_lang::eval::{EvalError, Object, Value};
use piperine_solver::analog::{BranchIdentifier, NodeIdentifier};
use piperine_solver::analysis::transient::TransientAnalysisResult;

use crate::objects::NetRef;

/// A series of `(axis, value)` samples — one measured quantity over time
/// (SPEC_BENCH.md §6.1). Points are assumed sorted by axis (true for every
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
}

fn as_real(v: Option<&Value>) -> Result<f64, EvalError> {
    match v {
        Some(Value::Real(r)) => Ok(*r),
        Some(Value::Nat(n)) => Ok(*n as f64),
        Some(Value::Int(n)) => Ok(*n as f64),
        _ => Err(EvalError::TypeMismatch("expected a Real argument".into())),
    }
}

/// The result of `$tran(stop, step)` (SPEC_BENCH.md §5/§6): a swept
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

    /// A force-device branch current over time (SPEC_BENCH.md §4/§6): the
    /// instance-port form, restricted to devices with an MNA branch
    /// unknown (an ideal source, `<-`) — the general two-terminal residual
    /// read `OpResult::i` performs is DC-only (no reactive part), so it is
    /// not offered here.
    fn i(&self, args: &[Value]) -> Result<Value, EvalError> {
        if args.len() != 2 {
            return Err(EvalError::TypeMismatch("i() needs exactly 2 arguments".into()));
        }
        let a = self.resolve_node(&args[0])?;
        let b = self.resolve_node(&args[1])?;
        let instance = self
            .info
            .instances
            .iter()
            .find(|inst| {
                inst.num_forces > 0
                    && inst.terminals.len() == 2
                    && ((inst.terminals[0] == a && inst.terminals[1] == b) || (inst.terminals[0] == b && inst.terminals[1] == a))
            })
            .ok_or_else(|| {
                EvalError::TypeMismatch("no force-device (ideal source) branch connects those nets over time".into())
            })?;
        let points = self
            .result
            .iter()
            .map(|step| {
                let branch = BranchIdentifier::new(instance.label.clone(), "force0".to_string());
                (step.time(), step.get_branch(branch).unwrap_or(0.0))
            })
            .collect();
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
