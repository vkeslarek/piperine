//! [`PluginHost`] — the one orchestrator: discover → verify → load →
//! register → dispatch. An empty `[plugins]` section yields an inert host;
//! the zero-plugin path costs one `is_empty` check.

use std::path::{Path, PathBuf};

use piperine::SimHooks;
use piperine_codegen::device::{DeviceProvider, PluginDeviceSpec};
use piperine_lang::elab::registry::{AttrField, ElabContext};
use piperine_lang::Design;
use piperine_project::resolver::Resolver;
use piperine_project::PiperineToml;
use piperine_solver::abi::Element;

use crate::backend::native::{self, NativePlugin};
use crate::capability::HostCtx;
use crate::contributions::{Contributions, Registrar};
use crate::error::{PluginError, PluginResult};
use crate::manifest::{Abi, Manifest};
use crate::trust::{artifact_hash, ensure_trusted, TrustMode};
use crate::view::{DesignStaging, SolveResultView};
use crate::Plugin;

/// One loaded plugin: its manifest plus the (backend-owning) instance.
struct LoadedPlugin {
    manifest: Manifest,
    instance: PluginInstance,
    /// The plugin's published `extern.phdl` stub (declared-language-surface
    /// T24, DLS-22 groundwork), parsed once at load time — `None` for
    /// `from_plugins`-loaded (in-process/test) plugins, which have no
    /// filesystem location a stub could live at, and for `load_for_project`
    /// plugins that simply don't publish one (T24 imposes no requirement
    /// yet; T25 is where "no stub" becomes load-time-fatal for a plugin
    /// that also contributes a schema).
    extern_stub: Option<Vec<piperine_lang::parse::ast::Item>>,
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
            // No filesystem location exists for an in-process plugin, so
            // there is nowhere an `extern.phdl` stub could live — always
            // `None` (declared-language-surface T24).
            host.register_one(&manifest.name.clone(), PluginInstance::InProcess(plugin), manifest, None)?;
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
        host.project_root = root.to_path_buf();
        // Deterministic load order (SPEC Part VI §8.1).
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
                Abi::Wasm => {
                    PluginInstance::InProcess(crate::backend::wasm::load(&manifest, &artifact)?)
                }
                Abi::Process => {
                    PluginInstance::InProcess(crate::backend::process::load(&manifest, &artifact)?)
                }
            };
            let extern_stub = Self::load_extern_stub(&manifest.name, plugin_root)?;
            host.register_one(&manifest.name.clone(), instance, manifest, extern_stub)?;
        }
        host.sort();
        Ok(host)
    }

    /// Parse `extern.phdl` alongside a loaded plugin's manifest, if it
    /// publishes one (declared-language-surface T24, DLS-22 groundwork).
    /// `Ok(None)` when no stub file exists — T24 imposes no requirement
    /// that one does; a malformed stub (a real authoring bug, not a
    /// "plugin didn't publish one" case) fails loud naming the plugin.
    fn load_extern_stub(
        plugin_name: &str,
        plugin_root: &Path,
    ) -> PluginResult<Option<Vec<piperine_lang::parse::ast::Item>>> {
        let stub_path = plugin_root.join("extern.phdl");
        let Ok(text) = std::fs::read_to_string(&stub_path) else {
            return Ok(None);
        };
        let source = piperine_lang::parse::parse_str(&text).map_err(|e| PluginError::Other {
            plugin: plugin_name.to_string(),
            message: format!("malformed extern stub `{}`: {e}", stub_path.display()),
        })?;
        Ok(Some(source.items))
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
        extern_stub: Option<Vec<piperine_lang::parse::ast::Item>>,
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
        self.plugins.push(LoadedPlugin { manifest, instance, extern_stub });
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

    /// Hook 1 — fired by whoever drives parsing (CLI), on the raw source.
    pub fn fire_after_parse(&self, source: &str) -> Result<(), String> {
        self.fire("after_parse", |p, cx| p.after_parse(cx, source))
    }

    /// Hook 2 — fired once the design elaborates. Native/in-process
    /// plugins see the real `&Design`; nothing is snapshotted for them.
    pub fn fire_after_elaborate(&self, design: &Design) -> Result<(), String> {
        if self.is_empty() {
            return Ok(());
        }
        self.fire("after_elaborate", |p, cx| p.after_elaborate(cx, design))
    }

    /// The plugin system's own `piperine plugin list` view: name, abi,
    /// and contribution counts.
    pub fn describe(&self) -> Vec<String> {
        self.plugins
            .iter()
            .map(|l| {
                let name = &l.manifest.name;
                let devices = self.contributions.devices.values().filter(|(o, _)| o == name).count();
                let schemas = self.contributions.schemas.values().filter(|(o, _)| o == name).count();
                let scripts: Vec<&str> = self
                    .contributions
                    .scripts
                    .iter()
                    .filter(|(_, (o, _))| o == name)
                    .map(|(n, _)| n.as_str())
                    .collect();
                format!(
                    "{name} ({}): {devices} device(s), {schemas} schema(s), scripts: [{}]",
                    l.manifest.abi.as_str(),
                    scripts.join(", ")
                )
            })
            .collect()
    }

    /// Run a plugin-contributed CLI script (SPEC Part VI §10). `None` when
    /// no loaded plugin registered `name`.
    pub fn run_script(&self, name: &str, args: &[String]) -> Option<Result<i32, PluginError>> {
        let (owner, handler) = self.contributions.scripts.get(name)?;
        let loaded = self.plugins.iter().find(|l| &l.manifest.name == owner)?;
        let mut cx = self.ctx_for(loaded);
        Some(handler.invoke(args, &mut cx).map_err(|e| PluginError::HookFailed {
            hook: "script",
            plugin: owner.clone(),
            message: e,
        }))
    }

    /// Seed the elaboration registries (Plugin plan D2): the plugin
    /// system's own `@device`/`@port` schemas, plus every plugin-declared
    /// schema. Called by whoever drives elaboration (CLI, hosts, tests)
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
        // Each loaded plugin's own published `extern.phdl` stub (T24,
        // DLS-22 groundwork) — auto-imported, no explicit `use` required
        // (mirrors `headers/spice/`'s availability). Ctrl+click on a
        // stub-declared `@name(...)` resolves to the stub's own
        // `decl_span`, exactly like any other `extern attribute`.
        for loaded in &self.plugins {
            if let Some(items) = &loaded.extern_stub {
                Self::register_attribute_items(ctx, items);
            }
        }
        for (name, (_owner, fields)) in &self.contributions.schemas {
            ctx.schemas.register_declared(name, fields.clone(), None);
        }
    }

    /// Register every `extern attribute` item's schema into `ctx.schemas` —
    /// shared by `@device`/`@port`'s own header and each plugin's
    /// `extern.phdl` stub (T23/T24). Non-attribute items are ignored (a
    /// stub could in principle carry other `extern` kinds; only attribute
    /// schemas are this feature's concern here).
    fn register_attribute_items(ctx: &mut ElabContext, items: &[piperine_lang::parse::ast::Item]) {
        for item in items {
            if let piperine_lang::parse::ast::Item::ExternDecl(
                piperine_lang::parse::ast::ExternDecl::Attribute { span, name, fields },
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
                ctx.schemas.register_declared(name, attr_fields, *span);
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
        Ok(())
    }

    fn before_lower(&self, design: &Design) -> Result<(), String> {
        if self.is_empty() {
            return Ok(());
        }
        self.fire("before_lower", |p, cx| p.before_lower(cx, design))
    }

    fn after_solve(&self, analysis: &str, node_voltages: &[(String, f64)]) -> Result<(), String> {
        if self.is_empty() {
            return Ok(());
        }
        let result = SolveResultView {
            analysis: analysis.to_string(),
            node_voltages: node_voltages.to_vec(),
        };
        self.fire("after_solve", |p, cx| p.after_solve(cx, &result))
    }

}

/// The codegen seam (Plugin plan D4): `CircuitCompiler` hands
/// `@device`-annotated instances here; the registered factory constructs
/// the solver `Element`.
impl DeviceProvider for PluginHost {
    fn build(&self, spec: PluginDeviceSpec) -> Result<Box<dyn Element>, String> {
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
