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
    /// The declaration span of this field's own name, so `@device(plugin =
    /// ...)`'s `plugin` field resolves independently of the schema name
    /// (declared-language-surface DLS-13/14 groundwork — populated once a
    /// field originates from a textual `extern attribute` declaration;
    /// `None` for host/plugin-registered fields with no textual source).
    pub decl_span: Option<miette::SourceSpan>,
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
    /// The declaration span of the schema *name* itself (as opposed to
    /// `AttrField::decl_span`, one per field) — populated only for
    /// `extern attribute` declarations (declared-language-surface T14),
    /// so `@name(...)` itself (not just its fields) is goto-def-able.
    /// `None` for bundle-backed schemas (their span is the bundle's own,
    /// already resolved via `design.bundles()`) and host/plugin-registered
    /// schemas with no textual source.
    decl_spans: HashMap<String, miette::SourceSpan>,
}

impl SchemaRegistry {
    pub fn new() -> Self {
        Self { schemas: HashMap::new(), decl_spans: HashMap::new() }
    }

    /// Register a schema name backed by the named bundle's fields.
    pub fn register(&mut self, schema_name: &str, bundle_name: &str) {
        self.schemas.insert(schema_name.to_string(), SchemaShape::Bundle(bundle_name.to_string()));
    }

    /// Register a schema name with directly-declared fields (host/plugins),
    /// optionally with the declaration's own `decl_span` (populated for
    /// `extern attribute`; `None` for host/plugin-registered schemas with
    /// no textual source, e.g. the built-in `@rfport`).
    pub fn register_declared(&mut self, schema_name: &str, fields: Vec<AttrField>, decl_span: Option<miette::SourceSpan>) {
        self.schemas.insert(schema_name.to_string(), SchemaShape::Declared(fields));
        if let Some(span) = decl_span {
            self.decl_spans.insert(schema_name.to_string(), span);
        }
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

    /// The schema name's own `decl_span`, when known (T14 — LSP indexing).
    pub fn decl_span(&self, schema_name: &str) -> Option<miette::SourceSpan> {
        self.decl_spans.get(schema_name).copied()
    }
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}
