//! `piperine-plugin.toml` — the plugin manifest (SPEC Part VI §4).
//! Intentionally minimal: identity, artifact location, and permissions.
//! Devices, schemas, tasks, and scripts are declared in code at
//! registration time (Plugin plan D1), never duplicated here.

use std::path::Path;

use serde::Deserialize;

use crate::error::PluginError;

/// The backend a plugin artifact runs under (SPEC Part VI §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Abi {
    /// Sandboxed WASM module (default for authors; backend lands in a
    /// later phase).
    Wasm,
    /// In-process shared library — full trust, TOFU required.
    Native,
    /// Out-of-process JSON-RPC executable (later phase).
    Process,
}

impl Abi {
    pub fn as_str(&self) -> &'static str {
        match self {
            Abi::Wasm => "wasm",
            Abi::Native => "native",
            Abi::Process => "process",
        }
    }
}

/// Capability declarations — deny-by-default (SPEC Part VI §3.3).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Permissions {
    /// Glob patterns the plugin may read/write (`"read *.cir"`,
    /// `"write *.ppr"`), relative to the project root.
    #[serde(default)]
    pub filesystem: Vec<String>,
    #[serde(default)]
    pub network: bool,
    /// Whitelist of executables the plugin may spawn; empty = none.
    #[serde(default)]
    pub process_spawn: Vec<String>,
    /// Per-hook-invocation timeout for WASM (milliseconds).
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_timeout() -> u64 {
    5000
}

impl Default for Permissions {
    fn default() -> Self {
        Self {
            filesystem: Vec::new(),
            network: false,
            process_spawn: Vec::new(),
            timeout_ms: default_timeout(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestFile {
    plugin: PluginSection,
    #[serde(default)]
    permissions: Option<Permissions>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PluginSection {
    name: String,
    abi: Abi,
    entry: String,
    #[serde(default)]
    description: Option<String>,
}

/// The parsed, validated manifest the host carries for a plugin's lifetime.
#[derive(Debug, Clone)]
pub struct Manifest {
    pub name: String,
    pub abi: Abi,
    /// Artifact path relative to the plugin root.
    pub entry: String,
    pub description: Option<String>,
    pub permissions: Permissions,
}

impl Manifest {
    /// Parse a manifest from TOML text. Malformed or incomplete manifests
    /// are `P0006 BadManifest` — validated before any plugin code runs.
    pub fn parse(name_hint: &str, text: &str) -> Result<Self, PluginError> {
        let bad = |reason: String| PluginError::BadManifest { plugin: name_hint.to_string(), reason };
        let file: ManifestFile = toml::from_str(text).map_err(|e| bad(e.to_string()))?;
        if file.plugin.name.is_empty() {
            return Err(bad("`plugin.name` must not be empty".into()));
        }
        if file.plugin.entry.is_empty() {
            return Err(bad("`plugin.entry` must not be empty".into()));
        }
        Ok(Self {
            name: file.plugin.name,
            abi: file.plugin.abi,
            entry: file.plugin.entry,
            description: file.plugin.description,
            permissions: file.permissions.unwrap_or_default(),
        })
    }

    /// Load `piperine-plugin.toml` from a resolved plugin directory.
    pub fn load(name_hint: &str, plugin_root: &Path) -> Result<Self, PluginError> {
        let path = plugin_root.join("piperine-plugin.toml");
        let text = std::fs::read_to_string(&path).map_err(|e| PluginError::BadManifest {
            plugin: name_hint.to_string(),
            reason: format!("{}: {e}", path.display()),
        })?;
        Self::parse(name_hint, &text)
    }
}
