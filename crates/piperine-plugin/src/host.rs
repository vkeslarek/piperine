//! [`PluginHost`] — the one orchestrator: discover → verify → load →
//! register → dispatch. An empty `[plugins]` section yields an inert host;
//! the zero-plugin path costs one `is_empty` check.

use std::path::{Path, PathBuf};

use piperine::SimHooks;
use piperine_codegen::device::{DeviceProvider, PluginDeviceSpec};
use piperine_lang::elab::registry::{AttrField, ElabContext};
use piperine_lang::Design;
use piperine_project::lockfile::PiperineLock;
use piperine_project::release::{GitHubClient, PluginCache, ReleaseError, ReleaseRef};
use piperine_project::resolver::Resolver;
use piperine_project::PiperineToml;
use piperine_solver::abi::Element;

use crate::backend::native::{self, NativePlugin};
use crate::capability::HostCtx;
use crate::contributions::Contributions;
use crate::error::{PluginError, PluginResult};
use crate::manifest::Manifest;
use crate::registry::{HookCall, HookPhase};
use crate::trust::{artifact_hash, ensure_release_trusted, ensure_trusted, TrustMode};
use crate::view::{DesignStaging, SolveResultView};
use crate::Plugin;

/// Map a release-fetch failure onto the plugin error catalog — the
/// unsupported-triple case keeps its own typed error (P0012, PLG-19).
fn release_error(plugin: &str, e: ReleaseError) -> PluginError {
    match e {
        ReleaseError::NoAssetForTriple { triple, release } => {
            PluginError::NoAssetForTriple { plugin: plugin.to_string(), triple, release }
        }
        other => PluginError::Other { plugin: plugin.to_string(), message: other.to_string() },
    }
}

/// One loaded plugin: its manifest plus the (backend-owning) instance.
struct LoadedPlugin {
    manifest: Manifest,
    instance: PluginInstance,
}

impl LoadedPlugin {
    fn plugin(&self) -> &dyn Plugin {
        match &self.instance {
            PluginInstance::Native(n) => n.plugin.as_ref(),
            PluginInstance::InProcess(p) => p.as_ref(),
        }
    }
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
    /// Where `Piperine.toml` lives — every capability-gated path resolves
    /// against this.
    project_root: PathBuf,
}

impl PluginHost {
    /// An inert host — no plugins, every dispatch a no-op.
    pub fn empty() -> Self {
        Self {
            plugins: Vec::new(),
            contributions: Contributions::default(),
            project_root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// Loaded plugin names, alphabetical.
    pub fn plugin_names(&self) -> Vec<&str> {
        self.plugins.iter().map(|p| p.manifest.name.as_str()).collect()
    }

    /// Rebase capability-gated paths onto `root` (tests, embedded hosts).
    pub fn with_project_root(mut self, root: &Path) -> Self {
        self.project_root = root.to_path_buf();
        self
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
    /// hash artifacts, run TOFU (P0001/P0007), dlopen, register (P0003). A
    /// scripted (`python = "…"`) plugin needs an embedded-Python host —
    /// [`Self::load_for_project_scripted`]; without one it is a loud error,
    /// never a silent skip.
    pub fn load_for_project(root: &Path, trust: TrustMode) -> PluginResult<Self> {
        Self::load(root, trust, None)
    }

    /// [`Self::load_for_project`] with a scripted host for `python = "…"`
    /// plugins (PLG-06/10 — the embedded-Python bridge execs the entry and
    /// reads back its decorator declarations).
    pub fn load_for_project_scripted(
        root: &Path,
        trust: TrustMode,
        scripted: &dyn crate::ScriptedHost,
    ) -> PluginResult<Self> {
        Self::load(root, trust, Some(scripted))
    }

    fn load(root: &Path, trust: TrustMode, scripted: Option<&dyn crate::ScriptedHost>) -> PluginResult<Self> {
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
        host.project_root = root.to_path_buf();
        // Deterministic load order (SPEC Part VI §8.1).
        let mut names: Vec<&String> = resolved.keys().collect();
        names.sort();
        for name in names {
            let plugin_root = &resolved[name];
            let manifest = Manifest::load(name, plugin_root)?;
            let source = toml
                .plugins
                .get(name)
                .map(|s| format!("{s:?}"))
                .unwrap_or_else(|| plugin_root.display().to_string());
            if let Some(device) = &manifest.device {
                let artifact = match &device.path {
                    Some(rel) => plugin_root.join(rel),
                    None => {
                        // Release distribution (plugin-interface v2,
                        // PLG-16..18): resolve the triple-matched asset,
                        // fetch + cache it, then verify/TOFU-pin
                        // `(release-url, triple, content-hash)`.
                        let coord = device.release.clone().unwrap_or_default();
                        let release = ReleaseRef::parse(&coord).map_err(|e| PluginError::Other {
                            plugin: manifest.name.clone(),
                            message: e.to_string(),
                        })?;
                        let triple = ReleaseRef::host_triple();
                        let pinned = PiperineLock::load(&root.join("Piperine.lock"))
                            .ok()
                            .flatten()
                            .and_then(|l| l.plugin_entry(&manifest.name).and_then(|e| e.content_hash.clone()));
                        let cache = PluginCache::new(PluginCache::default_dir());
                        let fetched = cache
                            .fetch(&GitHubClient, &release, &triple, pinned.as_deref())
                            .map_err(|e| release_error(&manifest.name, e))?;
                        ensure_release_trusted(
                            root,
                            &manifest,
                            &coord,
                            &fetched.content_hash,
                            &fetched.triple,
                            device.verify.as_deref(),
                            trust,
                        )?;
                        fetched.path
                    }
                };
                let hash = artifact_hash(&artifact)?;
                ensure_trusted(root, &manifest, &source, &hash, trust)?;
                let instance = PluginInstance::Native(native::load(&manifest.name, &artifact)?);
                let plugin_name = manifest.name.clone();
                host.register_one(&plugin_name, instance, manifest.clone())?;
            }
            if let Some(python) = &manifest.python {
                let Some(bridge) = scripted else {
                    return Err(PluginError::Other {
                        plugin: manifest.name.clone(),
                        message: "scripted (Python) plugin declared, but no embedded-Python host \
                                  is wired (load through PluginHost::load_for_project_scripted)"
                            .into(),
                    });
                };
                let entry = plugin_root.join(python);
                let hash = artifact_hash(&entry)?;
                ensure_trusted(root, &manifest, &source, &hash, trust)?;
                let plugin = bridge.load_scripted(plugin_root, &manifest).map_err(|e| {
                    PluginError::Other { plugin: manifest.name.clone(), message: e }
                })?;
                let plugin_name = manifest.name.clone();
                host.register_one(&plugin_name, PluginInstance::InProcess(plugin), manifest.clone())?;
            }
            // Neither key: a pure-PHDL plugin is a code library — its `pub`
            // items resolve via `use`, nothing runs.
        }
        host.sort();
        Ok(host)
    }

    fn sort(&mut self) {
        self.plugins.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
    }

    /// Merge one plugin's declared contributions (`Plugin::collect` — the
    /// `#[pip::…]` registry of the plugin's own binary). Distinct
    /// declarations colliding on a device type id or script name surface
    /// here as P0003.
    fn register_one(
        &mut self,
        name: &str,
        instance: PluginInstance,
        manifest: Manifest,
    ) -> PluginResult<()> {
        let declared = match &instance {
            PluginInstance::Native(n) => n.plugin.collect(),
            PluginInstance::InProcess(p) => p.collect(),
        };
        self.contributions.merge(name, declared)?;
        self.plugins.push(LoadedPlugin { manifest, instance });
        Ok(())
    }

    /// A capability facade for `plugin`, from its manifest permissions.
    fn ctx_for(&self, plugin: &LoadedPlugin) -> HostCtx {
        HostCtx::new(&plugin.manifest.name, &self.project_root, plugin.manifest.permissions.clone())
    }

    /// Fire one hook on every plugin, alphabetically; the first failure
    /// aborts the run as P0005 (fail loud — a failed hook is never skipped).
    fn fire(
        &self,
        hook: &'static str,
        mut f: impl FnMut(&dyn Plugin, &mut HostCtx) -> PluginResult<()>,
    ) -> Result<(), String> {
        for loaded in &self.plugins {
            let mut cx = self.ctx_for(loaded);
            f(loaded.plugin(), &mut cx).map_err(|e| {
                PluginError::HookFailed {
                    hook,
                    plugin: loaded.manifest.name.clone(),
                    message: e.to_string(),
                }
                .to_string()
            })?;
        }
        Ok(())
    }

    /// Fire the `#[pip::hook(phase)]` declarations merged into the
    /// contributions, in load order, each under its owning plugin's
    /// capability facade. Trait-method hooks (fired by [`Self::fire`]) and
    /// declared hooks coexist; both fail loud as P0005.
    fn fire_declared(
        &self,
        phase: HookPhase,
        source: Option<&str>,
        design: Option<&Design>,
        result: Option<&SolveResultView>,
    ) -> Result<(), String> {
        for entry in self.contributions.hooks.iter().filter(|h| h.phase == phase) {
            let Some(owner) = self.plugins.iter().find(|l| l.manifest.name == entry.plugin) else {
                return Err(format!(
                    "hook `{}` declared by `{}`, which is not a loaded plugin",
                    phase.as_str(),
                    entry.plugin
                ));
            };
            let cx = self.ctx_for(owner);
            let staging = design.map(|d| DesignStaging::new(d, &entry.plugin));
            let call = HookCall {
                host: &cx,
                source,
                design,
                staging: staging.as_ref(),
                result,
            };
            (entry.invoke)(&call).map_err(|e| {
                PluginError::HookFailed {
                    hook: phase.as_str(),
                    plugin: entry.plugin.clone(),
                    message: e,
                }
                .to_string()
            })?;
        }
        Ok(())
    }

    /// Hook 1 — fired by whoever drives parsing (CLI), on the raw source.
    pub fn fire_after_parse(&self, source: &str) -> Result<(), String> {
        self.fire("after_parse", |p, cx| p.after_parse(cx, source))?;
        self.fire_declared(HookPhase::AfterParse, Some(source), None, None)
    }

    /// Hook 2 — fired once the design elaborates. Native/in-process
    /// plugins see the real `&Design`; nothing is snapshotted for them.
    pub fn fire_after_elaborate(&self, design: &Design) -> Result<(), String> {
        if self.is_empty() {
            return Ok(());
        }
        self.fire("after_elaborate", |p, cx| p.after_elaborate(cx, design))?;
        self.fire_declared(HookPhase::AfterElaborate, None, Some(design), None)
    }

    /// The plugin system's own `piperine plugin list` view: name, shape,
    /// and contribution counts.
    pub fn describe(&self) -> Vec<String> {
        self.plugins
            .iter()
            .map(|l| {
                let name = &l.manifest.name;
                let devices =
                    self.contributions.devices.values().filter(|d| d.plugin == *name).count();
                let scripts: Vec<&str> = self
                    .contributions
                    .scripts
                    .iter()
                    .filter(|(_, s)| s.plugin == *name)
                    .map(|(n, _)| n.as_str())
                    .collect();
                format!(
                    "{name} ({}): {devices} device(s), scripts: [{}]",
                    l.manifest.shape().as_str(),
                    scripts.join(", ")
                )
            })
            .collect()
    }

    /// Run a plugin-contributed CLI script (SPEC Part VI §10). `None` when
    /// no loaded plugin registered `name`.
    pub fn run_script(&self, name: &str, args: &[String]) -> Option<Result<i32, PluginError>> {
        let entry = self.contributions.scripts.get(name)?;
        let loaded = self.plugins.iter().find(|l| l.manifest.name == entry.plugin)?;
        let mut cx = self.ctx_for(loaded);
        Some(entry.handler.invoke(args, &mut cx).map_err(|e| PluginError::HookFailed {
            hook: "script",
            plugin: entry.plugin.clone(),
            message: e,
        }))
    }

    /// Seed the elaboration registries (Plugin plan D2): the plugin
    /// system's own `@device`/`@port` schemas — the ONLY plugin-facing
    /// schema names (plugin-interface v2, PLG-08/09: plugins contribute
    /// no schemas; the per-plugin `extern.phdl` stub mechanism is
    /// deleted). Called by whoever drives elaboration (CLI, hosts, tests)
    /// through `parse_and_elaborate_seeded`.
    ///
    /// `@device`/`@port`'s shape (declared-language-surface T23, DLS-21)
    /// comes from `headers/device_port.phdl`'s `extern attribute`
    /// declarations, not a hand-rolled `AttrField` list — ctrl+click on
    /// either name now resolves to that header exactly like any other
    /// `extern attribute`. This header is parsed here (not embedded into
    /// every compilation unit like `piperine-lang`'s own
    /// `types.phdl`/`math.phdl`/etc.) because `@device`/`@port` are only
    /// meaningful once a plugin is loaded — unchanged: still gated on
    /// `!self.is_empty()` below.
    pub fn seed_schemas(&self, ctx: &mut ElabContext) {
        if self.is_empty() {
            return;
        }
        // The @device/@port schemas belong to the plugin *system*, not to
        // any single plugin — two device plugins must not collide on them.
        if let Ok(source) = piperine_lang::parse::parse_str(include_str!(
            "../../piperine-lang/headers/device_port.phdl"
        )) {
            Self::register_attribute_items(ctx, &source.items);
        }
    }

    /// Register every `extern attribute` item's schema into `ctx.schemas`
    /// (T23) — used for `@device`/`@port`'s own header. Non-attribute
    /// items are ignored.
    fn register_attribute_items(ctx: &mut ElabContext, items: &[piperine_lang::parse::ast::Item]) {
        for item in items {
            if let piperine_lang::parse::ast::Item::ExternDecl(
                piperine_lang::parse::ast::ExternDecl::Attribute { span, name, fields, doc },
            ) = item
            {
                let attr_fields = fields
                    .iter()
                    .map(|f| AttrField {
                        name: f.name.clone(),
                        ty: f.ty.name.clone(),
                        required: !f.ty.optional,
                        default: None,
                        decl_span: f.span,
                    })
                    .collect();
                ctx.schemas.register_declared(name, attr_fields, *span, doc.clone());
            }
        }
    }
}

/// The simulation seam (Plugin plan Phase 3): the host API's `SimSession`
/// fires the per-analysis lifecycle hooks through this.
impl SimHooks for PluginHost {
    fn transform_design(&self, design: &Design) -> Result<(), String> {
        if self.is_empty() {
            return Ok(());
        }
        // Per-plugin staging handles: each carries its writer name so a
        // collision surfaces as a typed P0008 naming both parties.
        for loaded in &self.plugins {
            let staging = DesignStaging::new(design, &loaded.manifest.name);
            let mut cx = self.ctx_for(loaded);
            loaded
                .plugin()
                .transform_design(&mut cx, &staging)
                .map_err(|e| match e {
                    conflict @ PluginError::StagingConflict { .. } => conflict.to_string(),
                    other => PluginError::HookFailed {
                        hook: "transform_design",
                        plugin: loaded.manifest.name.clone(),
                        message: other.to_string(),
                    }
                    .to_string(),
                })?;
        }
        self.fire_declared(HookPhase::TransformDesign, None, Some(design), None)
    }

    fn before_lower(&self, design: &Design) -> Result<(), String> {
        if self.is_empty() {
            return Ok(());
        }
        self.fire("before_lower", |p, cx| p.before_lower(cx, design))?;
        self.fire_declared(HookPhase::BeforeLower, None, Some(design), None)
    }

    fn after_solve(&self, analysis: &str, node_voltages: &[(String, f64)]) -> Result<(), String> {
        if self.is_empty() {
            return Ok(());
        }
        let result = SolveResultView {
            analysis: analysis.to_string(),
            node_voltages: node_voltages.to_vec(),
        };
        self.fire("after_solve", |p, cx| p.after_solve(cx, &result))?;
        self.fire_declared(HookPhase::AfterSolve, None, None, Some(&result))
    }

}

/// The codegen seam (Plugin plan D4): `CircuitCompiler` hands
/// `@device`-annotated instances here; the registered factory constructs
/// the solver `Element`.
impl DeviceProvider for PluginHost {
    fn build(&self, spec: PluginDeviceSpec) -> Result<Box<dyn Element>, String> {
        let entry = self
            .contributions
            .devices
            .get(&spec.type_id)
            .ok_or_else(|| PluginError::DeviceNotRegistered(spec.type_id.clone()).to_string())?;
        if entry.plugin != spec.plugin {
            return Err(format!(
                "device `{}` is registered by plugin `{}`, but @device names plugin `{}`",
                spec.type_id, entry.plugin, spec.plugin
            ));
        }
        entry.factory.instantiate(&spec)
    }
}
