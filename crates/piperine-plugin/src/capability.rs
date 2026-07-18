//! [`HostCtx`] — the capability facade (SPEC Part VI §3.3/§9): every
//! side-effecting call a plugin makes goes through here and is checked
//! against its manifest permissions. Denials are `P0002
//! UndeclaredCapability`, never a crash.

use std::path::{Path, PathBuf};

use crate::error::{PluginError, PluginResult};
use crate::manifest::Permissions;

/// The host context handed to hooks and scripts. Carries the owning
/// plugin's permissions and the project root every path resolves against.
pub struct HostCtx {
    plugin: String,
    project_root: PathBuf,
    permissions: Permissions,
}

impl HostCtx {
    pub(crate) fn new(plugin: &str, project_root: &Path, permissions: Permissions) -> Self {
        Self { plugin: plugin.to_string(), project_root: project_root.to_path_buf(), permissions }
    }

    /// The project root (where `Piperine.toml` lives). Always available.
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    /// Route a message to the host logger. Always available.
    pub fn log(&self, message: &str) {
        eprintln!("[plugin {}] {message}", self.plugin);
    }

    /// Read a project file — requires a matching `"read <glob>"` filesystem
    /// permission. The path resolves relative to the project root and must
    /// stay inside it.
    pub fn fs_read(&self, path: &str) -> PluginResult<String> {
        let full = self.checked_path(path, "read")?;
        std::fs::read_to_string(&full).map_err(|e| PluginError::Other {
            plugin: self.plugin.clone(),
            message: format!("{}: {e}", full.display()),
        })
    }

    /// Write a project file — requires a matching `"write <glob>"`
    /// filesystem permission.
    pub fn fs_write(&self, path: &str, contents: &str) -> PluginResult<()> {
        let full = self.checked_path(path, "write")?;
        std::fs::write(&full, contents).map_err(|e| PluginError::Other {
            plugin: self.plugin.clone(),
            message: format!("{}: {e}", full.display()),
        })
    }

    /// Resolve `path` against the project root, confining it there, and
    /// check the `verb` (`"read"`/`"write"`) against the manifest globs.
    fn checked_path(&self, path: &str, verb: &str) -> PluginResult<PathBuf> {
        let deny = || PluginError::UndeclaredCapability {
            plugin: self.plugin.clone(),
            capability: format!("filesystem {verb} {path}"),
        };
        if Path::new(path).is_absolute() || path.split('/').any(|seg| seg == "..") {
            return Err(deny());
        }
        let allowed = self.permissions.filesystem.iter().any(|entry| {
            entry
                .split_once(' ')
                .is_some_and(|(v, glob)| v == verb && glob_match(glob, path))
        });
        if !allowed {
            return Err(deny());
        }
        Ok(self.project_root.join(path))
    }
}

/// Minimal `*` glob (the only wildcard the manifest grammar defines):
/// `*.cir` matches any name ending in `.cir`, `out/*` any path under `out/`.
fn glob_match(glob: &str, path: &str) -> bool {
    fn inner(g: &[u8], p: &[u8]) -> bool {
        match (g.first(), p.first()) {
            (None, None) => true,
            (Some(b'*'), _) => inner(&g[1..], p) || (!p.is_empty() && inner(g, &p[1..])),
            (Some(c), Some(d)) if c == d => inner(&g[1..], &p[1..]),
            _ => false,
        }
    }
    inner(glob.as_bytes(), path.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::glob_match;

    #[test]
    fn globs() {
        assert!(glob_match("*.cir", "rectifier.cir"));
        assert!(glob_match("out/*", "out/a.phdl"));
        assert!(glob_match("*", "anything"));
        assert!(!glob_match("*.cir", "rectifier.phdl"));
        assert!(!glob_match("out/*", "elsewhere/a.phdl"));
    }
}
