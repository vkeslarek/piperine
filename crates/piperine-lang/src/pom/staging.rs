//! POM staging layer — parameter overrides staged for re-elaboration.
//!
//! Writing a parameter via `Param::set()` stages an override here; it does
//! not mutate the elaborated design in place. A subsequent re-elaboration
//! consumes the overrides purely and reproducibly.

use std::collections::HashMap;

use super::value::Value;

/// A map of staged parameter overrides keyed by `(instance_path, param_name)`.
///
/// The path is a hierarchical dotted name from the design root
/// (e.g. `"top.dac"` or `"top.rseg[0]"`). The param name is the
/// declared parameter name (e.g. `"r"`, `"c"`).
///
/// `OverrideMap` is the single mutation surface in the POM — everything
/// else is read-only. It is consumed by the next `elaborate`/`simulate`
/// call, which re-runs elaboration with the overrides applied.
#[derive(Debug, Clone, Default)]
pub struct OverrideMap {
    overrides: HashMap<(String, String), Value>,
}

impl OverrideMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Stage a parameter override. If an override for the same
    /// `(path, param)` pair already exists, it is replaced.
    pub fn set(&mut self, path: &str, param: &str, value: Value) {
        self.overrides.insert((path.into(), param.into()), value);
    }

    /// Look up a staged override.
    pub fn get(&self, path: &str, param: &str) -> Option<&Value> {
        self.overrides.get(&(path.into(), param.into()))
    }

    pub fn is_empty(&self) -> bool {
        self.overrides.is_empty()
    }

    pub fn len(&self) -> usize {
        self.overrides.len()
    }

    pub fn clear(&mut self) {
        self.overrides.clear();
    }

    /// Iterate over all staged overrides.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &String, &Value)> {
        self.overrides.iter().map(|((p, n), v)| (p, n, v))
    }
}