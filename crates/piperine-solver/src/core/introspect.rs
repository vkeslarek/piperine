//! The introspection ABI: parameter, query, and terminal metadata — the
//! OSDI-style surface an [`Element`](crate::core::element::Element) exposes so
//! host sweeps, optimization loops, plugins, and CLI/UI tooling discover and
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
/// loops do the least work that is still correct. Variants are declared in
/// escalating order, so `Ord` compares strength — a driver folding several
/// writes takes the `max` and recomputes once at the strongest level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

/// Whether a terminal is user-facing or internal (ABI-29). External ports
/// appear in the module signature; internal terminals are non-port wires
/// the kernel nonetheless surfaces (series-R, thermal, hidden probes); an
/// auxiliary terminal is a diagnostic-only point a host hides by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerminalKind {
    /// A port declared in the module signature (user-facing).
    External,
    /// An internal node (non-port `wire` — series-R, thermal, etc.).
    Internal,
    /// An auxiliary node (hidden, diagnostic-only — e.g., a probe point).
    Auxiliary,
}

/// Model identity and version for diagnostics + introspection (ABI-46). A
/// host uses this to render model-specific UI (e.g., picking the right
/// opvar table for `"mos"` vs `"diode"`) and to gate feature availability
/// without name-matching. The default `{ type_id: "", version: "" }` is
/// the conservative "no identity declared" — a host falls back to the
/// instance name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelDescriptor {
    /// Source-level type id (`"mos"`, `"diode"`, `"bjt"`, …). Empty when
    /// the device does not declare a model family.
    pub type_id: String,
    /// Model version (`"3"`, `"3.1"`, …). Empty when unversioned.
    pub version: String,
}

impl ModelDescriptor {
    /// The "no identity declared" default — a host falls back to the
    /// instance name from [`Element::name`](crate::core::element::Element::name).
    pub const EMPTY: ModelDescriptor = ModelDescriptor {
        type_id: String::new(),
        version: String::new(),
    };
}

/// What kind of runtime quantity an [`ObservableDescriptor`] names
/// (ABI-32). The kind tells a host how to interpret the recorded value
/// (a branch current is a current; a state slot is operator-dependent)
/// and lets it group probes by category in UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObservableKind {
    /// A branch current the device reports via a force branch (`V(...) <- …`
    /// with a series-R term). The descriptor's `name` is the branch label.
    BranchCurrent,
    /// A charge-storing reactive state (`ddt` companion). The `name` is the
    /// state slot name from the kernel catalog.
    Charge,
    /// A flux-storing reactive state (inductor companion).
    Flux,
    /// A runtime state slot (delay/transition/idt operator, `$limit` vold).
    /// The `name` is the slot name from [`Introspect::list_state_slot_names`].
    State,
    /// A module-level persistent variable slot. The `name` is the var name
    /// (or a synthesized `var[k]` when the kernel does not surface names).
    Var,
}

/// A device-declared observable a host can request for per-step recording
/// (ABI-32). The descriptor carries the source-level name, the kind (so a
/// host can render/group probes), and a relative recording cost hint
/// (0 = free, 1 = full bank clone) — letting a host budget recording
/// against simulation cost.
#[derive(Debug, Clone, PartialEq)]
pub struct ObservableDescriptor {
    /// Source-level name (matches the entry a `ProbeSelection` request
    /// names). Unique within one device.
    pub name: String,
    /// What the recorded value represents.
    pub kind: ObservableKind,
    /// Relative recording cost (0 = free, 1 = full bank clone). A host
    /// uses this to budget recording against simulation cost.
    pub cost: f32,
}

/// Per-device observable requests for transient recording (ABI-33). Each
/// entry is `(device_label, observable_name)`; the analysis driver filters
/// `collect_device_banks` to record only the requested observables. An
/// empty selection records nothing — today's default-off behavior. The
/// global `TransientAnalysisOptions::record_device_state = true` remains
/// the "record every observable on every device" shorthand.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProbeSelection {
    /// `(device_label, observable_name)` pairs.
    pub requests: Vec<(String, String)>,
}

impl ProbeSelection {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add one `(device_label, observable_name)` request.
    pub fn request(mut self, device_label: impl Into<String>, observable: impl Into<String>) -> Self {
        self.requests.push((device_label.into(), observable.into()));
        self
    }

    /// Whether `device_label`/`observable_name` was requested.
    pub fn contains(&self, device_label: &str, observable_name: &str) -> bool {
        self.requests
            .iter()
            .any(|(d, o)| d == device_label && o == observable_name)
    }
}

impl Default for ModelDescriptor {
    fn default() -> Self {
        Self::EMPTY
    }
}

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
    /// Whether the terminal is a user-facing port or an internal/auxiliary
    /// node (ABI-29). Defaults to [`TerminalKind::External`].
    pub kind: TerminalKind,
}

impl TerminalDescriptor {
    pub fn new(name: impl Into<String>, domain: Domain, direction: Direction) -> Self {
        Self {
            name: name.into(), domain, direction,
            required: true, discipline: None, sign: SignConvention::IntoTerminal,
            kind: TerminalKind::External,
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
    use crate::core::element::{AnalogDevice, DigitalDevice, Element, ElementCapabilities, Introspect};

    /// A resistor exposing one writable parameter (`r`) and one operating
    /// variable (`g` = 1/r) — a reference implementation of the introspection
    /// ABI a host drives without knowing the device family.
    struct Resistor {
        r: f64,
    }

    impl AnalogDevice for Resistor {}

    impl DigitalDevice for Resistor {}

    impl Introspect for Resistor {
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
            (name == "r").then_some(Value::Real(self.r))
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

    impl Element for Resistor {
        fn name(&self) -> &str {
            "r1"
        }
        fn capabilities(&self) -> ElementCapabilities {
            ElementCapabilities::ANALOG
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
        assert_eq!(desc.kind, TerminalKind::External);
    }

    #[test]
    fn terminal_descriptor_with_custom_values() {
        let mut desc = TerminalDescriptor::new("n", Domain::Analog, Direction::Inout);
        desc.discipline = Some("electrical".into());
        desc.sign = SignConvention::OutOfTerminal;
        desc.kind = TerminalKind::Internal;
        
        assert_eq!(desc.discipline, Some("electrical".into()));
        assert_eq!(desc.sign, SignConvention::OutOfTerminal);
        assert_eq!(desc.kind, TerminalKind::Internal);
    }

    #[test]
    fn terminal_kind_distinguishes_external_internal_auxiliary() {
        let ext = TerminalDescriptor::new("d", Domain::Analog, Direction::Inout);
        let mut int = TerminalDescriptor::new("dp", Domain::Analog, Direction::Inout);
        let mut aux = TerminalDescriptor::new("probe", Domain::Analog, Direction::Inout);
        int.kind = TerminalKind::Internal;
        aux.kind = TerminalKind::Auxiliary;
        // ABI-29: the three kinds are distinct values; `External` is the
        // default in `TerminalDescriptor::new`, the other two are opt-in.
        assert_eq!(ext.kind, TerminalKind::External);
        assert_eq!(int.kind, TerminalKind::Internal);
        assert_eq!(aux.kind, TerminalKind::Auxiliary);
        assert_ne!(ext.kind, int.kind);
        assert_ne!(int.kind, aux.kind);
        assert_ne!(ext.kind, aux.kind);
    }

    #[test]
    fn model_descriptor_default_is_empty_sentinel() {
        // ABI-46: a host-built Element with no kernel inherits the empty
        // descriptor — both fields empty, host falls back to instance name.
        let r = Resistor { r: 1000.0 };
        let descriptor = r.model_descriptor();
        assert_eq!(descriptor.type_id, "");
        assert_eq!(descriptor.version, "");
        assert_eq!(ModelDescriptor::default(), ModelDescriptor::EMPTY);
        assert_eq!(ModelDescriptor::EMPTY.type_id, "");
        assert_eq!(ModelDescriptor::EMPTY.version, "");
    }

    #[test]
    fn named_catalogs_default_empty_for_simple_element() {
        // ABI-47: a plain analog-only device inherits empty named catalogs
        // for state slots, force terminals, and noise terminals.
        let r = Resistor { r: 1000.0 };
        assert!(r.list_state_slot_names().is_empty());
        assert!(r.list_force_terminal_pairs().is_empty());
        assert!(r.list_noise_terminal_pairs().is_empty());
    }

    #[test]
    fn observable_catalog_defaults_empty_for_simple_element() {
        // ABI-32: a plain analog-only device inherits an empty observable
        // catalog — `list_observables()` defaults to nothing, so a host
        // requesting anything on it fails loud at setup (ABI-35).
        let r = Resistor { r: 1000.0 };
        assert!(r.list_observables().is_empty());
    }

    #[test]
    fn probe_selection_default_empty_and_contains_check_works() {
        // ABI-33: an empty `ProbeSelection` is the default-off recording
        // mode (no device/observable pairs requested). `contains` is the
        // per-(device, observable) lookup the analysis driver uses to
        // filter `collect_device_banks`.
        let sel = ProbeSelection::new();
        assert!(sel.requests.is_empty());
        assert!(!sel.contains("r1", "i"));

        let sel = ProbeSelection::new()
            .request("r1", "i(p,n)")
            .request("c1", "ddt[0]");
        assert_eq!(sel.requests.len(), 2);
        assert!(sel.contains("r1", "i(p,n)"));
        assert!(sel.contains("c1", "ddt[0]"));
        assert!(!sel.contains("r1", "ddt[0]"));
        assert!(!sel.contains("c1", "i(p,n)"));
        assert!(!sel.contains("missing", "i(p,n)"));
    }
}
