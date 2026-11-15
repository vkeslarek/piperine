//! What a host names about a device: a run-time [`Value`] (the kind a host
//! writes to a parameter between or during a run) and the [`ProbeSelection`] of
//! the device state a transient records. What a model declares about itself —
//! its parameters, units, defaults, terminals, observables — is its shape
//! (`piperine_abi::shape`), not a descriptor here.

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

#[cfg(test)]
mod tests {
    use super::*;

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
