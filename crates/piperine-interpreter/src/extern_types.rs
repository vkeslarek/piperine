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
/// `.signal()`, `.scale()`, `.ok()`, `.plot_name()` methods.
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
            "plot_name" => Ok(Value::String(self.result.plot_name.clone())),

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
