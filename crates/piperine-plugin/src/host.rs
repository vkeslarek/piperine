//! [`PluginHost`] — the one orchestrator: discover → verify → load →
//! register → dispatch. An empty `[plugins]` section yields an inert host;
//! the zero-plugin path costs one `is_empty` check.

use std::path::Path;

use piperine_codegen::device::{DeviceProvider, PluginDeviceSpec};
use piperine_lang::elab::registry::{AttrField, ElabContext};
use piperine_project::resolver::Resolver;
use piperine_project::PiperineToml;
use piperine_solver::core::device::Device;

use crate::backend::native::{self, NativePlugin};
use crate::contributions::{Contributions, Registrar};
use crate::error::{PluginError, PluginResult};
use crate::manifest::{Abi, Manifest};
use crate::trust::{artifact_hash, ensure_trusted, TrustMode};
use crate::Plugin;

/// One loaded plugin: its manifest plus the (backend-owning) instance.
struct LoadedPlugin {
    manifest: Manifest,
    /// Keeps the plugin (and, for native, its library) alive. In-process
    /// plugins (tests, builtin hosts) have no library to hold.
    _instance: PluginInstance,
}

enum PluginInstance {
    Native(NativePlugin),
    InProcess(Box<dyn Plugin>),
}

/// The plugin host: loaded plugins in deterministic (alphabetical) order
/// plus their merged contributions.
pub struct PluginHost {
    plugins: Vec<LoadedPlugin>,
    contributions: Contributions,
}

impl PluginHost {
    /// An inert host — no plugins, every dispatch a no-op.
    pub fn empty() -> Self {
        Self { plugins: Vec::new(), contributions: Contributions::default() }
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// Loaded plugin names, alphabetical.
    pub fn plugin_names(&self) -> Vec<&str> {
        self.plugins.iter().map(|p| p.manifest.name.as_str()).collect()
    }

    /// Build a host from in-process plugin instances — the test/builtin
    /// path (no manifest file, no dlopen, no TOFU). Contributions are
    /// registered exactly as for loaded artifacts.
    pub fn from_plugins(plugins: Vec<Box<dyn Plugin>>) -> PluginResult<Self> {
        let mut host = Self::empty();
        for plugin in plugins {
            let manifest = plugin.manifest().clone();
            host.register_one(&manifest.name.clone(), PluginInstance::InProcess(plugin), manifest)?;
        }
        host.sort();
        Ok(host)
    }

    /// Discover, verify, and load every `[plugins]` entry of the project at
    /// `root` (SPEC Part VI §5): resolve sources, parse manifests (P0006),
    /// hash artifacts, run TOFU (P0001/P0007), dlopen, register (P0003).
    pub fn load_for_project(root: &Path, trust: TrustMode) -> PluginResult<Self> {
        let toml_path = root.join("Piperine.toml");
        let Ok(toml) = PiperineToml::load(&toml_path) else {
            return Ok(Self::empty());
        };
        if toml.plugins.is_empty() {
            return Ok(Self::empty());
        }

        let mut resolver = Resolver::new(root, false);
        let resolved = resolver.resolve_plugins(&toml).map_err(|e| PluginError::Other {
            plugin: "<resolver>".into(),
            message: e.to_string(),
        })?;

        let mut host = Self::empty();
        // Deterministic load order (SPEC Part VI §8.4).
        let mut names: Vec<&String> = resolved.keys().collect();
        names.sort();
        for name in names {
            let plugin_root = &resolved[name];
            let manifest = Manifest::load(name, plugin_root)?;
            let artifact = plugin_root.join(&manifest.entry);
            let hash = artifact_hash(&artifact)?;
            let source = toml
                .plugins
                .get(name)
                .map(|s| format!("{s:?}"))
                .unwrap_or_else(|| plugin_root.display().to_string());
            ensure_trusted(root, &manifest, &source, &hash, trust)?;
            let instance = match manifest.abi {
                Abi::Native => PluginInstance::Native(native::load(&manifest.name, &artifact)?),
                Abi::Wasm | Abi::Process => {
                    return Err(PluginError::Other {
                        plugin: manifest.name.clone(),
                        message: format!("`{}` backend is not implemented yet", manifest.abi.as_str()),
                    });
                }
            };
            host.register_one(&manifest.name.clone(), instance, manifest)?;
        }
        host.sort();
        Ok(host)
    }

    fn sort(&mut self) {
        self.plugins.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
    }

    /// Run one plugin's `register()` and merge its contributions.
    /// Contribution collisions surface here as P0003.
    fn register_one(
        &mut self,
        name: &str,
        instance: PluginInstance,
        manifest: Manifest,
    ) -> PluginResult<()> {
        let plugin: &dyn Plugin = match &instance {
            PluginInstance::Native(n) => n.plugin.as_ref(),
            PluginInstance::InProcess(p) => p.as_ref(),
        };
        let mut errors = Vec::new();
        plugin.register(&mut Registrar::new(name, &mut self.contributions, &mut errors));
        if let Some(err) = errors.into_iter().next() {
            return Err(err);
        }
        self.plugins.push(LoadedPlugin { manifest, _instance: instance });
        Ok(())
    }

    /// Seed the elaboration registries (Plugin plan D2): the plugin
    /// system's own `@device`/`@port` schemas, plus every plugin-declared
    /// schema. Called by whoever drives elaboration (CLI, bench, tests)
    /// through `parse_and_elaborate_seeded`.
    pub fn seed_schemas(&self, ctx: &mut ElabContext) {
        if self.is_empty() {
            return;
        }
        // The @device/@port schemas belong to the plugin *system*, not to
        // any single plugin — two device plugins must not collide on them.
        let req = |name: &str, ty: &str| AttrField {
            name: name.into(),
            ty: ty.into(),
            required: true,
            default: None,
        };
        let opt = |name: &str, ty: &str| AttrField {
            name: name.into(),
            ty: ty.into(),
            required: false,
            default: None,
        };
        ctx.schemas.register_declared("device", vec![req("plugin", "String"), req("type", "String")]);
        ctx.schemas.register_declared("port", vec![req("name", "String"), opt("kind", "String")]);
        for (name, (_owner, fields)) in &self.contributions.schemas {
            ctx.schemas.register_declared(name, fields.clone());
        }
    }
}

/// The codegen seam (Plugin plan D4): `CircuitCompiler` hands
/// `@device`-annotated instances here; the registered factory constructs
/// the solver `Device`.
impl DeviceProvider for PluginHost {
    fn build(&self, spec: PluginDeviceSpec) -> Result<Box<dyn Device>, String> {
        let (owner, factory) = self
            .contributions
            .devices
            .get(&spec.type_id)
            .ok_or_else(|| PluginError::DeviceNotRegistered(spec.type_id.clone()).to_string())?;
        if *owner != spec.plugin {
            return Err(format!(
                "device `{}` is registered by plugin `{owner}`, but @device names plugin `{}`",
                spec.type_id, spec.plugin
            ));
        }
        factory.instantiate(&spec)
    }
}
