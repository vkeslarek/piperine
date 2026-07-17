//! `_Module` and its reflected children — read-only views over one POM
//! module's ports/nets/instances/params/behaviors (PY-03), plus the four
//! analyses (`op/tran/ac/noise`, PY-04) and `stage` (PY-12) that turn a
//! reflected module into a runnable one.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use pyo3::exceptions::{PyKeyError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;

use piperine::{SimSession, SolverConfig};
use piperine_lang::parse::ast::{BehaviorKind, Direction};
use piperine_lang::{Behavior, Design, Instance, Module, Param, Port, Value, ValueType, Wire};

use crate::results::_AcTrace;
use crate::results::_NoiseTrace;
use crate::results::_OpResult;
use crate::results::_Trace;
use crate::value_bridge::PyValue;

/// `_Module` — a reflected view of a named module in a shared [`Design`].
/// Stores `(Rc<Design>, name)` and re-looks the module up on each call so the
/// GIL-bound lifetime never fights the POM borrow (design
/// `python-bindings/design.md` — POM borrow-lifetime risk).
///
/// Staged overrides (`stage`, PY-12) are held in an isolated map and applied
/// to a fresh [`Design::fork`] per analysis call — the held parent `Design`
/// is never mutated (spec AC11), and each analysis is a pure function of the
/// design + currently-staged overrides + config (piperine-bench/docs/SPEC.md
/// §9 isolation). A re-stage of the same `(label, param)` overwrites the
/// previous value, matching the bench's last-write-wins staging semantics.
///
/// `unsendable`: shares an `Rc<Design>` whose interior is not `Sync` (see
/// [`crate::_Design`]); single-interpreter use only.
#[pyclass(module = "piperine", unsendable)]
pub struct _Module {
    design: Rc<Design>,
    name: String,
    /// `(instance label, param name) → staged value`. Isolated from the
    /// parent design so the user's `_Design` is untouched (AC11). Applied to
    /// each analysis's fork before solving.
    staged: RefCell<HashMap<(String, String), Value>>,
}

impl _Module {
    pub(crate) fn new(design: Rc<Design>, name: String) -> Self {
        Self {
            design,
            name,
            staged: RefCell::new(HashMap::new()),
        }
    }

    /// Re-resolve the live module from the shared POM.
    fn module(&self) -> PyResult<&Module> {
        self.design.module(&self.name).ok_or_else(|| {
            PyValueError::new_err(format!("module `{}` is no longer present", self.name))
        })
    }

    /// Build a fresh [`SimSession`] for one analysis: fork the parent design,
    /// replay every staged override onto the fork (the fork clears the
    /// parent's override layer by construction — see [`Design::fork`]), then
    /// hand the forked design to a new session. Each analysis call gets its
    /// own session + fork, so results never leak between calls (spec §9).
    fn session(&self) -> PyResult<SimSession> {
        let forked = self.design.fork();
        for ((label, param), value) in self.staged.borrow().iter() {
            forked.set_param(label, param, value.clone());
        }
        Ok(SimSession::new(forked, self.name.clone()))
    }

    /// Surface a bench analysis error as the right Python exception:
    /// net-not-addressable reads as `KeyError` (spec edge case — fail loud,
    /// never a silent NaN); everything else as `RuntimeError` carrying the
    /// diagnostic. Both error types implement `Display` via `thiserror`.
    fn analysis_err<E: std::fmt::Display>(e: E) -> PyErr {
        let msg = format!("{e}");
        if msg.contains("is not addressable") {
            PyKeyError::new_err(msg)
        } else {
            PyRuntimeError::new_err(msg)
        }
    }

    /// Build the [`InstanceResolver`] handed to result objects so they can
    /// detect instance paths in `__getitem__` (PY-13). The resolver shares
    /// this module's design handle — a fresh clone per call so each result
    /// owns its own (cheap `Rc` bump).
    fn instance_resolver(&self) -> crate::instance::InstanceResolver {
        crate::instance::InstanceResolver::new(Rc::clone(&self.design), self.name.clone())
    }

    /// Map the facade's `Solver` dataclass onto the host [`SolverConfig`].
    /// Duck-typed (reads the prelude `bundle Solver` fields by attribute) so
    /// the facade needs no mirrored pyclass; `None` keeps the defaults.
    /// A missing/mistyped attribute fails loud, never a silent default.
    /// Shared with [`crate::live::_LiveSession`] (LIVE-13: one config mapping).
    pub(crate) fn solver_config(solver: Option<&Bound<'_, PyAny>>) -> PyResult<SolverConfig> {
        let mut sc = SolverConfig::default();
        if let Some(obj) = solver {
            sc.temperature = obj.getattr("temperature")?.extract()?;
            sc.reltol = obj.getattr("reltol")?.extract()?;
            sc.abstol = obj.getattr("abstol")?.extract()?;
            sc.gmin = obj.getattr("gmin")?.extract()?;
            sc.max_iter = obj.getattr("max_iter")?.extract()?;
        }
        Ok(sc)
    }
}

#[pymethods]
impl _Module {
    /// The module's declared name (re-resolved against the live POM).
    #[getter]
    fn name(&self) -> PyResult<String> {
        Ok(self.module()?.name().to_string())
    }

    /// The module's ports (PY-03 / spec AC14).
    fn ports(&self) -> PyResult<Vec<_Port>> {
        Ok(self.module()?.ports().iter().map(_Port::of).collect())
    }

    /// The module's nets (its `wire` declarations) (PY-03 / spec AC14).
    fn nets(&self) -> PyResult<Vec<_Net>> {
        Ok(self.module()?.wires().iter().map(_Net::of).collect())
    }

    /// The module's submodule instances (PY-03 / spec AC14).
    fn instances(&self) -> PyResult<Vec<_Instance>> {
        Ok(self.module()?.instances().iter().map(_Instance::of).collect())
    }

    /// The module's params (PY-03 / spec AC14).
    fn params(&self) -> PyResult<Vec<_Param>> {
        Ok(self.module()?.params().iter().map(_Param::of).collect())
    }

    /// The module's `analog`/`digital` behavior blocks (PY-03 / spec AC14).
    fn behaviors(&self) -> PyResult<Vec<_Behavior>> {
        Ok(self.module()?.behaviors().iter().map(_Behavior::of).collect())
    }

    // ── analyses (PY-04) + staging (PY-12) ─────────────────────────────────
    //
    // Each analysis builds a fresh `SimSession` over a forked design with the
    // staged overrides replayed (see [`_Module::session`]); solver config
    // defaults to [`SolverConfig::default`]. The signatures mirror
    // `SimSession::run_*` positionally — the facade (P10) wraps these with
    // typed dataclasses (`OpConfig`/`TranConfig`/...).

    /// Run a DC operating-point analysis (PY-04 / spec AC3). Returns the
    /// solved node voltages + branch currents as an [`_OpResult`].
    /// `nodeset` seeds the Newton initial guess (spec §5.1 `OpConfig.nodeset`);
    /// `solver` is the facade's `Solver` dataclass (tolerances + max_iter).
    #[pyo3(signature = (nodeset=None, solver=None))]
    fn op(
        &self,
        nodeset: Option<HashMap<String, f64>>,
        solver: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<_OpResult> {
        let session = self.session()?;
        let result = session
            .run_op(&Self::solver_config(solver)?, nodeset.as_ref())
            .map_err(Self::analysis_err)?;
        Ok(_OpResult::new(result).with_resolver(self.instance_resolver()))
    }

    /// Run a transient analysis (PY-04 / spec AC6). `step = None` (or `0.0`)
    /// selects the adaptive stepper; `start` is the earliest recorded time
    /// (piperine-bench/docs/SPEC.md §5.1 `TranConfig.start`). `ic` is an
    /// optional per-node initial-condition map (spec §5.1 `TranConfig.ic`).
    #[pyo3(signature = (stop, step=None, start=0.0, ic=None, solver=None))]
    fn tran(
        &self,
        stop: f64,
        step: Option<f64>,
        start: f64,
        ic: Option<HashMap<String, f64>>,
        solver: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<_Trace> {
        let session = self.session()?;
        let result = session
            .run_tran(stop, step, start, &Self::solver_config(solver)?, ic.as_ref())
            .map_err(Self::analysis_err)?;
        Ok(_Trace::new(result).with_resolver(self.instance_resolver()))
    }

    /// Run an AC small-signal sweep (PY-04 / spec AC8). `logarithmic` defaults
    /// to `true` (matches the prelude's `Scale::Dec` default).
    #[pyo3(signature = (fstart, fstop, points=100, logarithmic=true, solver=None))]
    fn ac(
        &self,
        fstart: f64,
        fstop: f64,
        points: usize,
        logarithmic: bool,
        solver: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<_AcTrace> {
        let session = self.session()?;
        let result = session
            .run_ac(fstart, fstop, points, logarithmic, &Self::solver_config(solver)?)
            .map_err(Self::analysis_err)?;
        Ok(_AcTrace::new(result))
    }

    /// Run an output-referred noise analysis (PY-04 / spec AC9). `reference`
    /// defaults to `"gnd"` (the single-net `NoiseConfig.out` form).
    #[pyo3(signature = (out, fstart, fstop, points=100, reference="gnd", logarithmic=true, solver=None))]
    fn noise(
        &self,
        out: &str,
        fstart: f64,
        fstop: f64,
        points: usize,
        reference: &str,
        logarithmic: bool,
        solver: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<_NoiseTrace> {
        let session = self.session()?;
        let result = session
            .run_noise(
                out,
                reference,
                fstart,
                fstop,
                points,
                logarithmic,
                &Self::solver_config(solver)?,
            )
            .map_err(Self::analysis_err)?;
        Ok(_NoiseTrace::new(result))
    }

    /// Stage a parameter override on `label`'s `param` (PY-12 / spec AC11):
    /// the next analysis on this module uses `value`. Staging is pure — the
    /// held [`crate::design::_Design`] is never mutated; overrides live in an
    /// isolated map and replay onto each analysis's fork. A re-stage of the
    /// same `(label, param)` overwrites. Sweeps are native Python `for` loops
    /// (spec AC12).
    fn stage(&self, label: &str, param: &str, value: f64) {
        self.staged
            .borrow_mut()
            .insert((label.to_string(), param.to_string()), Value::Real(value));
    }

    /// Compile this module **once** into a live session (LIVE-10): the
    /// returned [`crate::live::_LiveSession`] holds the elaborated design and
    /// the JIT-compiled circuit; `set` + re-run analyses never recompile
    /// (MD-18). Currently staged overrides are baked into the compilation
    /// (same replay as [`Self::session`]); the parent `_Design` stays
    /// untouched.
    fn compile(&self) -> PyResult<crate::live::_LiveSession> {
        crate::live::_LiveSession::from_design(&self.design, &self.name, &self.staged.borrow())
    }
}

// ── reflected children ───────────────────────────────────────────────────────
//
// Each child snapshots its attributes at construction (when `_Module::ports()`
// / etc. enumerate the POM). The binding is read-only, so a snapshot is both
// lifetime-safe across the FFI boundary and an honest reflection of the POM
// at enumeration time.

/// A reflected port — name, direction, and net (discipline) type.
#[pyclass(module = "piperine")]
pub struct _Port {
    name: String,
    direction: String,
    ty: String,
}

impl _Port {
    fn of(port: &Port) -> Self {
        Self {
            name: port.name().to_string(),
            direction: match port.direction() {
                Direction::Input => "in",
                Direction::Output => "out",
                Direction::Inout => "inout",
            }
            .to_string(),
            ty: port.net_type().discipline_name().to_string(),
        }
    }
}

#[pymethods]
impl _Port {
    #[getter]
    fn name(&self) -> String {
        self.name.clone()
    }
    #[getter]
    fn direction(&self) -> String {
        self.direction.clone()
    }
    /// The net (discipline) type, e.g. `"Electrical"`.
    #[getter]
    fn ty(&self) -> String {
        self.ty.clone()
    }
}

/// A reflected net (wire) — name and net (discipline) type.
#[pyclass(module = "piperine")]
pub struct _Net {
    name: String,
    ty: String,
}

impl _Net {
    fn of(wire: &Wire) -> Self {
        Self {
            name: wire.name().to_string(),
            ty: wire.net_type().discipline_name().to_string(),
        }
    }
}

#[pymethods]
impl _Net {
    #[getter]
    fn name(&self) -> String {
        self.name.clone()
    }
    /// The net (discipline) type, e.g. `"Electrical"`.
    #[getter]
    fn ty(&self) -> String {
        self.ty.clone()
    }
}

/// A reflected submodule instance — label and the module it instantiates.
#[pyclass(module = "piperine")]
pub struct _Instance {
    name: String,
    module: String,
}

impl _Instance {
    fn of(inst: &Instance) -> Self {
        Self {
            name: inst.name().to_string(),
            module: inst.module_name().to_string(),
        }
    }
}

#[pymethods]
impl _Instance {
    /// The instance label (or the module name when unlabeled).
    #[getter]
    fn name(&self) -> String {
        self.name.clone()
    }
    /// The name of the module this instance instantiates.
    #[getter]
    fn module(&self) -> String {
        self.module.clone()
    }
}

/// A reflected param — name, value type, and default value.
#[pyclass(module = "piperine", unsendable)]
pub struct _Param {
    name: String,
    ty: String,
    default: Option<piperine_lang::Value>,
}

impl _Param {
    fn of(param: &Param) -> Self {
        Self {
            name: param.name().to_string(),
            ty: match param.value_type() {
                ValueType::Real => "Real",
                ValueType::Natural => "Natural",
                ValueType::Integer => "Integer",
                ValueType::Complex => "Complex",
                ValueType::Boolean => "Boolean",
                ValueType::Quad => "Quad",
                ValueType::Str => "String",
                ValueType::Enum(name) | ValueType::Bundle(name) => name,
                ValueType::Tuple(_) => "Tuple",
                ValueType::Array(_, _) => "Array",
                ValueType::FnPtr(..) => "Fn",
            }
            .to_string(),
            default: param.default().cloned(),
        }
    }
}

#[pymethods]
impl _Param {
    #[getter]
    fn name(&self) -> String {
        self.name.clone()
    }
    /// The declared value type (e.g. `"Real"`, or an enum/bundle name).
    #[getter]
    fn ty(&self) -> String {
        self.ty.clone()
    }
    /// The default value, or `None` if the param has none.
    #[getter]
    fn default(&self, py: Python<'_>) -> PyResult<PyObject> {
        match &self.default {
            Some(v) => PyValue(v).to_object(py),
            None => Ok(py.None()),
        }
    }
}

/// A reflected `analog`/`digital` behavior block.
#[pyclass(module = "piperine")]
pub struct _Behavior {
    name: String,
    kind: String,
}

impl _Behavior {
    fn of(beh: &Behavior) -> Self {
        Self {
            name: beh.name().to_string(),
            kind: match beh.kind() {
                BehaviorKind::Analog => "analog",
                BehaviorKind::Digital => "digital",
            }
            .to_string(),
        }
    }
}

#[pymethods]
impl _Behavior {
    #[getter]
    fn name(&self) -> String {
        self.name.clone()
    }
    /// `"analog"` or `"digital"`.
    #[getter]
    fn kind(&self) -> String {
        self.kind.clone()
    }
}
