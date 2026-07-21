//! The registration surface (SPEC Part VI §6): a plugin's `register()` fills
//! a passive [`Contributions`] snapshot through a [`Registrar`]. No plugin
//! code runs during elaboration or solve — the host consults the snapshot
//! (Plugin plan D2).

use std::collections::HashMap;

use piperine_codegen::device::PluginDeviceSpec;
use piperine_lang::elab::registry::AttrField;
use piperine_solver::abi::Element;

use crate::capability::HostCtx;
use crate::error::PluginError;

/// What kind of device a factory produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    Analog,
    Digital,
    Mixed,
}

/// Constructs one solver [`Element`] per `@device`-annotated instance
/// (SPEC Part VI §7.3). The returned element implements Piperine's own unified
/// mixed-signal ABI — the single `Element` contract, declaring analog and/or
/// digital capabilities — never an external model ABI (Plugin plan D13).
pub trait DeviceFactory: Send + Sync {
    fn kind(&self) -> DeviceKind;
    fn instantiate(&self, spec: &PluginDeviceSpec) -> Result<Box<dyn Element>, String>;
}

/// A plugin-contributed CLI subcommand (SPEC Part VI §10).
pub trait ScriptHandler: Send + Sync {
    /// Run the script with its CLI arguments; the return value becomes the
    /// process exit code.
    fn invoke(&self, args: &[String], cx: &mut HostCtx) -> Result<i32, String>;
}

/// The merged, collision-checked snapshot of everything loaded plugins
/// contribute. Owned by the host; queried at pipeline boundaries.
#[derive(Default)]
pub struct Contributions {
    /// schema name → (owning plugin, declared fields).
    pub schemas: HashMap<String, (String, Vec<AttrField>)>,
    /// device type id → (owning plugin, factory).
    pub devices: HashMap<String, (String, Box<dyn DeviceFactory>)>,
    /// script (CLI subcommand) name → (owning plugin, handler).
    pub scripts: HashMap<String, (String, Box<dyn ScriptHandler>)>,
}

/// The builder a plugin's `register()` receives. Records contributions under
/// the registering plugin's name; collisions are collected and surface as
/// `P0003 SchemaConflict` after registration (the `register()` signature
/// stays infallible for plugin authors).
pub struct Registrar<'a> {
    plugin: String,
    contributions: &'a mut Contributions,
    errors: &'a mut Vec<PluginError>,
}

impl<'a> Registrar<'a> {
    pub(crate) fn new(
        plugin: &str,
        contributions: &'a mut Contributions,
        errors: &'a mut Vec<PluginError>,
    ) -> Self {
        Self { plugin: plugin.to_string(), contributions, errors }
    }

    /// Contribute an attribute schema (SPEC Part VI §11). The name joins the
    /// same registry `@attribute(schema = …)` bundles use; a collision with
    /// another plugin (or the builtin `device`/`port` schemas) is P0003.
    pub fn attr_schema(&mut self, name: &str, fields: Vec<AttrField>) {
        if let Some((existing, _)) = self.contributions.schemas.get(name) {
            self.errors.push(PluginError::SchemaConflict {
                schema: name.to_string(),
                existing: existing.clone(),
                plugin: self.plugin.clone(),
            });
            return;
        }
        self.contributions.schemas.insert(name.to_string(), (self.plugin.clone(), fields));
    }

    /// Contribute a CLI subcommand (SPEC Part VI §10): `piperine <name> …`
    /// dispatches to `handler` when no builtin command matches.
    pub fn script(&mut self, name: &str, handler: Box<dyn ScriptHandler>) {
        if let Some((existing, _)) = self.contributions.scripts.get(name) {
            self.errors.push(PluginError::SchemaConflict {
                schema: format!("script `{name}`"),
                existing: existing.clone(),
                plugin: self.plugin.clone(),
            });
            return;
        }
        self.contributions.scripts.insert(name.to_string(), (self.plugin.clone(), handler));
    }

    /// Contribute a device factory for a `@device(type = …)` type id
    /// (SPEC Part VI §7).
    pub fn device(&mut self, type_id: &str, factory: Box<dyn DeviceFactory>) {
        if let Some((existing, _)) = self.contributions.devices.get(type_id) {
            self.errors.push(PluginError::SchemaConflict {
                schema: format!("device `{type_id}`"),
                existing: existing.clone(),
                plugin: self.plugin.clone(),
            });
            return;
        }
        self.contributions.devices.insert(type_id.to_string(), (self.plugin.clone(), factory));
    }
}
