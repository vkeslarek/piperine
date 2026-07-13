//! Attribute schema registry — maps schema names to the shape attribute
//! arguments are validated against. Two registration paths, one store
//! (SPEC Part I §8, Part VI §11):
//!
//! - **Bundle-backed** — `@attribute(schema = "name")` on a PHDL bundle;
//!   fields come from the bundle declaration.
//! - **Declared** — registered by the host (plugin-contributed schemas, and
//!   the plugin system's own `@device`/`@port`); fields carried directly.

use std::collections::HashMap;

/// One field of a declared (non-bundle) attribute schema.
#[derive(Debug, Clone)]
pub struct AttrField {
    pub name: String,
    /// The PHDL type name the value must satisfy (`"String"`, `"Real"`, …) —
    /// the same names bundle fields use.
    pub ty: String,
    /// Required fields must be provided at every use site; optional fields
    /// take `default` when omitted.
    pub required: bool,
    pub default: Option<crate::value::Value>,
}

/// What backs a registered schema name.
#[derive(Debug, Clone)]
pub enum SchemaShape {
    /// Backed by the named PHDL bundle's fields.
    Bundle(String),
    /// Fields declared directly (host/plugin-registered).
    Declared(Vec<AttrField>),
}

/// Tracks which schema names are registered and the shape each validates
/// against.
pub struct SchemaRegistry {
    schemas: HashMap<String, SchemaShape>,
}

impl SchemaRegistry {
    pub fn new() -> Self {
        Self { schemas: HashMap::new() }
    }

    /// Register a schema name backed by the named bundle's fields.
    pub fn register(&mut self, schema_name: &str, bundle_name: &str) {
        self.schemas.insert(schema_name.to_string(), SchemaShape::Bundle(bundle_name.to_string()));
    }

    /// Register a schema name with directly-declared fields (host/plugins).
    pub fn register_declared(&mut self, schema_name: &str, fields: Vec<AttrField>) {
        self.schemas.insert(schema_name.to_string(), SchemaShape::Declared(fields));
    }

    /// The shape backing a schema, if registered.
    pub fn shape(&self, schema_name: &str) -> Option<&SchemaShape> {
        self.schemas.get(schema_name)
    }

    /// Whether a schema name is already taken (collision detection).
    pub fn contains(&self, schema_name: &str) -> bool {
        self.schemas.contains_key(schema_name)
    }

    /// The bundle name backing a schema, when bundle-backed.
    pub fn lookup(&self, schema_name: &str) -> Option<&str> {
        match self.schemas.get(schema_name) {
            Some(SchemaShape::Bundle(b)) => Some(b.as_str()),
            _ => None,
        }
    }
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}
