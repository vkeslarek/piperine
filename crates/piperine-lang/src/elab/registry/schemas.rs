//! Attribute schema registry — maps schema names to the bundle declarations
//! that back them, registered via `@attribute(schema = "name")` on a bundle.

use std::collections::HashMap;

/// Tracks which schema names are registered and which bundle each maps to.
/// A bundle registered as a schema via `@attribute(schema = "foo")` may be
/// used as `@foo(field = value, ...)` on any declaration.
pub struct SchemaRegistry {
    schemas: HashMap<String, String>,
}

impl SchemaRegistry {
    pub fn new() -> Self {
        Self { schemas: HashMap::new() }
    }

    /// Register a schema name, backed by the named bundle's fields.
    pub fn register(&mut self, schema_name: &str, bundle_name: &str) {
        self.schemas.insert(schema_name.to_string(), bundle_name.to_string());
    }

    /// Look up the bundle name backing a schema, if registered.
    pub fn lookup(&self, schema_name: &str) -> Option<&str> {
        self.schemas.get(schema_name).map(|s| s.as_str())
    }
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}
