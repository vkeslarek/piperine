//! The contribution surface (plugin-interface v2, PLG-04/05/06): a plugin's
//! `#[pip::…]` attributes submit declaration records into its binary's
//! registry; `Plugin::collect` snapshots them into a [`Declared`], which the
//! host merges into its [`Contributions`] at load. Declaration and
//! registration are one attribute — there is no imperative `register()`
//! entry point.
//!
//! Plugins contribute **no attribute schemas** (PLG-08): `@device`/`@port`
//! (stdlib, `headers/device_port.phdl`) are the only plugin-facing schema
//! names.

use std::collections::HashMap;

use piperine_codegen::device::PluginDeviceSpec;
use piperine_solver::abi::Element;

use crate::capability::HostCtx;
use crate::error::{PluginError, PluginResult};
use crate::registry::{HookRegistration, Registry};

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

/// The declaration target of `#[pip::device("Type")]` (plugin-interface v2,
/// PLG-05): the annotated `Element` type implements this to build one solver
/// element from a `@device` instance's resolved ports/params. The macro
/// generates the [`DeviceFactory`] adapter and the registry submission —
/// declaration and registration are one attribute, never an imperative call.
pub trait PluginDevice: Element + Sized + 'static {
    /// Analog/digital/mixed — surfaced through the generated factory.
    const KIND: DeviceKind;
    /// Build one solver element from the `@device` instance's spec.
    fn from_spec(spec: &PluginDeviceSpec) -> Result<Self, String>;
}

/// A plugin-contributed CLI subcommand (SPEC Part VI §10).
pub trait ScriptHandler: Send + Sync {
    /// Run the script with its CLI arguments; the return value becomes the
    /// process exit code.
    fn invoke(&self, args: &[String], cx: &mut HostCtx) -> Result<i32, String>;
}

/// One declared device: the `@device(type = …)` id plus its factory.
pub struct DeclaredDevice {
    pub type_id: String,
    pub factory: Box<dyn DeviceFactory>,
    /// Stable identity of the declaration, used to skip a re-collected
    /// duplicate (an in-process binary shares one registry across plugins).
    origin: usize,
}

impl DeclaredDevice {
    /// A device declared outside the macro registry (an embedded bridge).
    pub fn new(type_id: &str, factory: Box<dyn DeviceFactory>) -> Self {
        let origin = &*factory as *const dyn DeviceFactory as *const () as usize;
        Self { type_id: type_id.to_string(), factory, origin }
    }
}

/// One declared script: the CLI subcommand name plus its handler.
pub struct DeclaredScript {
    pub name: String,
    pub handler: Box<dyn ScriptHandler>,
    origin: usize,
}

impl DeclaredScript {
    /// A script declared outside the macro registry (an embedded bridge).
    pub fn new(name: &str, handler: Box<dyn ScriptHandler>) -> Self {
        let origin = &*handler as *const dyn ScriptHandler as *const () as usize;
        Self { name: name.to_string(), handler, origin }
    }
}

/// The declarative snapshot `Plugin::collect` hands the host: everything the
/// plugin declared via `#[pip::…]` (or an embedded bridge's equivalent).
#[derive(Default)]
pub struct Declared {
    pub devices: Vec<DeclaredDevice>,
    pub scripts: Vec<DeclaredScript>,
    pub hooks: Vec<HookRegistration>,
}

impl Declared {
    /// The default [`crate::Plugin::collect`] body: drain the `#[pip::…]`
    /// registry of the calling binary (see `registry.rs` for attribution).
    pub fn from_registries() -> Self {
        Self {
            devices: Registry::devices()
                .map(|r| {
                    let origin = r.make as usize;
                    DeclaredDevice { type_id: r.type_id.to_string(), factory: (r.make)(), origin }
                })
                .collect(),
            scripts: Registry::scripts()
                .map(|r| {
                    let origin = r.make as usize;
                    DeclaredScript { name: r.name.to_string(), handler: (r.make)(), origin }
                })
                .collect(),
            hooks: Registry::hooks()
                .map(|r| HookRegistration { phase: r.phase, invoke: r.invoke })
                .collect(),
        }
    }
}

/// A merged device contribution: the owning plugin plus its factory.
pub struct DeviceContribution {
    pub plugin: String,
    pub factory: Box<dyn DeviceFactory>,
    origin: usize,
}

/// A merged script contribution: the owning plugin plus its handler.
pub struct ScriptContribution {
    pub plugin: String,
    pub handler: Box<dyn ScriptHandler>,
    origin: usize,
}

/// A merged hook contribution: the owning plugin, its phase, and the
/// macro-generated dispatch wrapper.
pub struct HookContribution {
    pub plugin: String,
    pub phase: crate::HookPhase,
    pub invoke: fn(&crate::HookCall<'_>) -> Result<(), String>,
}

/// The merged, collision-checked snapshot of everything loaded plugins
/// declare. Owned by the host; queried at pipeline boundaries.
#[derive(Default)]
pub struct Contributions {
    /// device type id → contribution.
    pub devices: HashMap<String, DeviceContribution>,
    /// script (CLI subcommand) name → contribution.
    pub scripts: HashMap<String, ScriptContribution>,
    /// Declared hooks, fired at their phases in load order.
    pub hooks: Vec<HookContribution>,
}

impl Contributions {
    /// Merge one plugin's [`Declared`] snapshot under its name. A distinct
    /// declaration reusing an occupied device type id or script name is a
    /// loud P0003 naming both plugins; re-collecting the *same* declaration
    /// (an in-process binary's shared registry) is idempotent, never a
    /// conflict.
    pub(crate) fn merge(&mut self, plugin: &str, declared: Declared) -> PluginResult<()> {
        for d in declared.devices {
            if let Some(existing) = self.devices.get(&d.type_id) {
                if existing.origin == d.origin {
                    continue;
                }
                return Err(PluginError::SchemaConflict {
                    schema: format!("device `{}`", d.type_id),
                    existing: existing.plugin.clone(),
                    plugin: plugin.to_string(),
                });
            }
            self.devices.insert(
                d.type_id.clone(),
                DeviceContribution { plugin: plugin.to_string(), factory: d.factory, origin: d.origin },
            );
        }
        for s in declared.scripts {
            if let Some(existing) = self.scripts.get(&s.name) {
                if existing.origin == s.origin {
                    continue;
                }
                return Err(PluginError::SchemaConflict {
                    schema: format!("script `{}`", s.name),
                    existing: existing.plugin.clone(),
                    plugin: plugin.to_string(),
                });
            }
            self.scripts.insert(
                s.name.clone(),
                ScriptContribution { plugin: plugin.to_string(), handler: s.handler, origin: s.origin },
            );
        }
        for h in declared.hooks {
            let duplicate = self
                .hooks
                .iter()
                .any(|e| e.phase == h.phase && e.invoke as usize == h.invoke as usize);
            if !duplicate {
                self.hooks.push(HookContribution {
                    plugin: plugin.to_string(),
                    phase: h.phase,
                    invoke: h.invoke,
                });
            }
        }
        Ok(())
    }
}
