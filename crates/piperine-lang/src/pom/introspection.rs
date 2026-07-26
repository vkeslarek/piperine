//! Device-introspection metadata sidecar (phdl-introspection-attributes,
//! design.md component 2). A per-module resolved bundle of the author-declared
//! `@model`/`@name`/`@unit`/`@description`/`@kind` attributes, built from POM
//! nodes by [`Design::introspection_meta`](crate::pom::Design::introspection_meta)
//! and consumed by `piperine-codegen`'s `Introspect` bridge.
//!
//! This module lives in `piperine-lang` (the POM reflection layer) and holds
//! the metadata as **validated strings**, not solver ABI enums: `piperine-lang`'s
//! library does not depend on `piperine-solver` (solver/codegen are
//! dev-dependencies only, see `piperine-lang/Cargo.toml`). Codegen maps these
//! canonical strings onto `ObservableKind`/`TerminalKind` at the bridge, where
//! the solver enums are already in scope. The accepted-value tables below are
//! the validation source of truth in lang; they mirror (lowercase) the solver
//! enum variants in `crates/piperine-solver/src/core/introspect.rs`.

use std::collections::HashMap;

/// Accepted `@kind(value)` strings on a `var` — the lowercased
/// `ObservableKind` variants. Codegen maps each onto the matching solver enum.
pub const VAR_KINDS: &[&str] = &["branchcurrent", "charge", "flux", "state", "var"];

/// Accepted `@kind(value)` strings on a port or wire — the lowercased
/// `TerminalKind` variants. Codegen maps each onto the matching solver enum.
pub const TERMINAL_KINDS: &[&str] = &["external", "internal", "auxiliary"];

/// Author-declared model identity (PIA-01) — `@model(type = ..., version = ...)`
/// on a module. `None` when the module carries no `@model`; codegen then falls
/// back to the module-name echo (PIA-02).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ModelId {
    pub type_id: String,
    pub version: String,
}

/// Per-`var` introspection metadata (PIA-05..09). `@name`/`@unit`/`@description`
/// annotate the opvar-query entry; `@name`/`@kind` annotate the observable
/// catalog entry — one `@name` feeds both catalogs (PIA-07). Every field is
/// optional; absent → codegen-derived default (PIA-08).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VarMeta {
    pub name: Option<String>,
    pub unit: Option<String>,
    pub description: Option<String>,
    /// Canonical lowercased `ObservableKind` variant name, validated at resolve.
    pub kind: Option<String>,
}

impl VarMeta {
    /// Whether any introspection field is set (the resolver stores only
    /// non-empty entries to keep the sidecar sparse).
    pub fn has_any(&self) -> bool {
        self.name.is_some() || self.unit.is_some() || self.description.is_some() || self.kind.is_some()
    }
}

/// Per-terminal (port/wire) introspection metadata (PIA-10..14): `@name`/`@kind`
/// classify and name the terminal; `@description` annotates it. `kind` is the
/// canonical lowercased `TerminalKind` variant name, validated at resolve.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TermMeta {
    pub name: Option<String>,
    pub kind: Option<String>,
    pub description: Option<String>,
}

impl TermMeta {
    /// Whether any introspection field is set.
    pub fn has_any(&self) -> bool {
        self.name.is_some() || self.kind.is_some() || self.description.is_some()
    }
}

/// A per-module resolved bundle of declared introspection metadata. Built by
/// [`Design::introspection_meta`](crate::pom::Design::introspection_meta);
/// consumed by codegen's `Introspect` bridge, which prefers a sidecar field
/// over the codegen-derived default and falls back when absent.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IntrospectionMeta {
    /// `@model` on the module (PIA-01).
    pub model: Option<ModelId>,
    /// Keyed by the POM `var` name (the kernel var id); the `@name(value)` is
    /// the display name held inside [`VarMeta::name`].
    pub vars: HashMap<String, VarMeta>,
    /// Keyed by the POM port/wire name; the `@name(value)` is the display name
    /// held inside [`TermMeta::name`].
    pub terminals: HashMap<String, TermMeta>,
}

impl IntrospectionMeta {
    /// Whether the sidecar carries any declared metadata at all.
    pub fn is_empty(&self) -> bool {
        self.model.is_none() && self.vars.is_empty() && self.terminals.is_empty()
    }
}
