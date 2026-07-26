//! The scripted-plugin seam (plugin-interface v2, PLG-06/10): a scripted
//! (`python = "…"`) plugin is loaded by an embedded-Python host
//! implementing this contract — piperine-plugin itself never embeds an
//! interpreter.

use std::path::Path;

use crate::manifest::Manifest;
use crate::Plugin;

/// Loads a scripted plugin: exec its `python` entry, read back its
/// decorator declarations, and return the plugin object whose script
/// handlers and hooks dispatch into the embedded interpreter.
pub trait ScriptedHost {
    fn load_scripted(
        &self,
        plugin_root: &Path,
        manifest: &Manifest,
    ) -> Result<Box<dyn Plugin>, String>;
}
