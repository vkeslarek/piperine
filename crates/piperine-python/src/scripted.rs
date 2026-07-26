//! The scripted-plugin bridge (plugin-interface v2, PLG-06/10/11): loads a
//! `python = "…"` plugin by exec-ing its entry in the embedded interpreter
//! and reading back the facade's decorator registration table; script and
//! hook dispatch re-exec the entry (a fresh module namespace + fresh
//! registry per dispatch) and call the registered functions. Parity with
//! the Rust `#[pip::…]` surface is at the API-name level (D14) — same
//! decorator names, same five phases, same `ctx` method names.

use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyModule;

use piperine_lang::{Design, Value};
use piperine_plugin::{
    Declared, DeclaredScript, DesignStaging, HookPhase, HostCtx, Manifest, Plugin, PluginResult,
    ScriptHandler, ScriptedHost, SolveResultView,
};

use crate::design::_Design;

/// The embedded-CPython scripted host: the bridge `PluginHost` consults for
/// `python = "…"` manifests (wired by the CLI and embedded hosts).
pub struct EmbeddedScriptedHost;

impl ScriptedHost for EmbeddedScriptedHost {
    fn load_scripted(
        &self,
        plugin_root: &Path,
        manifest: &Manifest,
    ) -> Result<Box<dyn Plugin>, String> {
        let Some(entry_rel) = &manifest.python else {
            return Err(format!("plugin `{}`: scripted load without a `python` entry", manifest.name));
        };
        let entry = plugin_root.join(entry_rel);
        let table = RegistrationTable::read(&entry)?;
        Ok(Box::new(ScriptedPlugin {
            manifest: manifest.clone(),
            entry,
            scripts: table.scripts,
            hooks: table.hooks,
        }))
    }
}

/// The decorator declarations read back from one plugin load.
struct RegistrationTable {
    scripts: Vec<String>,
    hooks: HashSet<HookPhase>,
}

impl RegistrationTable {
    /// Exec the plugin entry (fresh module namespace, fresh facade
    /// registry) and read back `piperine._take_registry()`.
    fn read(entry: &Path) -> Result<Self, String> {
        let _facade = crate::embed::facade_lock();
        Python::with_gil(|py| -> PyResult<Self> {
            let facade = crate::embed::register_modules(py)?;
            exec_entry(py, entry)?;
            let raw: HashMap<String, Vec<String>> =
                facade.getattr("_take_registry")?.call0()?.extract()?;
            let mut hooks = HashSet::new();
            for phase in raw.get("hooks").cloned().unwrap_or_default() {
                let Some(phase) = HookPhase::from_name(&phase) else {
                    return Err(PyValueError::new_err(format!(
                        "plugin declared unknown hook phase `{phase}`"
                    )));
                };
                hooks.insert(phase);
            }
            Ok(Self { scripts: raw.get("scripts").cloned().unwrap_or_default(), hooks })
        })
        .map_err(|e| e.to_string())
    }
}

/// Exec the plugin entry in its own module namespace (the facade's
/// decorator registry collects its declarations).
fn exec_entry(py: Python<'_>, entry: &Path) -> PyResult<()> {
    let src = std::fs::read_to_string(entry)
        .map_err(|e| PyValueError::new_err(format!("failed to read `{}`: {e}", entry.display())))?;
    let src = CString::new(src)
        .map_err(|_| PyValueError::new_err("plugin entry contains nul bytes"))?;
    let file = CString::new(entry.display().to_string())
        .map_err(|_| PyValueError::new_err("plugin entry path contains nul bytes"))?;
    PyModule::from_code(py, &src, &file, c"piperine_scripted_plugin")?;
    Ok(())
}

/// A loaded scripted plugin: its declarations were read back at load;
/// dispatch re-execs the entry and calls the registered functions.
struct ScriptedPlugin {
    manifest: Manifest,
    entry: PathBuf,
    scripts: Vec<String>,
    hooks: HashSet<HookPhase>,
}

impl ScriptedPlugin {
    /// Fire the Python functions registered for `phase` (no-op when the
    /// plugin declared none), each with the phase's payload.
    fn fire_hook(
        &self,
        phase: HookPhase,
        cx: &HostCtx,
        design: Option<&Design>,
        source: Option<&str>,
        result: Option<&SolveResultView>,
    ) -> PluginResult<()> {
        if !self.hooks.contains(&phase) {
            return Ok(());
        }
        let _facade = crate::embed::facade_lock();
        Python::with_gil(|py| -> PyResult<()> {
            let facade = crate::embed::register_modules(py)?;
            exec_entry(py, &self.entry)?;
            let fns: Vec<Py<PyAny>> =
                facade.getattr("_registered_hooks")?.call1((phase.as_str(),))?.extract()?;
            let ctx = Py::new(py, _Ctx { host: cx.clone(), design: design.map(|d| Rc::new(d.clone())) })?;
            for f in fns {
                match phase {
                    HookPhase::AfterParse => {
                        f.call1(py, (ctx.clone_ref(py), source.unwrap_or("")))?;
                    }
                    HookPhase::TransformDesign => {
                        let Some(design) = design else {
                            return Err(PyRuntimeError::new_err(
                                "transform_design fired without a design",
                            ));
                        };
                        let staging = Py::new(
                            py,
                            _Staging {
                                design: Rc::new(design.clone()),
                                plugin: self.manifest.name.clone(),
                            },
                        )?;
                        f.call1(py, (ctx.clone_ref(py), staging))?;
                    }
                    HookPhase::AfterSolve => {
                        let result = result.ok_or_else(|| {
                            PyRuntimeError::new_err("after_solve fired without a result")
                        })?;
                        let kwargs = pyo3::types::PyDict::new(py);
                        kwargs.set_item("analysis", result.analysis.clone())?;
                        kwargs.set_item("node_voltages", result.node_voltages.clone())?;
                        let ns = py
                            .import("types")?
                            .getattr("SimpleNamespace")?
                            .call((), Some(&kwargs))?;
                        f.call1(py, (ctx.clone_ref(py), ns))?;
                    }
                    HookPhase::AfterElaborate | HookPhase::BeforeLower => {
                        f.call1(py, (ctx.clone_ref(py),))?;
                    }
                }
            }
            Ok(())
        })
        .map_err(|e: PyErr| piperine_plugin::PluginError::Other {
            plugin: self.manifest.name.clone(),
            message: e.to_string(),
        })
    }
}

impl Plugin for ScriptedPlugin {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn collect(&self) -> Declared {
        let mut declared = Declared::default();
        for name in &self.scripts {
            declared.scripts.push(DeclaredScript::new(
                name,
                Box::new(PythonScript { entry: self.entry.clone(), name: name.clone() }),
            ));
        }
        declared
    }

    fn after_parse(&self, cx: &mut HostCtx, source: &str) -> PluginResult<()> {
        self.fire_hook(HookPhase::AfterParse, cx, None, Some(source), None)
    }

    fn after_elaborate(&self, cx: &mut HostCtx, design: &Design) -> PluginResult<()> {
        self.fire_hook(HookPhase::AfterElaborate, cx, Some(design), None, None)
    }

    fn transform_design(&self, cx: &mut HostCtx, staging: &DesignStaging<'_>) -> PluginResult<()> {
        self.fire_hook(HookPhase::TransformDesign, cx, Some(staging.design()), None, None)
    }

    fn before_lower(&self, cx: &mut HostCtx, design: &Design) -> PluginResult<()> {
        self.fire_hook(HookPhase::BeforeLower, cx, Some(design), None, None)
    }

    fn after_solve(&self, cx: &mut HostCtx, result: &SolveResultView) -> PluginResult<()> {
        self.fire_hook(HookPhase::AfterSolve, cx, None, None, Some(result))
    }
}

/// One `@pip.script("name")` handler: re-execs the entry and calls the
/// registered function with the CLI args and a capability ctx.
struct PythonScript {
    entry: PathBuf,
    name: String,
}

impl ScriptHandler for PythonScript {
    fn invoke(&self, args: &[String], cx: &mut HostCtx) -> Result<i32, String> {
        let _facade = crate::embed::facade_lock();
        Python::with_gil(|py| -> PyResult<i32> {
            let facade = crate::embed::register_modules(py)?;
            exec_entry(py, &self.entry)?;
            let f = facade.getattr("_registered_script")?.call1((self.name.as_str(),))?;
            if f.is_none() {
                return Err(PyRuntimeError::new_err(format!(
                    "script `{}` is not declared by the plugin entry",
                    self.name
                )));
            }
            let ctx = Py::new(py, _Ctx { host: cx.clone(), design: None })?;
            let code: i32 = f.call1((args.to_vec(), ctx))?.extract()?;
            Ok(code)
        })
        .map_err(|e| e.to_string())
    }
}

/// `_Ctx` — the native backing of the facade's `Ctx`: the hook/script
/// context (name-parity with the Rust `piperine_plugin::Ctx`, MD-22).
/// Capability-gated filesystem access delegates to the plugin's `HostCtx`.
#[pyclass(module = "piperine", unsendable)]
pub(crate) struct _Ctx {
    host: HostCtx,
    design: Option<Rc<Design>>,
}

#[pymethods]
impl _Ctx {
    /// The elaborated design, read-only (MD-25). Available in the
    /// `after_elaborate`/`transform_design`/`before_lower` hooks; calling it
    /// from `after_parse`, `after_solve`, or a script raises — loud, never
    /// a silent empty design.
    fn design(&self) -> PyResult<_Design> {
        self.design.clone().map(_Design::from_shared).ok_or_else(|| {
            PyRuntimeError::new_err(
                "ctx.design() is only available in the after_elaborate, transform_design, and \
                 before_lower hooks",
            )
        })
    }

    /// The project root (where `Piperine.toml` lives). Always available.
    fn project_root(&self) -> String {
        self.host.project_root().display().to_string()
    }

    /// Route a message to the host logger. Always available.
    fn log(&self, message: &str) {
        self.host.log(message);
    }

    /// Read a project file — requires a matching `"read <glob>"` filesystem
    /// permission (P0002 on denial).
    fn fs_read(&self, path: &str) -> PyResult<String> {
        self.host.fs_read(path).map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Write a project file — requires a matching `"write <glob>"`
    /// filesystem permission (P0002 on denial).
    fn fs_write(&self, path: &str, contents: &str) -> PyResult<()> {
        self.host.fs_write(path, contents).map_err(|e| PyValueError::new_err(e.to_string()))
    }
}

/// `_Staging` — the native backing of the facade's `Staging`: the mutable
/// surface of a `transform_design` hook (name-parity with the Rust
/// `piperine_plugin::DesignStaging`, MD-22). Mutations go through the
/// design's staging layer and are consumed by the next pure re-elaboration.
#[pyclass(module = "piperine", unsendable)]
pub(crate) struct _Staging {
    design: Rc<Design>,
    plugin: String,
}

#[pymethods]
impl _Staging {
    /// The design being transformed — the full POM reflection surface,
    /// read-only.
    fn design(&self) -> _Design {
        _Design::from_shared(self.design.clone())
    }

    /// Stage a parameter override on `instance` (empty label = the module's
    /// own params) — same semantics as a host `set` write.
    fn set_param(&self, instance: &str, param: &str, value: f64) {
        let staging = DesignStaging::new(&self.design, &self.plugin);
        staging.set_param(instance, param, Value::Real(value));
    }

    /// Stage an instance injection into `parent`. An undeclared module or a
    /// conflicting label raises (fail loud — P0005/P0008 semantics).
    fn add_instance(
        &self,
        parent: &str,
        label: &str,
        module: &str,
        ports: Vec<String>,
        params: Vec<(String, f64)>,
    ) -> PyResult<()> {
        let staging = DesignStaging::new(&self.design, &self.plugin);
        let params = params.into_iter().map(|(n, v)| (n, Value::Real(v))).collect();
        staging
            .add_instance(parent, label, module, ports, params)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Stage a net connection into `parent`.
    fn add_connection(&self, parent: &str, lhs: &str, rhs: &str) -> PyResult<()> {
        let staging = DesignStaging::new(&self.design, &self.plugin);
        staging.add_connection(parent, lhs, rhs).map_err(|e| PyValueError::new_err(e.to_string()))
    }
}
