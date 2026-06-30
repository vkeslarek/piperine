//! POM `Value` — the value layer of the language, returned by attribute accessors.

use std::fmt;

/// A value-layer scalar. Returned by `Param::value()`, `Field::default()`, etc.
///
/// Never a net or a piece of hardware — hardware stays in the static net
/// layer; POM describes it but is itself value-level data.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Real(f64),
    Natural(u64),
    Integer(i64),
    Boolean(bool),
    Quad(u8),
    String(String),
    Complex(f64, f64),
}

impl Value {
    pub fn as_real(&self) -> Option<f64> {
        match self { Self::Real(v) => Some(*v), _ => None }
    }
    pub fn as_natural(&self) -> Option<u64> {
        match self { Self::Natural(v) => Some(*v), _ => None }
    }
    pub fn as_integer(&self) -> Option<i64> {
        match self { Self::Integer(v) => Some(*v), _ => None }
    }
    pub fn as_boolean(&self) -> Option<bool> {
        match self { Self::Boolean(v) => Some(*v), _ => None }
    }
    pub fn as_string(&self) -> Option<&str> {
        match self { Self::String(v) => Some(v), _ => None }
    }

    /// The type name as it appears in PHDL source: `"Real"`, `"Natural"`, etc.
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

impl From<f64> for Value {
    fn from(v: f64) -> Self { Self::Real(v) }
}
impl From<u64> for Value {
    fn from(v: u64) -> Self { Self::Natural(v) }
}
impl From<i64> for Value {
    fn from(v: i64) -> Self { Self::Integer(v) }
}
impl From<bool> for Value {
    fn from(v: bool) -> Self { Self::Boolean(v) }
}
impl From<String> for Value {
    fn from(v: String) -> Self { Self::String(v) }
}
impl From<&str> for Value {
    fn from(v: &str) -> Self { Self::String(v.into()) }
}