//! Attribute schema registry — which bundle names are registered as
//! attribute schemas via `@attribute(schema = "BundleName")`.

use std::collections::HashSet;

/// Tracks which bundle names have been registered as attribute schemas.
/// A bundle registered as a schema may be used as `@BundleName(field = value, ...)`
/// on any declaration; the arguments are validated against the bundle's fields.
pub struct SchemaRegistry {
    schemas: HashSet<String>,
}

impl SchemaRegistry {
    pub fn new() -> Self {
        Self { schemas: HashSet::new() }
    }

    /// Register a bundle name as an attribute schema.
    pub fn register(&mut self, name: &str) {
        self.schemas.insert(name.to_string());
    }

    /// Check if a name is a registered attribute schema.
    pub fn contains(&self, name: &str) -> bool {
        self.schemas.contains(name)
    }
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}
