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
    Enum { type_id: u32, variant: i64 },
    Struct { type_id: u32, fields: HashMap<String, Value> },
}

#[derive(Default, Debug, Clone)]
pub struct TypeRegistry {
    pub enums: HashMap<String, EnumTypeDef>,
    pub structs: HashMap<String, StructTypeDef>,
}

#[derive(Debug, Clone)]
pub struct EnumTypeDef {
    pub type_id: u32,
    pub variants: Vec<(String, i64)>,
}

#[derive(Debug, Clone)]
pub struct StructTypeDef {
    pub type_id: u32,
    pub fields: Vec<(String, String)>,
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
            (Value::ExternObject(a), Value::ExternObject(b)) => std::ptr::addr_eq(Arc::as_ptr(a), Arc::as_ptr(b)),
            (Value::Enum { type_id: a_id, variant: a_v }, Value::Enum { type_id: b_id, variant: b_v }) => {
                a_id == b_id && a_v == b_v
            }
            (Value::Struct { type_id: a_id, fields: a_f }, Value::Struct { type_id: b_id, fields: b_f }) => {
                a_id == b_id && a_f == b_f
            }
            _ => false,
        }
    }
}

pub trait ExternClass: std::fmt::Debug + Send + Sync {
    fn type_name(&self) -> &str;
    fn call_method(&self, method: &str, args: &[Value]) -> Result<Value, String>;
}

#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub kind: AnalysisKind,
    pub plot_name: String,
    pub vectors: HashMap<String, VectorData>,
    pub run_errors: Vec<RunError>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AnalysisKind { Op, Tran, Ac, Dc, Noise, Tf, Pz, Sens, Disto, Pss, Sp }

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

/// Infer `AnalysisKind` from an ngspice command string.
/// Matches the first word, case-insensitive. Used by the interpreter
/// so the backend doesn't need to know about `AnalysisKind`.
pub fn parse_analysis_kind(cmd: &str) -> AnalysisKind {
    match cmd.split_whitespace().next().unwrap_or("").to_lowercase().as_str() {
        "op"    => AnalysisKind::Op,
        "tran"  => AnalysisKind::Tran,
        "run"   => AnalysisKind::Tran,
        "ac"    => AnalysisKind::Ac,
        "dc"    => AnalysisKind::Dc,
        "noise" => AnalysisKind::Noise,
        "tf"    => AnalysisKind::Tf,
        "pz"    => AnalysisKind::Pz,
        "sens"  => AnalysisKind::Sens,
        "disto" => AnalysisKind::Disto,
        "pss"   => AnalysisKind::Pss,
        "sp"    => AnalysisKind::Sp,
        _       => AnalysisKind::Tran,
    }
}

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
            Value::Enum { .. } => "enum",
            Value::Struct { .. } => "struct",
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
            Value::Enum { type_id, variant } => write!(f, "<enum type={} variant={}>", type_id, variant),
            Value::Struct { type_id, fields } => write!(f, "<struct type={} fields={}>", type_id, fields.len()),
        }
    }
}
