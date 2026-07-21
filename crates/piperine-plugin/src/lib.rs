//! Piperine plugin SDK + host (SPEC Part VI; engineering plan in
//! `Plugin plan.md`).
//!
//! A plugin implements [`Plugin`]: a [`Manifest`] accessor plus a
//! `register()` that contributes devices, attribute schemas, and CLI
//! scripts through the
//! [`Registrar`]. The host ([`PluginHost`]) discovers plugins from
//! `Piperine.toml [plugins]`, verifies them (TOFU + content hash), loads
//! them (native dlopen today; WASM/process later), and answers the
//! pipeline's queries: schema seeding at elaboration, device construction
//! at circuit build.
//!
//! The device ABI is Piperine's own unified `Element` trait — one contract that
//! declares analog and/or digital capabilities — never OSDI or any external
//! model ABI (Plugin plan D13).

mod backend;
mod capability;
mod contributions;
mod error;
mod host;
mod manifest;
mod trust;
mod view;

pub use backend::native::ABI_VERSION;
pub use capability::HostCtx;
pub use contributions::{Contributions, DeviceFactory, DeviceKind, Registrar, ScriptHandler};
pub use error::{PluginError, PluginResult};
pub use host::PluginHost;
pub use manifest::{Abi, Manifest, Permissions};
pub use trust::{artifact_hash, ensure_trusted as trust_check, TrustMode};
pub use view::{DesignStaging, SolveResultView};

// Re-exported so plugin authors depend on one crate for the whole contract.
pub use piperine_codegen::device::{PluginDeviceSpec, PluginPort, PortBinding};
pub use piperine_lang::elab::registry::{AttrField, ElabContext};
pub use piperine_lang::pom::Design;

/// The plugin contract (SPEC Part VI §6/§8). Every contribution and hook is
/// optional; hooks default to no-ops. Read-only hooks receive **the real
/// POM** (`&Design`, SPEC Part IV) — no parallel view model; in-process
/// plugins reflect over the same structure the rest of the pipeline sees.
/// `after_lower` is deliberately absent until a real consumer exists (D12).
pub trait Plugin: Send + Sync {
    fn manifest(&self) -> &Manifest;

    /// Contribute devices, attribute schemas, and scripts. Runs once at
    /// load time, before elaboration.
    fn register(&self, r: &mut Registrar) {
        let _ = r;
    }

    /// Hook 1: after parsing, before elaboration — raw source, read-only.
    fn after_parse(&self, cx: &mut HostCtx, source: &str) -> PluginResult<()> {
        let _ = (cx, source);
        Ok(())
    }

    /// Hook 2: once the `Design` is elaborated — read-only.
    fn after_elaborate(&self, cx: &mut HostCtx, design: &Design) -> PluginResult<()> {
        let _ = (cx, design);
        Ok(())
    }

    /// Hook 3: before an analysis consumes staged overrides — the one
    /// mutable hook, through the staging surface (SPEC Part VI §8.2).
    fn transform_design(&self, cx: &mut HostCtx, staging: &DesignStaging<'_>) -> PluginResult<()> {
        let _ = (cx, staging);
        Ok(())
    }

    /// Hook 4: the applied design, just before body lowering — read-only.
    fn before_lower(&self, cx: &mut HostCtx, design: &Design) -> PluginResult<()> {
        let _ = (cx, design);
        Ok(())
    }

    /// Hook 7: after an analysis, with its kind and (for `$op`) the solved
    /// node voltages.
    fn after_solve(&self, cx: &mut HostCtx, result: &SolveResultView) -> PluginResult<()> {
        let _ = (cx, result);
        Ok(())
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
