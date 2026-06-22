//! ExternClass implementations for analysis result objects and signal vectors.
//!
//! These types are returned by Phase 3 analysis tasks (`$op()`, `$tran()`, `$ac()`, …)
//! and are accessible from Piperine testbenches via method calls:
//!
//! ```verilog
//! TranResult t = $tran(1e-9, 1e-3);
//! Signal vout = t.signal("v(out)");
//! real pk = vout.max();
//! ```

use std::sync::Arc;
use crate::value::{AnalysisResult, ExternClass, Value, VectorData};

/// Returned by all `$analysis()` tasks. Wraps `AnalysisResult` and exposes
/// `.signal()`, `.scale()`, `.ok()`, `.dataset()` methods.
#[derive(Debug)]
pub struct AnalysisHandleObj {
    pub result: Arc<AnalysisResult>,
    pub kind_name: &'static str,
}

impl AnalysisHandleObj {
    pub fn new(result: AnalysisResult, kind_name: &'static str) -> Value {
        Value::ExternObject(Arc::new(Self {
            result: Arc::new(result),
            kind_name,
        }))
    }
}

impl ExternClass for AnalysisHandleObj {
    fn type_name(&self) -> &str { self.kind_name }

    fn call_method(&self, method: &str, args: &[Value]) -> Result<Value, String> {
        match method {
            "dataset" => Ok(Value::String(self.result.dataset.clone())),

            "ok" => Ok(Value::Integer(self.result.run_errors.is_empty() as i64)),

            "signal" => {
                let name = args.first()
                    .and_then(|v| v.as_str())
                    .ok_or("signal(name: string) requires a string argument")?
                    .to_string();
                let data = self.result.vectors.get(&name)
                    .ok_or_else(|| format!("no vector '{}' in {} result", name, self.kind_name))?
                    .clone();
                Ok(Value::ExternObject(Arc::new(SignalObj {
                    name,
                    data,
                    result: Arc::clone(&self.result),
                })))
            }

            "scale" => {
                let scale_name = ["time", "frequency", "sweep"].iter()
                    .find(|&&k| self.result.vectors.contains_key(k))
                    .map(|s| s.to_string())
                    .or_else(|| self.result.vectors.keys().next().cloned())
                    .unwrap_or_default();
                let data = self.result.vectors.get(&scale_name)
                    .cloned()
                    .unwrap_or(VectorData::Real(vec![]));
                Ok(Value::ExternObject(Arc::new(SignalObj {
                    name: scale_name,
                    data,
                    result: Arc::clone(&self.result),
                })))
            }

            _ => Err(format!("unknown method '{}' on {}", method, self.kind_name)),
        }
    }
}

/// A named vector from an `AnalysisResult`. Provides signal measurement methods.
///
/// Obtained via `result.signal("v(out)")`. Supports:
/// - `.values()` — raw `real[$]` data
/// - `.max()`, `.min()`, `.mean()`, `.rms()`, `.peak_to_peak()`
/// - `.integral()` — trapezoidal (needs scale vector)
/// - `.bandwidth_3db()` — AC -3dB frequency
/// - `.phase_margin()` — AC phase at 0dB crossing
/// - `.at(x)` — interpolate at scale value `x`
/// - `.len()` — number of data points
#[derive(Debug)]
pub struct SignalObj {
    pub name: String,
    pub data: VectorData,
    pub result: Arc<AnalysisResult>,
}

impl ExternClass for SignalObj {
    fn type_name(&self) -> &str { "Signal" }

    fn call_method(&self, method: &str, args: &[Value]) -> Result<Value, String> {
        let reals: Vec<f64> = match &self.data {
            VectorData::Real(v) => v.clone(),
            VectorData::Complex(v) => v.iter().map(|(r, _)| *r).collect(),
        };

        match method {
            "values" => Ok(Value::RealVec(reals)),
            "len"    => Ok(Value::Integer(reals.len() as i64)),

            "max" => reals.iter().cloned().reduce(f64::max)
                .map(Value::Real)
                .ok_or_else(|| "max() on empty signal".into()),

            "min" => reals.iter().cloned().reduce(f64::min)
                .map(Value::Real)
                .ok_or_else(|| "min() on empty signal".into()),

            "mean" => {
                if reals.is_empty() { return Err("mean() on empty signal".into()); }
                Ok(Value::Real(reals.iter().sum::<f64>() / reals.len() as f64))
            }

            "rms" => {
                if reals.is_empty() { return Err("rms() on empty signal".into()); }
                let sum_sq: f64 = reals.iter().map(|x| x * x).sum();
                Ok(Value::Real((sum_sq / reals.len() as f64).sqrt()))
            }

            "peak_to_peak" => {
                if reals.is_empty() { return Err("peak_to_peak() on empty signal".into()); }
                let mx = reals.iter().cloned().reduce(f64::max).unwrap();
                let mn = reals.iter().cloned().reduce(f64::min).unwrap();
                Ok(Value::Real(mx - mn))
            }

            "integral" => {
                let scale = self.find_scale()
                    .ok_or("integral() requires a time/frequency scale vector")?;
                if scale.len() != reals.len() || reals.len() < 2 {
                    return Err("scale and signal length mismatch or too short for integral()".into());
                }
                let mut sum = 0.0f64;
                for i in 1..reals.len() {
                    let dt = scale[i] - scale[i - 1];
                    sum += 0.5 * (reals[i] + reals[i - 1]) * dt;
                }
                Ok(Value::Real(sum))
            }

            "bandwidth_3db" => {
                if reals.is_empty() { return Err("bandwidth_3db() on empty signal".into()); }
                let max_val = reals.iter().cloned().reduce(f64::max).unwrap();
                let threshold = max_val / std::f64::consts::SQRT_2;
                let scale = self.find_scale()
                    .ok_or("bandwidth_3db() requires a frequency scale vector")?;
                for i in 1..reals.len() {
                    if reals[i] < threshold {
                        let t = (threshold - reals[i - 1]) / (reals[i] - reals[i - 1]);
                        let f = scale[i - 1] + t * (scale[i] - scale[i - 1]);
                        return Ok(Value::Real(f));
                    }
                }
                Err("signal never crosses -3dB point in bandwidth_3db()".into())
            }

            "phase_margin" => {
                match &self.data {
                    VectorData::Complex(cv) => {
                        for i in 1..cv.len() {
                            let mag_prev = (cv[i-1].0.powi(2) + cv[i-1].1.powi(2)).sqrt();
                            let mag_curr = (cv[i].0.powi(2) + cv[i].1.powi(2)).sqrt();
                            if mag_prev >= 1.0 && mag_curr < 1.0 {
                                let phase = cv[i].1.atan2(cv[i].0).to_degrees();
                                return Ok(Value::Real(180.0 + phase));
                            }
                        }
                        Err("gain never crosses 0dB in phase_margin()".into())
                    }
                    VectorData::Real(_) => Err("phase_margin() requires complex AC vector data".into()),
                }
            }

            "at" => {
                let x = args.first().and_then(|v| v.as_f64())
                    .ok_or("at(x: real) requires a real argument")?;
                let scale = self.find_scale()
                    .ok_or("at() requires a time/frequency scale vector")?;
                for i in 1..scale.len() {
                    if scale[i] >= x {
                        let t = (x - scale[i - 1]) / (scale[i] - scale[i - 1]);
                        let val = reals[i - 1] + t * (reals[i] - reals[i - 1]);
                        return Ok(Value::Real(val));
                    }
                }
                Err(format!("at(): x={} is out of scale range [{}, {}]",
                    x, scale.first().unwrap_or(&0.0), scale.last().unwrap_or(&0.0)))
            }

            _ => Err(format!("unknown method '{}' on Signal", method)),
        }
    }
}

impl SignalObj {
    fn find_scale(&self) -> Option<Vec<f64>> {
        for key in ["time", "frequency", "sweep"] {
            if let Some(VectorData::Real(v)) = self.result.vectors.get(key) {
                return Some(v.clone());
            }
        }
        None
    }
}

// ── ArrayObj: dynamic array / queue with reference (handle) semantics ─────────
//
// Created from an array literal (`'{1, 2, 3}`) and mutated through methods
// (`push_back`, indexed `set`, …). The backing `Vec` lives behind an `Arc<Mutex>`,
// so assigning an array to another variable shares the same storage — like a
// SystemVerilog object handle, not a value copy.

use std::sync::Mutex;

#[derive(Debug)]
pub struct ArrayObj {
    items: Mutex<Vec<Value>>,
}

impl ArrayObj {
    /// Wrap a list of values into an array handle.
    pub fn new(items: Vec<Value>) -> Value {
        Value::ExternObject(Arc::new(Self { items: Mutex::new(items) }))
    }
}

/// Numeric view of a value for reductions; non-numeric counts as 0.
fn num(v: &Value) -> f64 {
    v.as_f64().unwrap_or(0.0)
}

impl ExternClass for ArrayObj {
    fn type_name(&self) -> &str { "Array" }

    fn call_method(&self, method: &str, args: &[Value]) -> Result<Value, String> {
        let mut items = self.items.lock().unwrap();
        let arg0 = || args.first().cloned().unwrap_or(Value::Void);
        let idx0 = || args.first().and_then(|v| v.as_integer());

        match method {
            "size" | "len" => Ok(Value::Integer(items.len() as i64)),

            "push_back" | "push" => { items.push(arg0()); Ok(Value::Void) }
            "push_front"         => { items.insert(0, arg0()); Ok(Value::Void) }

            "pop_back"  => items.pop().ok_or_else(|| "pop_back on empty array".into()),
            "pop_front" => {
                if items.is_empty() { Err("pop_front on empty array".into()) }
                else { Ok(items.remove(0)) }
            }

            "get" => {
                let i = idx0().ok_or("get(i) requires an integer index")?;
                items.get(i as usize).cloned()
                    .ok_or_else(|| format!("array index {i} out of bounds (len {})", items.len()))
            }
            "set" => {
                let i = idx0().ok_or("set(i, v) requires an integer index")? as usize;
                let v = args.get(1).cloned().unwrap_or(Value::Void);
                if i < items.len() { items[i] = v; Ok(Value::Void) }
                else { Err(format!("array index {i} out of bounds (len {})", items.len())) }
            }
            "insert" => {
                let i = idx0().ok_or("insert(i, v) requires an integer index")? as usize;
                let v = args.get(1).cloned().unwrap_or(Value::Void);
                if i <= items.len() { items.insert(i, v); Ok(Value::Void) }
                else { Err(format!("insert index {i} out of bounds (len {})", items.len())) }
            }
            "delete" => {
                match idx0() {
                    Some(i) => {
                        let i = i as usize;
                        if i < items.len() { items.remove(i); Ok(Value::Void) }
                        else { Err(format!("delete index {i} out of bounds (len {})", items.len())) }
                    }
                    None => { items.clear(); Ok(Value::Void) } // delete() clears all
                }
            }
            "clear" => { items.clear(); Ok(Value::Void) }

            "first" => items.first().cloned().ok_or_else(|| "first() on empty array".into()),
            "last"  => items.last().cloned().ok_or_else(|| "last() on empty array".into()),
            "reverse" => { items.reverse(); Ok(Value::Void) }

            "sum"     => Ok(Value::Real(items.iter().map(num).sum())),
            "product" => Ok(Value::Real(items.iter().map(num).product())),
            "mean" => {
                if items.is_empty() { return Err("mean() on empty array".into()); }
                Ok(Value::Real(items.iter().map(num).sum::<f64>() / items.len() as f64))
            }
            "min" => items.iter().map(num).reduce(f64::min)
                .map(Value::Real).ok_or_else(|| "min() on empty array".into()),
            "max" => items.iter().map(num).reduce(f64::max)
                .map(Value::Real).ok_or_else(|| "max() on empty array".into()),

            "values" => Ok(Value::RealVec(items.iter().map(num).collect())),

            _ => Err(format!("unknown method '{method}' on Array")),
        }
    }
}
