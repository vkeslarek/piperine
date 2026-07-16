//! The introspection ABI: parameter, query, and terminal metadata — the
//! OSDI-style surface an [`Element`](crate::core::element::Element) exposes so
//! bench sweeps, optimization loops, plugins, and CLI/UI tooling discover and
//! poke a model without knowing its family.
//!
//! Three concerns, all optional (defaulted on `Element`):
//! - **Parameters** — declared with descriptors ([`ParamDescriptor`]) and read/
//!   written at run time (`get_param`/`set_param`), where every write reports
//!   the [`Invalidation`] it forces so a sweep restamps, recomputes, or rebuilds
//!   exactly as much as needed.
//! - **Queries** — operating variables, terminal quantities, internal state,
//!   and counters, declared with [`QueryDescriptor`] and read with `query`.
//! - **Terminals** — declared with [`TerminalDescriptor`] for diagnostics,
//!   current queries, and external-model wrapping.

use std::fmt;

/// A runtime parameter, query, or operating-variable value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Real(f64),
    Integer(i64),
    Boolean(bool),
    Text(String),
}

impl Value {
    /// The kind this value carries.
    pub fn kind(&self) -> ValueKind {
        match self {
            Value::Real(_) => ValueKind::Real,
            Value::Integer(_) => ValueKind::Integer,
            Value::Boolean(_) => ValueKind::Boolean,
            Value::Text(_) => ValueKind::Text,
        }
    }

    /// The value as `f64` when it is `Real` or `Integer`.
    pub fn as_real(&self) -> Option<f64> {
        match self {
            Value::Real(v) => Some(*v),
            Value::Integer(v) => Some(*v as f64),
            _ => None,
        }
    }
}

/// The type a [`Value`] takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueKind {
    Real,
    Integer,
    Boolean,
    Text,
}

/// Whether a parameter belongs to the shared model card or one instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParamScope {
    /// Shared by every instance of the model card.
    Model,
    /// Owned by a single element instance.
    Instance,
}

/// What recomputation a parameter change forces. Lets sweeps and optimization
/// loops do the least work that is still correct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Invalidation {
    /// Metadata only; nothing to recompute.
    None,
    /// Restamp numeric values on the next load; no structural change.
    Restamp,
    /// Recompute temperature-dependent constants, then restamp.
    Temperature,
    /// Restart the operating point.
    OperatingPoint,
    /// Rebuild matrix structure / reconstruct the element.
    Rebuild,
}

/// Inclusive numeric bounds on a real or integer parameter. Absent ends are
/// unbounded.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Bounds {
    pub min: Option<f64>,
    pub max: Option<f64>,
}

impl Bounds {
    pub const UNBOUNDED: Bounds = Bounds { min: None, max: None };

    /// Whether `v` is within the (inclusive) bounds.
    pub fn contains(&self, v: f64) -> bool {
        self.min.is_none_or(|lo| v >= lo) && self.max.is_none_or(|hi| v <= hi)
    }
}

/// Metadata for one parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamDescriptor {
    pub name: String,
    pub kind: ValueKind,
    pub default: Value,
    pub unit: Option<String>,
    pub bounds: Bounds,
    pub scope: ParamScope,
    /// What a write to this parameter invalidates.
    pub invalidation: Invalidation,
}

/// What a query reports about an element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueryKind {
    /// An operating-point variable (`gm`, `vbe`, `gds`, …).
    OperatingVariable,
    /// A terminal voltage.
    TerminalVoltage,
    /// A terminal current.
    TerminalCurrent,
    /// Internal hidden state (charge, latch, register).
    InternalState,
    /// An event/activity counter.
    EventCounter,
    /// Device limiting/convergence state.
    LimitState,
}

/// Metadata for one query / operating variable.
#[derive(Debug, Clone, PartialEq)]
pub struct QueryDescriptor {
    pub name: String,
    pub kind: QueryKind,
    pub unit: Option<String>,
    pub description: Option<String>,
}

impl QueryDescriptor {
    /// A bare operating variable, no unit or description — the shape the
    /// default `list_queries` derives from `read_opvars`.
    pub fn opvar(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: QueryKind::OperatingVariable,
            unit: None,
            description: None,
        }
    }
}

/// The domain a terminal lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Domain {
    Analog,
    Digital,
}

/// A terminal's flow direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    In,
    Out,
    Inout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignConvention { IntoTerminal, OutOfTerminal }

/// Metadata for one declared terminal.
#[derive(Debug, Clone, PartialEq)]
pub struct TerminalDescriptor {
    pub name: String,
    pub domain: Domain,
    pub direction: Direction,
    /// Whether the terminal must be connected. Optional terminals may be left
    /// unbound where the model contract allows it.
    pub required: bool,
    pub discipline: Option<String>,
    pub sign: SignConvention,
}

impl TerminalDescriptor {
    pub fn new(name: impl Into<String>, domain: Domain, direction: Direction) -> Self {
        Self {
            name: name.into(), domain, direction,
            required: true, discipline: None, sign: SignConvention::IntoTerminal,
        }
    }
}

/// Why a `set_param` was rejected.
#[derive(Debug, Clone, PartialEq)]
pub enum ParamError {
    /// No parameter by that name.
    Unknown(String),
    /// The parameter exists but cannot be written at run time.
    ReadOnly(String),
    /// The value lies outside the parameter's declared bounds.
    OutOfRange { name: String, value: Value },
    /// The value's type does not match the parameter's declared kind.
    TypeMismatch { name: String, expected: ValueKind },
}

impl fmt::Display for ParamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParamError::Unknown(name) => write!(f, "unknown parameter `{name}`"),
            ParamError::ReadOnly(name) => write!(f, "parameter `{name}` is read-only"),
            ParamError::OutOfRange { name, value } => {
                write!(f, "value {value:?} is out of range for parameter `{name}`")
            }
            ParamError::TypeMismatch { name, expected } => {
                write!(f, "parameter `{name}` expects a {expected:?} value")
            }
        }
    }
}

impl std::error::Error for ParamError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::element::{Element, ElementCapabilities};

    /// A resistor exposing one writable parameter (`r`) and one operating
    /// variable (`g` = 1/r) — a reference implementation of the introspection
    /// ABI a host drives without knowing the device family.
    struct Resistor {
        r: f64,
    }

    impl Element for Resistor {
        fn name(&self) -> &str {
            "r1"
        }
        fn capabilities(&self) -> ElementCapabilities {
            ElementCapabilities::ANALOG
        }
        fn read_opvars(&self) -> Vec<(String, f64)> {
            vec![("g".into(), 1.0 / self.r)]
        }
        fn list_params(&self) -> Vec<ParamDescriptor> {
            vec![ParamDescriptor {
                name: "r".into(),
                kind: ValueKind::Real,
                default: Value::Real(1000.0),
                unit: Some("ohm".into()),
                bounds: Bounds { min: Some(0.0), max: None },
                scope: ParamScope::Instance,
                invalidation: Invalidation::Restamp,
            }]
        }
        fn get_param(&self, name: &str) -> Option<Value> {
            (name == "r").then(|| Value::Real(self.r))
        }
        fn set_param(&mut self, name: &str, value: Value) -> Result<Invalidation, ParamError> {
            if name != "r" {
                return Err(ParamError::Unknown(name.into()));
            }
            let Some(v) = value.as_real() else {
                return Err(ParamError::TypeMismatch { name: name.into(), expected: ValueKind::Real });
            };
            if v <= 0.0 {
                return Err(ParamError::OutOfRange { name: name.into(), value });
            }
            self.r = v;
            Ok(Invalidation::Restamp)
        }
    }

    #[test]
    fn parameters_are_discoverable_and_writable() {
        let mut r = Resistor { r: 1000.0 };

        let params = r.list_params();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "r");
        assert_eq!(params[0].invalidation, Invalidation::Restamp);

        assert_eq!(r.get_param("r"), Some(Value::Real(1000.0)));
        assert_eq!(r.get_param("nope"), None);

        assert_eq!(r.set_param("r", Value::Real(2000.0)), Ok(Invalidation::Restamp));
        assert_eq!(r.get_param("r"), Some(Value::Real(2000.0)));

        assert_eq!(
            r.set_param("r", Value::Real(-1.0)),
            Err(ParamError::OutOfRange { name: "r".into(), value: Value::Real(-1.0) })
        );
        assert!(matches!(r.set_param("x", Value::Real(1.0)), Err(ParamError::Unknown(_))));
    }

    #[test]
    fn queries_default_through_opvars() {
        let r = Resistor { r: 2000.0 };
        // The default `list_queries`/`query` read `read_opvars` — no extra impl.
        let queries = r.list_queries();
        assert_eq!(queries.len(), 1);
        assert_eq!(queries[0].name, "g");
        assert_eq!(queries[0].kind, QueryKind::OperatingVariable);
        assert_eq!(r.query("g"), Some(Value::Real(1.0 / 2000.0)));
        assert_eq!(r.query("missing"), None);
    }
    #[test]
    fn terminal_descriptor_new_sets_defaults() {
        let desc = TerminalDescriptor::new("p", Domain::Analog, Direction::Inout);
        assert_eq!(desc.name, "p");
        assert_eq!(desc.domain, Domain::Analog);
        assert_eq!(desc.direction, Direction::Inout);
        assert!(desc.required);
        assert_eq!(desc.discipline, None);
        assert_eq!(desc.sign, SignConvention::IntoTerminal);
    }

    #[test]
    fn terminal_descriptor_with_custom_values() {
        let mut desc = TerminalDescriptor::new("n", Domain::Analog, Direction::Inout);
        desc.discipline = Some("electrical".into());
        desc.sign = SignConvention::OutOfTerminal;
        
        assert_eq!(desc.discipline, Some("electrical".into()));
        assert_eq!(desc.sign, SignConvention::OutOfTerminal);
    }
}
