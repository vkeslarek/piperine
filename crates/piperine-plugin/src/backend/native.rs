//! Native backend (SPEC Part VI §6.2): the plugin is a shared library
//! loaded in-process via dlopen. Full trust — the security posture comes
//! from TOFU + content hash + capabilities, not isolation.
//!
//! The library exports two C symbols:
//!
//! ```c
//! uint32_t piperine_plugin_abi_version(void);   // must equal ABI_VERSION
//! void    *piperine_plugin_entry(void);         // Box<Box<dyn Plugin>> as raw
//! ```
//!
//! The double-box round-trips the fat trait pointer through a thin
//! `*mut c_void`. Host and plugin must be built by the same Rust toolchain —
//! documented native-tier contract; the WASM/process backends carry no such
//! constraint.

use std::ffi::c_void;
use std::path::Path;

use libloading::Library;

use crate::error::{PluginError, PluginResult};
use crate::Plugin;

/// The native ABI contract version. Bumped on any breaking change to the
/// `Plugin` trait object surface.
pub const ABI_VERSION: u32 = 1;

pub(crate) const ENTRY_SYMBOL: &[u8] = b"piperine_plugin_entry";
pub(crate) const ABI_SYMBOL: &[u8] = b"piperine_plugin_abi_version";

/// A loaded native plugin. The backing library is intentionally **never
/// unloaded** (`dlclose` on a Rust cdylib is unsound — TLS destructors and
/// fini sections crash after unload; every plugin host leaks the handle and
/// keeps the code mapped for the process lifetime).
pub struct NativePlugin {
    pub plugin: Box<dyn Plugin>,
}

/// dlopen `artifact`, verify the ABI version, and take ownership of the
/// plugin the entry symbol returns.
pub fn load(name: &str, artifact: &Path) -> PluginResult<NativePlugin> {
    let err = |message: String| PluginError::Other { plugin: name.to_string(), message };
    let lib = unsafe { Library::new(artifact) }
        .map_err(|e| err(format!("loading {}: {e}", artifact.display())))?;

    let abi: libloading::Symbol<unsafe extern "C" fn() -> u32> =
        unsafe { lib.get(ABI_SYMBOL) }.map_err(|e| err(format!("missing ABI symbol: {e}")))?;
    let version = unsafe { abi() };
    if version != ABI_VERSION {
        return Err(err(format!(
            "ABI version mismatch: plugin has {version}, host expects {ABI_VERSION}"
        )));
    }

    let entry: libloading::Symbol<unsafe extern "C" fn() -> *mut c_void> =
        unsafe { lib.get(ENTRY_SYMBOL) }.map_err(|e| err(format!("missing entry symbol: {e}")))?;
    let raw = unsafe { entry() };
    if raw.is_null() {
        return Err(err("entry symbol returned null".into()));
    }
    let plugin: Box<dyn Plugin> = *unsafe { Box::from_raw(raw as *mut Box<dyn Plugin>) };
    // Leak the handle: the plugin's code (vtables, fns) must stay mapped for
    // the process lifetime — see the `NativePlugin` doc.
    std::mem::forget(lib);
    Ok(NativePlugin { plugin })
}
