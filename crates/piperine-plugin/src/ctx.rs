//! The hook/script context (plugin-interface v2, PLG-06/11, D14): one
//! `&Ctx` per decorator callback, name-identical to the Python `@pip` ctx
//! (MD-22). Read-only hooks expose the real `&Design`; capability-gated
//! filesystem access delegates to the plugin's [`HostCtx`].

use std::path::Path;

use piperine_lang::pom::Design;

use crate::capability::HostCtx;
use crate::error::PluginResult;

/// The context handed to `#[pip::script]`/`#[pip::hook]` functions.
pub struct Ctx<'a> {
    host: &'a HostCtx,
    design: Option<&'a Design>,
}

impl<'a> Ctx<'a> {
    pub fn new(host: &'a HostCtx, design: Option<&'a Design>) -> Self {
        Self { host, design }
    }

    /// The elaborated design, read-only (MD-25). Available in the
    /// `after_elaborate`/`transform_design`/`before_lower` hooks; calling it
    /// from `after_parse`, `after_solve`, or a script is a programming
    /// error — a loud panic naming the contract, never a silent empty
    /// design.
    pub fn design(&self) -> &Design {
        self.design.expect(
            "ctx.design() is only available in the after_elaborate, transform_design, and \
             before_lower hooks",
        )
    }

    /// The project root (where `Piperine.toml` lives). Always available.
    pub fn project_root(&self) -> &Path {
        self.host.project_root()
    }

    /// Route a message to the host logger. Always available.
    pub fn log(&self, message: &str) {
        self.host.log(message);
    }

    /// Read a project file — requires a matching `"read <glob>"` filesystem
    /// permission (P0002 on denial).
    pub fn fs_read(&self, path: &str) -> PluginResult<String> {
        self.host.fs_read(path)
    }

    /// Write a project file — requires a matching `"write <glob>"`
    /// filesystem permission (P0002 on denial).
    pub fn fs_write(&self, path: &str, contents: &str) -> PluginResult<()> {
        self.host.fs_write(path, contents)
    }
}
