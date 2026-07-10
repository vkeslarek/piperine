//! Piperine plugin SDK + host (SPEC Part VI; engineering plan in
//! `Plugin plan.md`).
//!
//! A plugin implements [`Plugin`]: a [`Manifest`] accessor plus a
//! `register()` that contributes devices and attribute schemas through the
//! [`Registrar`]. The host ([`PluginHost`]) discovers plugins from
//! `Piperine.toml [plugins]`, verifies them (TOFU + content hash), loads
//! them (native dlopen today; WASM/process later), and answers the
//! pipeline's queries: schema seeding at elaboration, device construction
//! at circuit build.
//!
//! The device ABI is Piperine's own `AnalogDevice`/`DigitalDevice` trait
//! pair — never OSDI or any external model ABI (Plugin plan D13).

mod backend;
mod contributions;
mod error;
mod host;
mod manifest;
mod trust;

pub use backend::native::ABI_VERSION;
pub use contributions::{Contributions, DeviceFactory, DeviceKind, Registrar};
pub use error::{PluginError, PluginResult};
pub use host::PluginHost;
pub use manifest::{Abi, Manifest, Permissions};
pub use trust::{artifact_hash, ensure_trusted as trust_check, TrustMode};

// Re-exported so plugin authors depend on one crate for the whole contract.
pub use piperine_codegen::device::{PluginDeviceSpec, PluginPort, PortBinding};
pub use piperine_lang::elab::registry::{AttrField, ElabContext};

/// The plugin contract (SPEC Part VI §6). Lifecycle hooks (§8) are added in
/// a later phase — additively, as defaulted methods.
pub trait Plugin: Send + Sync {
    fn manifest(&self) -> &Manifest;

    /// Contribute devices and attribute schemas. Runs once at load time,
    /// before elaboration; every contribution is optional.
    fn register(&self, r: &mut Registrar) {
        let _ = r;
    }
}

/// Box a plugin for the native entry symbol. A native plugin's crate writes:
///
/// ```ignore
/// #[unsafe(no_mangle)]
/// pub extern "C" fn piperine_plugin_abi_version() -> u32 { piperine_plugin::ABI_VERSION }
///
/// #[unsafe(no_mangle)]
/// pub extern "C" fn piperine_plugin_entry() -> *mut core::ffi::c_void {
///     piperine_plugin::entry(MyPlugin::new())
/// }
/// ```
pub fn entry(plugin: impl Plugin + 'static) -> *mut core::ffi::c_void {
    Box::into_raw(Box::new(Box::new(plugin) as Box<dyn Plugin>)) as *mut core::ffi::c_void
}
