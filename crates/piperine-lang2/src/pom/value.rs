//! POM `Value` — the value layer of the language, returned by attribute accessors.

use std::fmt;

use crate::elab::const_eval::ConstVal;

/// A value-layer scalar. Returned by `Param::value()`, `Field::default()`, etc.
///
/// Never a net or a piece of hardware — hardware stays in the static net
/// layer; POM describes it but is itself value-level data.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// IEEE 754 double-precision floating point.
    Real(f64),
    /// Unsigned 64-bit integer.
    Natural(u64),
    /// Signed 64-bit integer.
    Integer(i64),
    /// Boolean value.
    Boolean(bool),
    /// 8-level logic value (backed by a `u8`).
    Quad(u8),
    /// String value.
    String(String),
    /// Complex number (real, imag).
    Complex(f64, f64),
}

impl Value {
    /// Extract the inner `f64` if this is a `Real`.
    pub fn as_real(&self) -> Option<f64> {
        match self { Self::Real(v) => Some(*v), _ => None }
    }
    /// Extract the inner `u64` if this is a `Natural`.
    pub fn as_natural(&self) -> Option<u64> {
        match self { Self::Natural(v) => Some(*v), _ => None }
    }
    /// Extract the inner `i64` if this is an `Integer`.
    pub fn as_integer(&self) -> Option<i64> {
        match self { Self::Integer(v) => Some(*v), _ => None }
    }
    /// Extract the inner `bool` if this is a `Boolean`.
    pub fn as_boolean(&self) -> Option<bool> {
        match self { Self::Boolean(v) => Some(*v), _ => None }
    }
    /// Extract the inner `&str` if this is a `String`.
    pub fn as_string(&self) -> Option<&str> {
        match self { Self::String(v) => Some(v), _ => None }
    }
    /// Extract the inner `u8` if this is a `Quad`.
    pub fn as_quad(&self) -> Option<u8> {
        match self { Self::Quad(v) => Some(*v), _ => None }
    }
    /// Extract the `(re, im)` pair if this is a `Complex`.
    pub fn as_complex(&self) -> Option<(f64, f64)> {
        match self { Self::Complex(re, im) => Some((*re, *im)), _ => None }
    }

    /// Returns the variant name as a static string (e.g. `"Real"`, `"Complex"`).
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Real(_) => "Real",
            Self::Natural(_) => "Natural",
            Self::Integer(_) => "Integer",
            Self::Boolean(_) => "Boolean",
            Self::Quad(_) => "Quad",
            Self::String(_) => "String",
            Self::Complex(_, _) => "Complex",
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Real(v) => write!(f, "{v}"),
            Self::Natural(v) => write!(f, "{v}"),
            Self::Integer(v) => write!(f, "{v}"),
            Self::Boolean(v) => write!(f, "{v}"),
            Self::Quad(v) => write!(f, "0q{v}"),
            Self::String(v) => write!(f, "\"{v}\""),
            Self::Complex(re, im) => write!(f, "{re}+{im}j"),
        }
    }
}

/// Convert an `f64` into a `Value::Real`.
impl From<f64> for Value {
    fn from(v: f64) -> Self { Self::Real(v) }
}
/// Convert a `u64` into a `Value::Natural`.
impl From<u64> for Value {
    fn from(v: u64) -> Self { Self::Natural(v) }
}
/// Convert an `i64` into a `Value::Integer`.
impl From<i64> for Value {
    fn from(v: i64) -> Self { Self::Integer(v) }
}
/// Convert a `bool` into a `Value::Boolean`.
impl From<bool> for Value {
    fn from(v: bool) -> Self { Self::Boolean(v) }
}
/// Convert a `String` into a `Value::String`.
impl From<String> for Value {
    fn from(v: String) -> Self { Self::String(v) }
}
/// Convert a `&str` into a `Value::String`.
impl From<&str> for Value {
    fn from(v: &str) -> Self { Self::String(v.into()) }
}

/// Convert a `Complex64` into a `Value::Complex`.
impl From<num_complex::Complex64> for Value {
    fn from(c: num_complex::Complex64) -> Self {
        Self::Complex(c.re, c.im)
    }
}

/// Convert a `&ConstVal` into the corresponding `Value`.
impl From<&ConstVal> for Value {
    fn from(cv: &ConstVal) -> Self {
        match cv {
            ConstVal::Real(v) => Value::Real(*v),
            ConstVal::Int(v) => Value::Integer(*v),
            ConstVal::Nat(v) => Value::Natural(*v),
            ConstVal::Bool(v) => Value::Boolean(*v),
            ConstVal::Str(v) => Value::String(v.clone()),
        }
    }
}