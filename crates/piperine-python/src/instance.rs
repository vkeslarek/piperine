//! `_InstanceView` — the terminal sub-view returned by `result["instance.path"]`
//! (PY-13 / spec AC13). A thin wrapper over
//! [`piperine_api::model::InstanceView`] (CLA-18): resolution, connectivity,
//! and terminal readouts all delegate to the api model; what remains here is
//! the `PyErr` mapping and the trace-view guards that keep the documented
//! Python error messages byte-identical.

use std::rc::Rc;

use pyo3::exceptions::{PyKeyError, PyRuntimeError};
use pyo3::prelude::*;

use piperine_api::model::InstanceReadout;
use piperine_api::{OpResult, Trace};
use piperine_lang::Design;

use crate::results::{readout_err, _Waveform};

/// Bridge between a hierarchical/dot-notation instance path the user types
/// and the POM instance it names — the Python-facing shell over
/// [`piperine_api::model::InstanceResolver`], kept so `results.rs`/`live.rs`
/// construct and route through one type (delegation + `PyErr` mapping only).
pub(crate) struct InstanceResolver {
    inner: piperine_api::model::InstanceResolver,
}

impl InstanceResolver {
    pub(crate) fn new(design: Rc<Design>, module_name: String) -> Self {
        Self { inner: piperine_api::model::InstanceResolver::new(design, module_name) }
    }

    /// A shared handle for the sub-view to clone (cheap `Rc` bump).
    pub(crate) fn shared(&self) -> Self {
        Self { inner: self.inner.clone() }
    }

    /// Whether `key` looks like an instance reference (not a plain net
    /// name): a path separator is present, OR `key` matches an instance
    /// label in the parent module. The caller uses this to route
    /// `__getitem__` between the existing net lookup and the instance
    /// sub-view (PY-13).
    pub(crate) fn looks_like_instance(&self, key: &str) -> bool {
        self.inner.looks_like_instance(key)
    }

    /// Resolve `key` to a single leaf instance label that exists in the POM.
    /// `KeyError` for zero matches, `RuntimeError` for an ambiguous match
    /// (spec edge case — fail loud).
    pub(crate) fn resolve_label(&self, key: &str) -> PyResult<String> {
        self.inner.resolve_label(key).map_err(|e| match e {
            piperine_api::Error::NotFound(msg) => PyKeyError::new_err(msg),
            other => PyRuntimeError::new_err(format!("{other}")),
        })
    }

    /// Hand the api resolver to the api view constructor.
    fn api(&self) -> piperine_api::model::InstanceResolver {
        self.inner.clone()
    }
}

/// `_InstanceView` — the terminal sub-view returned by `op["instance.path"]`
/// or `trace["instance.path"]` (PY-13 / spec AC13). Exposes the instance's
/// terminals (`terminals()`), per-terminal voltage (`.v(port)`), and the
/// branch current through the instance (`.i(port_a, port_b)`). Voltages are
/// scalars when the parent is an `_OpResult`, `_Waveform`s when the parent is
/// a `_Trace` — the uniform shape of `.v/.i` over the connected nets.
#[pyclass(module = "piperine", unsendable)]
pub struct _InstanceView {
    inner: piperine_api::model::InstanceView,
    /// Trace-bound views guard the op-side accessors (`opvar`/`model`/…)
    /// with the documented `RuntimeError`s — the api view stays host-neutral,
    /// the message contract is the binding's.
    trace: bool,
}

impl _InstanceView {
    /// Construct an `_InstanceView` over an op() snapshot (PY-13).
    pub(crate) fn new_op(
        inner: Rc<OpResult>,
        resolver: InstanceResolver,
        label: String,
    ) -> PyResult<Self> {
        piperine_api::model::InstanceView::new_op(inner, resolver.api(), &label)
            .map(|inner| Self { inner, trace: false })
            .map_err(readout_err)
    }

    /// Construct an `_InstanceView` over a tran() snapshot (PY-13).
    pub(crate) fn new_trace(
        inner: Rc<Trace>,
        resolver: InstanceResolver,
        label: String,
    ) -> Self {
        Self {
            inner: piperine_api::model::InstanceView::new_trace(inner, resolver.api(), &label),
            trace: true,
        }
    }

    /// The documented trace-view guard for the op-side accessors.
    fn require_op_side(&self, what: &str) -> PyResult<()> {
        if self.trace {
            return Err(PyRuntimeError::new_err(format!("{what} is not available on a trace view")));
        }
        Ok(())
    }
}

#[pymethods]
impl _InstanceView {
    /// The instance label this view projects (terminal quantities of that
    /// instance). Read-only reflection (PY-13).
    #[getter]
    fn label(&self) -> String {
        self.inner.label().to_string()
    }

    /// The instance's terminal connectivity as a list of `_Terminal(port, net)`
    /// pairs (port name + connected top-level net name), in port-declaration
    /// order (PY-13: "exposing that instance's terminal quantities"). Renamed
    /// from `terminals()` to free the `terminals` property for the HOST-09
    /// descriptor catalog.
    fn terminal_connections(&self) -> PyResult<Vec<_Terminal>> {
        Ok(self
            .inner
            .terminal_connections()
            .map_err(readout_err)?
            .iter()
            .map(|t| _Terminal::new(t.port().to_string(), t.net().to_string()))
            .collect())
    }

    /// Terminal voltage at `port_a` minus `port_b` (ground-referenced when
    /// `port_b` is omitted) — the voltage at the connected net(s). Returns
    /// a `float` when the parent is an `_OpResult`, a `_Waveform` when the
    /// parent is a `_Trace` (uniform-shape over `.v(net)`).
    #[pyo3(signature = (port_a, port_b=None))]
    fn v(&self, py: Python<'_>, port_a: &str, port_b: Option<&str>) -> PyResult<PyObject> {
        match self.inner.v(port_a, port_b).map_err(readout_err)? {
            InstanceReadout::Scalar(f) => Ok(f.into_pyobject(py)?.into_any().unbind()),
            InstanceReadout::Waveform(w) => Ok(Py::new(py, _Waveform::new(w))?.into_any()),
        }
    }

    /// Branch current from `port_a` to `port_b` (ground-referenced when
    /// `port_b` is omitted) through this instance — the current in the
    /// branch the instance's two terminals define. Returns a `float` when
    /// the parent is an `_OpResult`, a `_Waveform` when the parent is a
    /// `_Trace`.
    #[pyo3(signature = (port_a, port_b=None))]
    fn i(&self, py: Python<'_>, port_a: &str, port_b: Option<&str>) -> PyResult<PyObject> {
        match self.inner.i(port_a, port_b).map_err(readout_err)? {
            InstanceReadout::Scalar(f) => Ok(f.into_pyobject(py)?.into_any().unbind()),
            InstanceReadout::Waveform(w) => Ok(Py::new(py, _Waveform::new(w))?.into_any()),
        }
    }

    /// `view[port]` SHALL equal `view.v(port)` (uniform shape: the same
    /// `__getitem__ → .v` mapping the parent result defines for net names).
    fn __getitem__(&self, py: Python<'_>, port: &str) -> PyResult<PyObject> {
        self.v(py, port, None)
    }

    /// The device's computed operating-point variable `name` (HOST-07):
    /// reads the `read_opvars` snapshot taken when the parent `.op()`
    /// solved. Fails loud — never `None`/`NaN` — on an unknown opvar or on
    /// a trace view (recorded observables use `trace.opvar(path)` instead,
    /// HOST-08).
    fn opvar(&self, name: &str) -> PyResult<f64> {
        if self.trace {
            return Err(PyRuntimeError::new_err(
                "opvar() is not available on a trace view; use trace.opvar(path) instead",
            ));
        }
        self.inner.opvar(name).map_err(readout_err)
    }

    /// Every opvar this device declared, as `(name, value)` pairs (HOST-07).
    fn opvars(&self) -> PyResult<Vec<(String, f64)>> {
        if self.trace {
            return Err(PyRuntimeError::new_err(
                "opvars() is not available on a trace view; use trace.opvar(path) instead",
            ));
        }
        Ok(self.inner.opvars())
    }

    /// The device's model identity (HOST-09 / ABI-46): `type_id` and
    /// `version` from the shipped `model_descriptor()` catalog. Read-only
    /// reflection — not available on a trace view.
    #[getter]
    fn model(&self) -> PyResult<_ModelDescriptor> {
        self.require_op_side("model")?;
        Ok(_ModelDescriptor::from_solver(self.inner.model().clone()))
    }

    /// The device's terminal descriptors (HOST-09 / ABI-27): each carrying
    /// `.name`, `.kind` (`"external"`/`"internal"`/`"auxiliary"`), `.domain`,
    /// `.direction`. Read-only reflection — not available on a trace view.
    /// For port→net connectivity, use `terminal_connections()`.
    #[getter]
    fn terminals(&self) -> PyResult<Vec<_TerminalDescriptor>> {
        self.require_op_side("terminals")?;
        Ok(self.inner.terminals().to_vec().into_iter().map(_TerminalDescriptor::from_solver).collect())
    }

    /// The device's observable catalog (HOST-09 / ABI-32): what CAN be probed
    /// via `probe=["inst.name"]` — each entry carries `.name`, `.kind`, and
    /// `.cost`. Read-only reflection — not available on a trace view.
    fn observables(&self) -> PyResult<Vec<_ObservableDescriptor>> {
        self.require_op_side("observables()")?;
        Ok(self.inner.observables().to_vec().into_iter().map(_ObservableDescriptor::from_solver).collect())
    }

    /// The device's parameter descriptor for `name` (HOST-12): `bounds`,
    /// `unit`, `scope`, `invalidation`. Fails loud on an unknown param.
    fn param(&self, name: &str) -> PyResult<_ParamDescriptor> {
        self.require_op_side("param()")?;
        Ok(_ParamDescriptor::from_solver(self.inner.param(name).map_err(readout_err)?.clone()))
    }

    /// The device's full parameter descriptor catalog (HOST-12): `bounds`/
    /// `unit`/`scope`/`invalidation` for each declared parameter.
    fn params(&self) -> PyResult<Vec<_ParamDescriptor>> {
        self.require_op_side("params()")?;
        Ok(self.inner.params().to_vec().into_iter().map(_ParamDescriptor::from_solver).collect())
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

/// `_ModelDescriptor` — model identity and version (HOST-09 / ABI-46).
/// Uniform-shape (MD-22): mirrors the api `ModelDescriptor` — `type_id` +
/// `version`, same field names on both hosts.
#[pyclass(module = "piperine")]
pub struct _ModelDescriptor {
    #[pyo3(get)]
    type_id: String,
    #[pyo3(get)]
    version: String,
}

impl _ModelDescriptor {
    pub(crate) fn from_solver(m: piperine_api::ModelDescriptor) -> Self {
        Self { type_id: m.type_id, version: m.version }
    }
}

/// `_TerminalDescriptor` — one terminal's metadata (HOST-09 / ABI-27). The
/// `.kind` string (`"external"`/`"internal"`/`"auxiliary"`) carries the
/// terminal classification; `.domain` is `"analog"` or `"digital"`.
/// Uniform-shape (MD-22): mirrors the api `TerminalDescriptor`.
#[pyclass(module = "piperine")]
pub struct _TerminalDescriptor {
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    kind: String,
    #[pyo3(get)]
    domain: String,
    #[pyo3(get)]
    direction: String,
}

impl _TerminalDescriptor {
    pub(crate) fn from_solver(t: piperine_solver::prelude::TerminalDescriptor) -> Self {
        use piperine_solver::prelude::{Direction, Domain, TerminalKind};
        let kind = match t.kind {
            TerminalKind::External => "external",
            TerminalKind::Internal => "internal",
            TerminalKind::Auxiliary => "auxiliary",
        };
        let domain = match t.domain {
            Domain::Analog => "analog",
            Domain::Digital => "digital",
        };
        let direction = match t.direction {
            Direction::In => "in",
            Direction::Out => "out",
            Direction::Inout => "inout",
        };
        Self {
            name: t.name,
            kind: kind.to_string(),
            domain: domain.to_string(),
            direction: direction.to_string(),
        }
    }
}

/// `_ObservableDescriptor` — one device-declared observable (HOST-09 /
/// ABI-32): the source-level `.name`, what the recorded value represents
/// (`.kind`), and a relative recording `.cost` hint.
#[pyclass(module = "piperine")]
pub struct _ObservableDescriptor {
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    kind: String,
    #[pyo3(get)]
    cost: f32,
}

impl _ObservableDescriptor {
    pub(crate) fn from_solver(o: piperine_solver::prelude::ObservableDescriptor) -> Self {
        use piperine_solver::prelude::ObservableKind;
        let kind = match o.kind {
            ObservableKind::BranchCurrent => "branch_current",
            ObservableKind::Charge => "charge",
            ObservableKind::Flux => "flux",
            ObservableKind::State => "state",
            ObservableKind::Var => "var",
        };
        Self { name: o.name, kind: kind.to_string(), cost: o.cost }
    }
}

/// `_ParamDescriptor` — one parameter's metadata (HOST-12): `bounds`,
/// `unit`, `scope`, `invalidation`. The `.bounds` is a `(min, max)` tuple
/// (either may be `None` = unbounded). Uniform-shape (MD-22): mirrors the
/// api `ParamDescriptor`.
#[pyclass(module = "piperine")]
pub struct _ParamDescriptor {
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    bounds: (Option<f64>, Option<f64>),
    #[pyo3(get)]
    unit: Option<String>,
    #[pyo3(get)]
    scope: String,
    #[pyo3(get)]
    invalidation: String,
}

impl _ParamDescriptor {
    pub(crate) fn from_solver(p: piperine_solver::prelude::ParamDescriptor) -> Self {
        use piperine_solver::prelude::{Invalidation, ParamScope};
        let scope = match p.scope {
            ParamScope::Model => "model",
            ParamScope::Instance => "instance",
        };
        let invalidation = match p.invalidation {
            Invalidation::None => "none",
            Invalidation::Restamp => "restamp",
            Invalidation::Temperature => "temperature",
            Invalidation::OperatingPoint => "operating_point",
            Invalidation::Rebuild => "rebuild",
        };
        Self {
            name: p.name,
            bounds: (p.bounds.min, p.bounds.max),
            unit: p.unit,
            scope: scope.to_string(),
            invalidation: invalidation.to_string(),
        }
    }
}
