use std::collections::HashMap;

/// A resolved parameter value — evaluated at elaboration time from AST literals.
#[derive(Debug, Clone)]
pub enum ParameterValue {
    Real(f64),
    Integer(i64),
    String(std::string::String),
}

impl ParameterValue {
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            ParameterValue::Real(v)    => Some(*v),
            ParameterValue::Integer(i) => Some(*i as f64),
            ParameterValue::String(_)  => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self { ParameterValue::String(s) => Some(s), _ => None }
    }
}

impl std::fmt::Display for ParameterValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParameterValue::Real(v)    => write!(f, "{v}"),
            ParameterValue::Integer(i) => write!(f, "{i}"),
            ParameterValue::String(s)  => write!(f, "{s}"),
        }
    }
}

/// `parameter_name → value` mapping for one instance.
pub type ParameterMap = HashMap<std::string::String, ParameterValue>;

/// `port_name → net_name` mapping for one instance.
pub type ConnectionMap = HashMap<std::string::String, std::string::String>;

/// Parse a Verilog-A SI-suffix real literal into f64.
/// Handles: T G M k m u n p f a ns us ms ps fs
pub fn parse_si_real(s: &str) -> Option<f64> {
    let bytes = s.as_bytes();
    let (number_str, suffix) = match bytes.last() {
        Some(c) if c.is_ascii_alphabetic() => {
            // Check for two-char suffix (ns, us, ms, ps, fs)
            if bytes.len() >= 2 && bytes[bytes.len() - 1] == b's'
                && bytes[bytes.len() - 2].is_ascii_alphabetic()
            {
                (&s[..s.len() - 2], &s[s.len() - 2..])
            } else {
                (&s[..s.len() - 1], &s[s.len() - 1..])
            }
        }
        _ => return s.parse::<f64>().ok(),
    };
    let base: f64 = number_str.parse().ok()?;
    let scale = match suffix {
        "T"  => 1e12,  "G"  => 1e9,   "M"  => 1e6,
        "K" | "k" => 1e3,
        "m"  => 1e-3,  "u"  => 1e-6,  "n"  => 1e-9,
        "p"  => 1e-12, "f"  => 1e-15, "a"  => 1e-18,
        "ns" => 1e-9,  "us" => 1e-6,  "ms" => 1e-3,
        "ps" => 1e-12, "fs" => 1e-15,
        _ => return None,
    };
    Some(base * scale)
}
