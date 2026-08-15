//! [`PluginHost`] — the one orchestrator: discover → verify → load →
//! register → dispatch. An empty `[plugins]` section yields an inert host;
//! the zero-plugin path costs one `is_empty` check.
//!
//! **T53 (VAMS-21)**: [`PluginHost::register_system_task`]/
//! [`Self::resolve_system_task`]/[`Self::call_system_task`] are the native
//! ABI ch. 11's `vpi_register_systf`/`vpi_register_analog_systf` (§11.2.1)
//! route through — Piperine never hosts a literal VPI C shim (spec's Out of
//! Scope table); a plugin registers a [`SystemTaskHandler`] under a name and
//! [`piperine_lang::eval::tasks::VpiTaskKind`], and calls to that `$name`
//! dispatch to it directly, in-process. Name bookkeeping (collision with a
//! ch. 9 builtin, duplicate registration, unknown-name lookup) is
//! [`piperine_lang::eval::tasks::VpiTaskRegistry`]'s own typed E4010–E4012
//! errors, reused here rather than duplicated.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use piperine::SimHooks;
use piperine_codegen::device::{DeviceProvider, PluginDeviceSpec};
use piperine_lang::elab::registry::ElabContext;
use piperine_lang::eval::tasks::{VpiTaskKind, VpiTaskRegistry};
use piperine_lang::value::Value;
use piperine_lang::Design;
use piperine_project::lockfile::PiperineLock;
use piperine_project::release::{GitHubClient, PluginCache, ReleaseError, ReleaseRef};
use piperine_project::resolver::Resolver;
use piperine_project::PiperineToml;
use piperine_solver::abi::{DigitalNet, Element};
use piperine_solver::{Channel, DelayTable, FileError, FileTable, McdTable, PathDelay, SimOutput};

use crate::backend::native::{self, NativePlugin};
use crate::capability::HostCtx;
use crate::contributions::Contributions;
use crate::error::{PluginError, PluginResult};
use crate::manifest::Manifest;
use crate::registry::{CallbackId, CallbackRegistry, HookCall, HookPhase, SimEvent};
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

/// A native Rust handler for one VPI-registered system task/function
/// (LRM §11.2.1's `vpi_register_systf`/`vpi_register_analog_systf`
/// mechanism), dispatched in-process through Piperine's own plugin ABI —
/// never a `dlopen`'d IEEE-VPI C shim (`plugin-abi-custom` decision).
pub trait SystemTaskHandler: Send + Sync {
    /// Runs the task/function with its call-site arguments. `Ok(None)` for
    /// a `SysTask`/`AnalogSysTask` (no return value); `Ok(Some(_))` for a
    /// `SysFunc`/`SysRealFunc`/`AnalogSysFunc`.
    fn call(&self, args: &[Value]) -> Result<Option<Value>, String>;
}

/// The plugin host: loaded plugins in deterministic (alphabetical) order
/// plus their merged contributions.
pub struct PluginHost {
    plugins: Vec<LoadedPlugin>,
    contributions: Contributions,
    /// Where `Piperine.toml` lives — every capability-gated path resolves
    /// against this.
    project_root: PathBuf,
    /// LRM §11.2.1 VPI-associated system task/function registrations
    /// (T53, VAMS-21) — name/kind bookkeeping.
    system_tasks: VpiTaskRegistry,
    /// The native handler each registered name dispatches to.
    system_task_handlers: HashMap<String, Arc<dyn SystemTaskHandler>>,
    /// T59 (LRMC-C10-11): runtime event-grained callbacks, orthogonal to
    /// the static `HookPhase` mechanism above — `RefCell` because
    /// `register_cb`/`remove_cb` are reachable from `&self` (a plugin may
    /// register from inside a running hook, not just at setup).
    callbacks: RefCell<CallbackRegistry>,
}

impl PluginHost {
    /// An inert host — no plugins, every dispatch a no-op.
    pub fn empty() -> Self {
        Self {
            plugins: Vec::new(),
            contributions: Contributions::default(),
            project_root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            system_tasks: VpiTaskRegistry::new(),
            system_task_handlers: HashMap::new(),
            callbacks: RefCell::new(CallbackRegistry::new()),
        }
    }

    /// Registers `callback` to fire every time `event` occurs (T59,
    /// LRMC-C10-11) — the native equivalent of `vpi_register_cb()`.
    /// Returns a [`CallbackId`] usable with [`Self::remove_cb`].
    pub fn register_cb(&self, event: SimEvent, callback: impl FnMut() + 'static) -> CallbackId {
        self.callbacks.borrow_mut().register_cb(event, callback)
    }

    /// Deregisters `id` — the native equivalent of `vpi_remove_cb()`.
    /// Returns `true` if it was registered and is now removed.
    pub fn remove_cb(&self, id: CallbackId) -> bool {
        self.callbacks.borrow_mut().remove_cb(id)
    }

    /// How many runtime callbacks are currently registered (any event) —
    /// test/introspection only.
    pub fn callback_count(&self) -> usize {
        self.callbacks.borrow().len()
    }

    /// `vpi_get_delays()`'s native equivalent (T61, LRMC-C10-12 surface
    /// half): reads `net`'s recorded [`PathDelay`] from `table` (T60's
    /// model — `piperine_solver::abi::DigitalState::delay_table`, e.g.
    /// `circuit.digital_state.delay_table`). `PluginHost` holds no live
    /// circuit state itself (MD-18: a compiled circuit belongs to the
    /// host/`Session` driving it, never re-elaborated or owned mid-run by
    /// the plugin layer) — this is a thin, named routine over the caller-
    /// supplied table, the same "native equivalent, not a literal
    /// `vpiHandle`" shape every other `vpi_*` entry in this crate takes
    /// (`vpi.rs`'s own module doc).
    pub fn get_delays(&self, table: &DelayTable, net: DigitalNet) -> PathDelay {
        table.get(net)
    }

    /// `vpi_put_delays()`'s native equivalent — writes `net`'s
    /// [`PathDelay`] into `table`. Takes effect for any future scheduled
    /// transition that consults the table (a gate/model whose `comb_phase`
    /// reads `delay_table.get(...)` per evaluation, e.g. this crate's own
    /// `tests/delay_processing.rs`); an event already queued before the
    /// write keeps its originally-computed time (MD-18's "restamp on the
    /// compiled circuit, never re-elaborate" extends naturally to "never
    /// retroactively rewrite an already-scheduled event").
    pub fn put_delays(&self, table: &mut DelayTable, net: DigitalNet, delay: PathDelay) {
        table.set(net, delay);
    }

    /// `vpi_put_value()`'s native equivalent (T62, LRMC-C10-13): drives
    /// `label`.`param` into `session`'s running solve, symmetric with the
    /// read path (`SimHooks::after_solve`'s `SolveResultView`). A thin
    /// wrapper over `piperine::Session::set` — which already is MD-18-
    /// compliant (restamps the compiled circuit, never re-JITs) and
    /// already fails loud on a structural write or an out-of-bounds
    /// value; this method exists to give the mechanism a named,
    /// `vpi_*`-shaped entry point in this crate's own surface, the same
    /// reason `get_delays`/`put_delays` exist rather than callers reaching
    /// into `piperine-solver` directly.
    pub fn put_value(
        &self,
        session: &mut piperine::Session,
        label: &str,
        param: &str,
        value: f64,
    ) -> PluginResult<()> {
        session
            .set(label, param, value)
            .map_err(|e| PluginError::Other { plugin: "<vpi_put_value>".into(), message: e.to_string() })
    }

    /// `vpi_printf()`'s native equivalent (T63, LRMC-C10-14): writes
    /// `text` to `sink`'s standard-output channel — the *same* sink
    /// `Self::mcd_printf`'s bit-0 (`McdTable::STDOUT`) case routes
    /// through, so `vpi_printf` and an `mcd_printf` targeting only stdout
    /// are indistinguishable in destination, matching 1364's own "`vpi_
    /// printf` behaves as though it were `vpi_mcd_printf` to the standard
    /// output channel" framing.
    pub fn vpi_printf(&self, sink: &dyn SimOutput, text: &str) {
        sink.write(Channel::Stdout, text);
    }

    /// `vpi_mcd_open()`'s native equivalent: opens `path` and allocates it
    /// a fresh channel bit in `mcd`, over the shared `files` table (T4's
    /// `FileTable` — the same descriptor space `$fopen`-family tasks use,
    /// per this task's own "multi-channel descriptors as a channel mask
    /// over the same file table" framing).
    pub fn mcd_open(&self, mcd: &mut McdTable, files: &mut FileTable, path: &str) -> Result<u32, FileError> {
        mcd.open(files, path)
    }

    /// `vpi_mcd_close()`'s native equivalent: closes every file-backed
    /// channel named by a set bit in `channel_mask` (the stdout bit is
    /// silently skipped — it can never be closed).
    pub fn mcd_close(&self, mcd: &mut McdTable, files: &mut FileTable, channel_mask: u32) -> Result<(), FileError> {
        mcd.close(files, channel_mask)
    }

    /// `vpi_mcd_printf()`'s native equivalent: writes `text` to every
    /// channel named by a set bit in `channel_mask` — stdout (bit 0)
    /// through `sink`, every other bit through its bound `FileTable`
    /// entry.
    pub fn mcd_printf(&self, mcd: &McdTable, files: &mut FileTable, sink: &dyn SimOutput, channel_mask: u32, text: &str) {
        mcd.printf(files, sink, channel_mask, text);
    }

    /// `vpi_mcd_flush()`-equivalent (1364 also exposes this as
    /// `$fflush(mcd)` over a channel mask): flushes every channel named by
    /// a set bit in `channel_mask`.
    pub fn mcd_flush(&self, mcd: &McdTable, files: &mut FileTable, sink: &dyn SimOutput, channel_mask: u32) {
        mcd.flush(files, sink, channel_mask);
    }

    /// `vpi_mcd_name()`'s native equivalent: the path bound to
    /// `channel_mask`'s lowest set file-backed bit. `None` for the stdout
    /// bit or an unallocated/closed bit.
    pub fn mcd_name(&self, mcd: &McdTable, files: &FileTable, channel_mask: u32) -> Option<String> {
        let (_, id) = mcd.name(channel_mask)?;
        files.path(id).map(str::to_string)
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

    /// Discover, verify, and load every plugin of the project at `root`
    /// (SPEC Part VI §5) — its `[plugins]` entries plus every
    /// `[dependencies]` entry carrying a `piperine-plugin.toml` (D9: a
    /// plugin is a contributing dependency, so `piperine add <git>` is the
    /// whole install): resolve sources, parse manifests (P0006),
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
        // A project with neither section has nothing to load; a project with
        // only `[dependencies]` may still carry contributing dependencies
        // (D9 — `piperine add <git>` is the whole install), so the resolver
        // decides, not the section count.
        if toml.plugins.is_empty() && toml.dependencies.is_empty() {
            return Ok(Self::empty());
        }

        let mut resolver = Resolver::new(root, false);
        let resolved = resolver.resolve_plugins(&toml).map_err(|e| PluginError::Other {
            plugin: "<resolver>".into(),
            message: e.to_string(),
        })?;
        if resolved.is_empty() {
            return Ok(Self::empty());
        }

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
        self.callbacks.borrow_mut().fire(&SimEvent::AfterParse);
        self.fire("after_parse", |p, cx| p.after_parse(cx, source))?;
        self.fire_declared(HookPhase::AfterParse, Some(source), None, None)
    }

    /// Hook 2 — fired once the design elaborates. Native/in-process
    /// plugins see the real `&Design`; nothing is snapshotted for them.
    pub fn fire_after_elaborate(&self, design: &Design) -> Result<(), String> {
        self.callbacks.borrow_mut().fire(&SimEvent::AfterElaborate);
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
    /// **T35 (VAMS-12/13)**: `@device`/`@port`'s shape used to come from
    /// `headers/device_port.phdl`'s `extern attribute` declarations
    /// (declared-language-surface T23, DLS-21) — that mechanism has no
    /// VAMS Annex A production and was deleted with the PHDL parser
    /// (T21, MD-45). Schema authority stays here (MD-21/D2: two device
    /// plugins never collide, still gated on `!self.is_empty()` below),
    /// delegating the actual registration to
    /// [`piperine_lang::seed_device_port_schemas`] — the one place the
    /// `device`/`port` names are spelled, so a plugin-driven elaboration
    /// and a direct `parse_and_elaborate_seeded(.., seed_device_port_schemas)`
    /// call (used by this crate's own tests and `piperine-api`'s) always
    /// agree.
    /// Registers a native handler for a VPI-associated system task/function
    /// (LRM §11.2.1) under `name` and `kind` — the native-ABI equivalent of
    /// a `vpi_register_systf`/`vpi_register_analog_systf` call. Fails loud
    /// (E4010/E4011, via [`VpiTaskRegistry::register`]) on a name colliding
    /// with a ch. 9 builtin or a duplicate registration; never silently
    /// overwrites (C6).
    pub fn register_system_task(
        &mut self,
        name: &str,
        kind: VpiTaskKind,
        handler: impl SystemTaskHandler + 'static,
    ) -> PluginResult<()> {
        self.system_tasks
            .register(name, kind)
            .map_err(|e| PluginError::Other { plugin: "<system-task>".into(), message: e.to_string() })?;
        self.system_task_handlers.insert(name.to_string(), Arc::new(handler));
        Ok(())
    }

    /// Looks up a registered VPI system task/function's [`VpiTaskKind`].
    /// Fails loud (E4012, via [`VpiTaskRegistry::resolve`]) with near-match
    /// suggestions when `name` has no registration — never a silent
    /// unknown-identifier pass-through (C6).
    pub fn resolve_system_task(&self, name: &str) -> PluginResult<VpiTaskKind> {
        self.system_tasks
            .resolve(name)
            .map_err(|e| PluginError::Other { plugin: "<system-task>".into(), message: e.to_string() })
    }

    /// Dispatches a call to `name`'s registered native handler. `None` when
    /// nothing registered `name` — the same not-silently-absent shape as
    /// [`Self::run_script`] (a caller that needs "unknown" to be an error
    /// should check [`Self::resolve_system_task`] first).
    pub fn call_system_task(
        &self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Option<Value>, PluginError>> {
        let handler = self.system_task_handlers.get(name)?;
        Some(handler.call(args).map_err(|message| PluginError::HookFailed {
            hook: "system_task",
            plugin: name.to_string(),
            message,
        }))
    }

    pub fn seed_schemas(&self, ctx: &mut ElabContext) {
        if self.is_empty() {
            return;
        }
        piperine_lang::seed_device_port_schemas(ctx);
    }
}

/// The simulation seam (Plugin plan Phase 3): the host API's `Session` fires
/// the build hooks from `SessionBuilder::compile` and `after_solve` from every
/// analysis, through this.
impl SimHooks for PluginHost {
    fn transform_design(&self, design: &Design) -> Result<(), String> {
        self.callbacks.borrow_mut().fire(&SimEvent::TransformDesign);
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
        self.callbacks.borrow_mut().fire(&SimEvent::BeforeLower);
        if self.is_empty() {
            return Ok(());
        }
        self.fire("before_lower", |p, cx| p.before_lower(cx, design))?;
        self.fire_declared(HookPhase::BeforeLower, None, Some(design), None)
    }

    fn after_solve(&self, analysis: &str, node_voltages: &[(String, f64)]) -> Result<(), String> {
        self.callbacks.borrow_mut().fire(&SimEvent::AfterSolve(analysis.to_string()));
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

#[cfg(test)]
mod tests {
    use super::*;

    struct Echo;
    impl SystemTaskHandler for Echo {
        fn call(&self, args: &[Value]) -> Result<Option<Value>, String> {
            Ok(args.first().cloned())
        }
    }

    struct AlwaysFails;
    impl SystemTaskHandler for AlwaysFails {
        fn call(&self, _args: &[Value]) -> Result<Option<Value>, String> {
            Err("boom".to_string())
        }
    }

    #[test]
    fn registers_resolves_and_dispatches() {
        let mut host = PluginHost::empty();
        host.register_system_task("$my_func", VpiTaskKind::SysFunc, Echo).unwrap();
        assert_eq!(host.resolve_system_task("$my_func").unwrap(), VpiTaskKind::SysFunc);
        let out = host.call_system_task("$my_func", &[Value::Int(42)]).unwrap().unwrap();
        assert_eq!(out, Some(Value::Int(42)));
    }

    #[test]
    fn analog_registration_kinds_round_trip() {
        let mut host = PluginHost::empty();
        host.register_system_task("$my_analog_func", VpiTaskKind::AnalogSysFunc, Echo).unwrap();
        assert_eq!(host.resolve_system_task("$my_analog_func").unwrap(), VpiTaskKind::AnalogSysFunc);
    }

    #[test]
    fn unregistered_call_is_none_not_a_panic() {
        let host = PluginHost::empty();
        assert!(host.call_system_task("$nope", &[]).is_none());
    }

    #[test]
    fn unregistered_resolve_fails_loud() {
        let host = PluginHost::empty();
        assert!(host.resolve_system_task("$nope").is_err());
    }

    #[test]
    fn cannot_shadow_a_ch9_builtin() {
        let mut host = PluginHost::empty();
        let err = host.register_system_task("$display", VpiTaskKind::SysTask, Echo).unwrap_err();
        assert!(err.to_string().contains("$display"));
    }

    #[test]
    fn duplicate_registration_fails_loud() {
        let mut host = PluginHost::empty();
        host.register_system_task("$my_task", VpiTaskKind::SysTask, Echo).unwrap();
        let err = host.register_system_task("$my_task", VpiTaskKind::SysFunc, Echo).unwrap_err();
        assert!(err.to_string().contains("$my_task"));
    }

    #[test]
    fn handler_failure_surfaces_as_hook_failed() {
        let mut host = PluginHost::empty();
        host.register_system_task("$boom", VpiTaskKind::SysTask, AlwaysFails).unwrap();
        let err = host.call_system_task("$boom", &[]).unwrap().unwrap_err();
        assert!(matches!(err, PluginError::HookFailed { .. }));
    }
}
