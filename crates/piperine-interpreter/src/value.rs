use std::fmt;

/// A runtime value in the Piperine interpreter.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Real(f64),
    Integer(i64),
    String(std::string::String),
    Void,
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
        }
    }
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Real(_)    => "real",
            Value::Integer(_) => "integer",
            Value::String(_)  => "string",
            Value::Void       => "void",
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
        }
    }
}
