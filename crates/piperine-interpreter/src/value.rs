use std::fmt;
use std::sync::Arc;
use std::collections::HashMap;

/// A runtime value in the Piperine interpreter.
#[derive(Debug, Clone)]
pub enum Value {
    Real(f64),
    Integer(i64),
    String(std::string::String),
    Void,
    RealVec(Vec<f64>),
    Complex(f64, f64),
    AnalysisHandle(Arc<AnalysisResult>),
    ExternObject(Arc<dyn ExternClass>),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Real(a), Value::Real(b)) => a == b,
            (Value::Integer(a), Value::Integer(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Void, Value::Void) => true,
            (Value::RealVec(a), Value::RealVec(b)) => a == b,
            (Value::Complex(r1, i1), Value::Complex(r2, i2)) => r1 == r2 && i1 == i2,
            (Value::AnalysisHandle(a), Value::AnalysisHandle(b)) => Arc::ptr_eq(a, b),
            // For ExternObject we just use pointer equality
            (Value::ExternObject(a), Value::ExternObject(b)) => std::ptr::addr_eq(Arc::as_ptr(a), Arc::as_ptr(b)),
            _ => false,
        }
    }
}

pub trait ExternClass: std::fmt::Debug + Send + Sync {}

#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub kind: AnalysisKind,
    pub plot_name: String,
    pub vectors: HashMap<String, VectorData>,
    pub run_errors: Vec<RunError>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AnalysisKind { Op, Tran, Ac, Dc, Noise, Tf, Pz, Sens }

#[derive(Debug, Clone, PartialEq)]
pub enum VectorData {
    Real(Vec<f64>),
    Complex(Vec<(f64, f64)>),   // (real, imag) pairs
}

#[derive(Debug, Clone, PartialEq)]
pub struct RunError {
    pub message: String,
    pub time: Option<f64>,
    pub kind: RunErrorKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RunErrorKind { SoaViolation, UserAssert, SimulatorError }

impl Value {
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Real(v)    => Some(*v),
            Value::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self { Value::String(s) => Some(s), _ => None }
    }
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            Value::Integer(i) => Some(*i),
            Value::Real(v)    => Some(*v as i64),
            _ => None,
        }
    }
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Real(v)    => *v != 0.0,
            Value::Integer(i) => *i != 0,
            Value::String(s)  => !s.is_empty(),
            Value::Void       => false,
            _                 => true, // other types like vec/handle are truthy
        }
    }
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Real(_)    => "real",
            Value::Integer(_) => "integer",
            Value::String(_)  => "string",
            Value::Void       => "void",
            Value::RealVec(_) => "real_vec",
            Value::Complex(_,_) => "complex",
            Value::AnalysisHandle(_) => "analysis_handle",
            Value::ExternObject(_) => "extern_object",
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Real(v)    => write!(f, "{v}"),
            Value::Integer(i) => write!(f, "{i}"),
            Value::String(s)  => write!(f, "{s}"),
            Value::Void       => write!(f, "<void>"),
            Value::RealVec(v) => write!(f, "<vec of len {}>", v.len()),
            Value::Complex(r, i) => write!(f, "{r}+{i}i"),
            Value::AnalysisHandle(a) => write!(f, "<analysis {}>", a.plot_name),
            Value::ExternObject(_) => write!(f, "<extern_object>"),
        }
    }
}
