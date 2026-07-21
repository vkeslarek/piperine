//! POM staging layer — parameter overrides staged for re-elaboration.
//!
//! Writing a parameter via `Param::set()` stages an override here; it does
//! not mutate the elaborated design in place. A subsequent re-elaboration
//! consumes the overrides purely and reproducibly.

use std::collections::HashMap;

use super::value::Value;

/// A staged instance injection (SPEC Part VI §8.2 `add_instance`): plain
/// data, applied by the next pure re-elaboration. The module name must be a
/// type declared in the design — validated at staging time by
/// [`Design::stage_instance`](super::design::Design::stage_instance)
/// (no-netlist-magic, Part VI §2).
#[derive(Debug, Clone, PartialEq)]
pub struct InstanceSpec {
    /// The label the new instance gets in the parent module.
    pub label: String,
    /// The declared module type to instantiate.
    pub module: String,
    /// Net names to connect, in the module's port order.
    pub ports: Vec<String>,
    /// Parameter overrides for the new instance.
    pub params: Vec<(String, Value)>,
}

/// A staged net connection (SPEC Part VI §8.2 `add_connection`).
#[derive(Debug, Clone)]
pub struct ConnectionSpec {
    pub lhs: String,
    pub rhs: String,
}

/// One staged instance injection with its provenance — `staged_by` names
/// the writer (a plugin name), so a conflict can name both
/// parties (SPEC Part VI §8.2, P0008).
#[derive(Debug, Clone)]
pub struct StagedInstance {
    pub parent: String,
    pub spec: InstanceSpec,
    pub staged_by: String,
}

/// A typed staging conflict: two writers staged different specs under the
/// same `(parent, label)`.
#[derive(Debug, Clone)]
pub struct StagingConflict {
    pub parent: String,
    pub label: String,
    /// Who staged first.
    pub first: String,
    /// Who collided.
    pub second: String,
}

impl std::fmt::Display for StagingConflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "staging conflict: `{}` and `{}` staged different specs for instance `{}` in `{}`",
            self.first, self.second, self.label, self.parent
        )
    }
}

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
    /// Staged instance injections, keyed by parent module. Applied (and
    /// fully validated) by `with_overrides_applied`.
    added_instances: Vec<StagedInstance>,
    /// Staged net connections, keyed by parent module.
    added_connections: Vec<(String, ConnectionSpec)>,
}

impl OverrideMap {
    /// Create an empty override map.
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

    /// Stage an instance injection. A second injection with the same
    /// `(parent, label)` but a different spec is a typed
    /// [`StagingConflict`] naming both writers (SPEC Part VI §8.2, P0008).
    /// Re-staging an identical spec is idempotent — hooks fire once per
    /// analysis.
    pub fn add_instance(
        &mut self,
        parent: &str,
        spec: InstanceSpec,
        staged_by: &str,
    ) -> Result<(), StagingConflict> {
        if let Some(existing) = self
            .added_instances
            .iter()
            .find(|e| e.parent == parent && e.spec.label == spec.label)
        {
            if existing.spec == spec {
                return Ok(());
            }
            return Err(StagingConflict {
                parent: parent.to_string(),
                label: spec.label,
                first: existing.staged_by.clone(),
                second: staged_by.to_string(),
            });
        }
        self.added_instances.push(StagedInstance {
            parent: parent.to_string(),
            spec,
            staged_by: staged_by.to_string(),
        });
        Ok(())
    }

    /// Stage a net connection.
    pub fn add_connection(&mut self, parent: &str, spec: ConnectionSpec) {
        self.added_connections.push((parent.to_string(), spec));
    }

    /// Staged instance injections.
    pub fn added_instances(&self) -> &[StagedInstance] {
        &self.added_instances
    }

    /// Staged net connections.
    pub fn added_connections(&self) -> &[(String, ConnectionSpec)] {
        &self.added_connections
    }

    /// Returns `true` if no overrides are staged.
    pub fn is_empty(&self) -> bool {
        self.overrides.is_empty() && self.added_instances.is_empty() && self.added_connections.is_empty()
    }

    /// Number of staged overrides.
    pub fn len(&self) -> usize {
        self.overrides.len()
    }

    /// Remove all staged overrides.
    pub fn clear(&mut self) {
        self.overrides.clear();
        self.added_instances.clear();
        self.added_connections.clear();
    }

    /// Iterate over all staged overrides.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &String, &Value)> {
        self.overrides.iter().map(|((p, n), v)| (p, n, v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pom::Value;

    #[test]
    fn override_map_empty() {
        let map = OverrideMap::new();
        assert!(map.is_empty());
    }

    #[test]
    fn override_map_set_and_get() {
        let mut map = OverrideMap::new();
        map.set("top.u1", "r", Value::Real(2.0e3));
        assert!(!map.is_empty());
        let v = map.get("top.u1", "r").expect("override present");
        assert_eq!(v.as_real(), Some(2.0e3));
    }

    #[test]
    fn override_map_miss() {
        let map = OverrideMap::new();
        assert!(map.get("top.u1", "r").is_none());
    }

    #[test]
    fn override_map_clear() {
        let mut map = OverrideMap::new();
        map.set("top", "r", Value::Real(1.0));
        assert!(!map.is_empty());
        map.clear();
        assert!(map.is_empty());
    }

    #[test]
    fn override_map_overwrite() {
        let mut map = OverrideMap::new();
        map.set("top", "r", Value::Real(1.0));
        map.set("top", "r", Value::Real(2.0));
        let v = map.get("top", "r").expect("present");
        assert_eq!(v.as_real(), Some(2.0));
    }

    #[test]
    fn override_map_distinct_paths() {
        let mut map = OverrideMap::new();
        map.set("top.u1", "r", Value::Real(1.0));
        map.set("top.u2", "r", Value::Real(2.0));
        assert_eq!(map.get("top.u1", "r").unwrap().as_real(), Some(1.0));
        assert_eq!(map.get("top.u2", "r").unwrap().as_real(), Some(2.0));
    }
}