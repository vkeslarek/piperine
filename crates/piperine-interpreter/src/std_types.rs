//! Standard Piperine value types available in testbenches.
//!
//! These types are returned by simulator APIs and expose methods callable
//! from Piperine testbench code via the `obj.method()` syntax.

use std::sync::Arc;
use crate::value::{ExternClass, Value};

/// A complex number (real + imaginary pair). Returned by AC analysis vectors.
///
/// Methods:
/// - `.real()` — real part
/// - `.imag()` — imaginary part
/// - `.magnitude()` — |z| = sqrt(re²+im²)
/// - `.phase()` — angle in degrees
/// - `.phase_rad()` — angle in radians
/// - `.db20()` — 20·log10(|z|), i.e. dB for voltage/current
/// - `.db10()` — 10·log10(|z|²), i.e. dB for power
/// - `.conjugate()` — complex conjugate
#[derive(Debug, Clone)]
pub struct ComplexValue(pub f64, pub f64);

impl ComplexValue {
    pub fn new(re: f64, im: f64) -> Value {
        Value::ExternObject(Arc::new(Self(re, im)))
    }

    fn magnitude(&self) -> f64 {
        (self.0 * self.0 + self.1 * self.1).sqrt()
    }
}

impl ExternClass for ComplexValue {
    fn type_name(&self) -> &str { "Complex" }

    fn call_method(&self, method: &str, _args: &[Value]) -> Result<Value, String> {
        match method {
            "real"      => Ok(Value::Real(self.0)),
            "imag"      => Ok(Value::Real(self.1)),
            "magnitude" => Ok(Value::Real(self.magnitude())),
            "phase"     => Ok(Value::Real(self.1.atan2(self.0).to_degrees())),
            "phase_rad" => Ok(Value::Real(self.1.atan2(self.0))),
            "db20"      => Ok(Value::Real(20.0 * self.magnitude().log10())),
            "db10"      => Ok(Value::Real(10.0 * (self.0 * self.0 + self.1 * self.1).log10())),
            "conjugate" => Ok(ComplexValue::new(self.0, -self.1)),
            _ => Err(format!("unknown method '{}' on Complex", method)),
        }
    }
}
