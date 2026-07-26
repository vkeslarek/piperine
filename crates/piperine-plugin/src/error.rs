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

    /// Plugin-interface v2 (PLG-02): a manifest declaring a removed
    /// backend (`abi = "wasm"|"process"`) gets a targeted removal error —
    /// never a generic unknown-field/unknown-value one.
    #[error("plugin `{plugin}`: the `{backend}` backend was removed — plugins are native + Python only (MD-21); a device plugin declares `device = {{ … }}`, a scripted plugin `python = \"…\"`")]
    #[diagnostic(code(P0011))]
    RemovedBackend { plugin: String, backend: String },

    /// Plugin-interface v2 (PLG-19): no release asset matches the host
    /// target triple — prebuilt-binary only, no build-from-source (D6).
    #[error("plugin `{plugin}`: no release asset for target triple `{triple}` in release `{release}` (prebuilt-binary only)")]
    #[diagnostic(code(P0012))]
    NoAssetForTriple { plugin: String, triple: String, release: String },

    /// Plugin-interface v2 (PLG-18): the fetched asset's hash does not
    /// match the manifest's `verify` hash — a hard fail, never a prompt.
    #[error("plugin `{plugin}`: fetched asset for `{release}` does not match the manifest's `verify` hash")]
    #[diagnostic(code(P0013))]
    VerifyMismatch { plugin: String, release: String },

    /// Plugin-interface v2 (PLG-23): the user denied the declared
    /// `[plugin.permissions]` at `add` — the install aborts.
    #[error("plugin `{0}`: declared permissions denied — install aborted")]
    #[diagnostic(code(P0014))]
    PermissionsDenied(String),

    #[error("plugin `{plugin}`: {message}")]
    #[diagnostic(code(P0099))]
    Other { plugin: String, message: String },
}

pub type PluginResult<T> = Result<T, PluginError>;
