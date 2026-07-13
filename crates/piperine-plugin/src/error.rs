//! [`PluginError`] — the P0xxx catalog (SPEC Part VI §12). Every failed or
//! denied plugin path is one of these; nothing plugin-related fails silently.

/// Plugin errors, code range P0xxx (distinct from parse E1xxx, elaboration
/// E2xxx, reflection E3xxx).
#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum PluginError {
    #[error("plugin `{0}` is untrusted (TOFU pending — run interactively or record a trust decision)")]
    #[diagnostic(code(P0001))]
    Untrusted(String),

    #[error("plugin `{plugin}` used capability `{capability}` not declared in its manifest")]
    #[diagnostic(code(P0002))]
    UndeclaredCapability { plugin: String, capability: String },

    #[error("schema `{schema}` already registered by `{existing}`; `{plugin}` cannot re-register it")]
    #[diagnostic(code(P0003))]
    SchemaConflict { schema: String, existing: String, plugin: String },

    #[error("device type `{0}` is not registered by any loaded plugin")]
    #[diagnostic(code(P0004))]
    DeviceNotRegistered(String),

    #[error("hook `{hook}` failed in plugin `{plugin}`: {message}")]
    #[diagnostic(code(P0005))]
    HookFailed { hook: &'static str, plugin: String, message: String },

    #[error("plugin `{plugin}`: bad manifest: {reason}")]
    #[diagnostic(code(P0006))]
    BadManifest { plugin: String, reason: String },

    #[error("plugin `{plugin}`: artifact hash does not match the trusted hash in Piperine.lock")]
    #[diagnostic(code(P0007))]
    HashMismatch { plugin: String },

    #[error("plugins `{a}` and `{b}` staged conflicting changes at `{path}`")]
    #[diagnostic(code(P0008))]
    StagingConflict { a: String, b: String, path: String },

    #[error("`{0}` is not a builtin command or a script registered by any loaded plugin")]
    #[diagnostic(code(P0009))]
    UnknownScript(String),

    #[error("plugin `{plugin}`: {message}")]
    #[diagnostic(code(P0099))]
    Other { plugin: String, message: String },
}

pub type PluginResult<T> = Result<T, PluginError>;
