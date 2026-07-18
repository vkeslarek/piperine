//! `_InstanceView` — the terminal sub-view returned by `result["instance.path"]`
//! (PY-13 / spec AC13). Resolves an instance path against the POM, then exposes
//! the instance's terminal quantities (terminal voltages + branch current) by
//! delegating to the parent result's `.v/.i` readouts over the connected nets.
//!
//! Uniform shape (PY-17): the sub-view's `.v/.i` calls walk the same POM
//! edges the elaborator resolved (instance port → connected top-level net).

use std::rc::Rc;

use pyo3::exceptions::{PyKeyError, PyRuntimeError};
use pyo3::prelude::*;

use piperine::{OpResult, Trace};
use piperine_lang::pom::node::Node;
use piperine_lang::Design;

use crate::results::{readout_err, _Waveform};

/// Bridge between a hierarchical/dot-notation instance path the user types and
/// the POM instance it names. Carries a shared design + the parent module name
/// (the module the user called `.op/.tran/...` on); result objects use it to
/// detect instance paths in `__getitem__` and resolve them to terminal info.
pub(crate) struct InstanceResolver {
    design: Rc<Design>,
    module_name: String,
}

impl InstanceResolver {
    pub(crate) fn new(design: Rc<Design>, module_name: String) -> Self {
        Self { design, module_name }
    }

    /// A shared handle for the sub-view to clone (cheap `Rc` bump).
    pub(crate) fn shared(&self) -> Self {
        Self {
            design: Rc::clone(&self.design),
            module_name: self.module_name.clone(),
        }
    }

    // SPEC_PRECISION: spec AC13 examples use dot-notation (`buck.r1`); the POM
    // selector grammar (`piperine-lang/src/pom/selector/parse.rs`) uses
    // `/`-separated `axis::name` steps and does not accept `.`. Decision
    // (path-notation, option a): accept dot-notation and translate to selector
    // grammar internally. Rationale — best serves Python ergonomics (matches
    // every spec example) AND the uniform-shape mandate (PY-17): Python users
    // write the same shape they see in the docs; the translation is a private
    // concern of the binding. Bare labels (`r_top`) are detected directly
    // against the parent module's instance list (no separator needed).
    fn to_selector_path(key: &str) -> String {
        let trimmed = key.trim_start_matches('/');
        let body = trimmed.replace('.', "/");
        format!("/{body}")
    }

    /// Whether `key` looks like an instance reference (not a plain net name):
    /// a path separator is present, OR `key` matches an instance label in
    /// the parent module. The caller uses this to route `__getitem__` between
    /// the existing net lookup and the instance sub-view (PY-13).
    pub(crate) fn looks_like_instance(&self, key: &str) -> bool {
        if key.contains('.') || key.contains('/') {
            return true;
        }
        let Some(module) = self.design.module(&self.module_name) else {
            return false;
        };
        module.instances().iter().any(|i| i.name() == key)
    }

    /// Resolve `key` to a single leaf instance label that exists in the POM.
    /// One-segment keys (no separator) are looked up directly; multi-segment
    /// dot-paths are translated to selector grammar and resolved via
    /// [`Design::select`]. `KeyError` for zero matches, `RuntimeError` for
    /// an ambiguous match (spec edge case — fail loud).
    pub(crate) fn resolve_label(&self, key: &str) -> PyResult<String> {
        if !key.contains('.') && !key.contains('/') {
            let module = self.design.module(&self.module_name).ok_or_else(|| {
                PyKeyError::new_err(format!("module `{}` not found", self.module_name))
            })?;
            if module.instances().iter().any(|i| i.name() == key) {
                return Ok(key.to_string());
            }
            return Err(PyKeyError::new_err(format!(
                "`{key}` is not a net or instance of `{}`",
                self.module_name
            )));
        }
        let sel_path = Self::to_selector_path(key);
        let selection = self
            .design
            .select(&sel_path)
            .map_err(|e| PyKeyError::new_err(format!("`{key}` did not resolve: {e}")))?;
        let labels: Vec<String> = selection
            .iter()
            .filter_map(|n| match n {
                Node::Instance(inst) => Some(inst.name().to_string()),
                _ => None,
            })
            .collect();
        match labels.as_slice() {
            [one] => Ok(one.clone()),
            [] => Err(PyKeyError::new_err(format!(
                "`{key}` did not resolve to an instance"
            ))),
            many => Err(PyRuntimeError::new_err(format!(
                "`{key}` resolved to {} instances; expected one",
                many.len()
            ))),
        }
    }

    /// Map `label`'s port names to their connected top-level net names by
    /// walking the POM. Returns `(port_name, net_name)` pairs
    /// in port-declaration order. `KeyError` when the instance or its module
    /// is not found (fail loud).
    pub(crate) fn terminal_nets(&self, label: &str) -> PyResult<Vec<(String, String)>> {
        let module = self.design.module(&self.module_name).ok_or_else(|| {
            PyKeyError::new_err(format!("module `{}` not found", self.module_name))
        })?;
        let inst = module
            .instances()
            .iter()
            .find(|i| i.name() == label)
            .ok_or_else(|| {
                PyKeyError::new_err(format!(
                    "instance `{label}` not found in module `{}`",
                    self.module_name
                ))
            })?;
        let child = self.design.module(inst.module_name()).ok_or_else(|| {
            PyKeyError::new_err(format!(
                "child module `{}` not found",
                inst.module_name()
            ))
        })?;
        inst.ports()
            .iter()
            .zip(child.ports().iter())
            .map(|(binding, port)| Ok((port.name().to_string(), binding.net().to_string())))
            .collect()
    }

    /// Resolve a single `port_name` to its connected top-level net name.
    fn terminal_net(&self, label: &str, port_name: &str) -> PyResult<String> {
        let pairs = self.terminal_nets(label)?;
        pairs
            .into_iter()
            .find(|(p, _)| p == port_name)
            .map(|(_, n)| n)
            .ok_or_else(|| {
                PyKeyError::new_err(format!(
                    "port `{port_name}` not found on instance `{label}`"
                ))
            })
    }
}

/// The parent result an `_InstanceView` projects. Kept as a small enum so a
/// single pyclass covers both `op["r1"]` (scalars) and `trace["r1"]`
/// (waveforms) without two near-identical wrappers (MD-13: one struct, one
/// owner per operation). The bench result lives behind `Rc` so the sub-view
/// shares the parent's snapshot without copying.
pub(crate) enum InstanceResult {
    Op(Rc<OpResult>),
    Trace(Rc<Trace>),
}

/// `_InstanceView` — the terminal sub-view returned by `op["instance.path"]`
/// or `trace["instance.path"]` (PY-13 / spec AC13). Exposes the instance's
/// terminals (`terminals()`), per-terminal voltage (`.v(port)`), and the
/// branch current through the instance (`.i(port_a, port_b)`). Voltages are
/// scalars when the parent is an `_OpResult`, `_Waveform`s when the parent is
/// a `_Trace` — the uniform shape of `.v/.i` over the connected nets.
#[pyclass(module = "piperine", unsendable)]
pub struct _InstanceView {
    inner: InstanceResult,
    resolver: InstanceResolver,
    label: String,
}

impl _InstanceView {
    /// Construct an `_InstanceView` over an op() snapshot (PY-13).
    pub(crate) fn new_op(
        inner: Rc<OpResult>,
        resolver: InstanceResolver,
        label: String,
    ) -> Self {
        Self {
            inner: InstanceResult::Op(inner),
            resolver,
            label,
        }
    }

    /// Construct an `_InstanceView` over a tran() snapshot (PY-13).
    pub(crate) fn new_trace(
        inner: Rc<Trace>,
        resolver: InstanceResolver,
        label: String,
    ) -> Self {
        Self {
            inner: InstanceResult::Trace(inner),
            resolver,
            label,
        }
    }

    /// Translate a Python-side `(port_a, port_b?)` into the connected net
    /// names; `b` defaults to the implicit ground reference (`None`) per the
    /// `.v/.i` convention (spec AC4/AC7).
    fn resolve_pair(
        &self,
        port_a: &str,
        port_b: Option<&str>,
    ) -> PyResult<(piperine::NetRef, Option<piperine::NetRef>)> {
        let a = self.resolver.terminal_net(&self.label, port_a)?;
        let b = match port_b {
            Some(p) => Some(self.resolver.terminal_net(&self.label, p)?),
            None => None,
        };
        Ok((bench_net(&a), b.map(|n| bench_net(&n))))
    }
}

#[pymethods]
impl _InstanceView {
    /// The instance label this view projects (terminal quantities of that
    /// instance). Read-only reflection (PY-13).
    #[getter]
    fn label(&self) -> String {
        self.label.clone()
    }

    /// The instance's terminals as a list of `_Terminal(port, net)` pairs
    /// (port name + connected top-level net name), in port-declaration order
    /// (PY-13: "exposing that instance's terminal quantities").
    fn terminals(&self) -> PyResult<Vec<_Terminal>> {
        Ok(self
            .resolver
            .terminal_nets(&self.label)?
            .into_iter()
            .map(|(port, net)| _Terminal::new(port, net))
            .collect())
    }

    /// Terminal voltage at `port_a` minus `port_b` (ground-referenced when
    /// `port_b` is omitted) — the voltage at the connected net(s). Returns
    /// a `float` when the parent is an `_OpResult`, a `_Waveform` when the
    /// parent is a `_Trace` (uniform-shape over `.v(net)`).
    #[pyo3(signature = (port_a, port_b=None))]
    fn v(&self, port_a: &str, port_b: Option<&str>) -> PyResult<PyObject> {
        let (net_a, net_b) = self.resolve_pair(port_a, port_b)?;
        Python::with_gil(|py| match &self.inner {
            InstanceResult::Op(op) => {
                let f = op.v(&net_a, net_b.as_ref()).map_err(readout_err)?;
                Ok(f.into_pyobject(py)?.into_any().unbind())
            }
            InstanceResult::Trace(trace) => {
                let w = trace.v(&net_a, net_b.as_ref()).map_err(readout_err)?;
                Ok(Py::new(py, _Waveform::new(w))?.into_any())
            }
        })
    }

    /// Branch current from `port_a` to `port_b` (ground-referenced when
    /// `port_b` is omitted) through this instance — the current in the
    /// branch the instance's two terminals define. Returns a `float` when
    /// the parent is an `_OpResult`, a `_Waveform` when the parent is a
    /// `_Trace`.
    #[pyo3(signature = (port_a, port_b=None))]
    fn i(&self, port_a: &str, port_b: Option<&str>) -> PyResult<PyObject> {
        let (net_a, net_b) = self.resolve_pair(port_a, port_b)?;
        Python::with_gil(|py| match &self.inner {
            InstanceResult::Op(op) => {
                let f = op.i(&net_a, net_b.as_ref()).map_err(readout_err)?;
                Ok(f.into_pyobject(py)?.into_any().unbind())
            }
            InstanceResult::Trace(trace) => {
                let w = trace.i(&net_a, net_b.as_ref()).map_err(readout_err)?;
                Ok(Py::new(py, _Waveform::new(w))?.into_any())
            }
        })
    }

    /// `view[port]` SHALL equal `view.v(port)` (uniform shape: the same
    /// `__getitem__ → .v` mapping the parent result defines for net names).
    fn __getitem__(&self, port: &str) -> PyResult<PyObject> {
        self.v(port, None)
    }
}

/// Construct a bench [`piperine::NetRef`] from a net name — the typed
/// handle every bench readout takes (MD-13: lives on the file that owns the
/// conversion).
fn bench_net(name: &str) -> piperine::NetRef {
    piperine::NetRef {
        name: name.to_string(),
    }
}

/// `_Terminal` — one entry in an `_InstanceView`'s terminal list: a port name
/// and the top-level net it connects to. Read-only reflection (PY-13).
#[pyclass(module = "piperine")]
pub struct _Terminal {
    port: String,
    net: String,
}

impl _Terminal {
    fn new(port: String, net: String) -> Self {
        Self { port, net }
    }
}

#[pymethods]
impl _Terminal {
    /// The port name on the instance's module declaration.
    #[getter]
    fn port(&self) -> String {
        self.port.clone()
    }

    /// The top-level net name this terminal connects to (the parent module's
    /// scope). Voltage/current reads on the parent result use this name.
    #[getter]
    fn net(&self) -> String {
        self.net.clone()
    }
}

// (`_OpResult` and `_Trace` pyclasses are not referenced directly here — the
// enum holds the bench `OpResult`/`Trace` types behind `Rc`, not the Python
// wrappers. The wrappers' `shared()` methods produce the `Rc`s this module
// consumes.)
