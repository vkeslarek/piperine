//! The macro registry (plugin-interface v2, PLG-05/06): `#[pip::…]`
//! expansions submit declaration records here (life-before-main, inside the
//! binary the plugin's code ships in). A native cdylib's registry holds
//! exactly that plugin's declarations — the host reads them through
//! `Plugin::collect`, whose default body is compiled into the plugin's own
//! code (vtable dispatch), so attribution across dlopen needs no extra
//! symbols. An in-process host (`PluginHost::from_plugins`) shares one
//! registry with the plugins compiled into its binary.

use crate::contributions::DeviceFactory;

/// One `#[pip::device("Type")]` declaration: the `@device(type = …)` id
/// plus a constructor for the generated factory.
pub struct DeviceRegistration {
    pub type_id: &'static str,
    pub make: fn() -> Box<dyn DeviceFactory>,
}

impl DeviceRegistration {
    pub const fn new(type_id: &'static str, make: fn() -> Box<dyn DeviceFactory>) -> Self {
        Self { type_id, make }
    }
}

inventory::collect!(DeviceRegistration);

/// The read side of the macro registry. Zero-sized: every method reads the
/// calling binary's own registry (see the module doc for attribution).
pub struct Registry;

impl Registry {
    /// Every `#[pip::device]` declaration in the calling binary.
    pub fn devices() -> impl Iterator<Item = &'static DeviceRegistration> {
        inventory::iter::<DeviceRegistration>.into_iter()
    }
}
