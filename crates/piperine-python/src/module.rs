//! `_Module` and its reflected children — read-only views over one POM
//! module's ports/nets/instances/params/behaviors (PY-03).

use std::rc::Rc;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use piperine_lang::parse::ast::{BehaviorKind, Direction};
use piperine_lang::{Behavior, Design, Instance, Module, Param, Port, ValueType, Wire};

use crate::value_bridge::PyValue;

/// `_Module` — a reflected view of a named module in a shared [`Design`].
/// Stores `(Rc<Design>, name)` and re-looks the module up on each call so the
/// GIL-bound lifetime never fights the POM borrow (design
/// `python-bindings/design.md` — POM borrow-lifetime risk).
///
/// `unsendable`: shares an `Rc<Design>` whose interior is not `Sync` (see
/// [`crate::_Design`]); single-interpreter use only.
#[pyclass(module = "piperine", unsendable)]
pub struct _Module {
    design: Rc<Design>,
    name: String,
}

impl _Module {
    pub(crate) fn new(design: Rc<Design>, name: String) -> Self {
        Self { design, name }
    }

    /// Re-resolve the live module from the shared POM.
    fn module(&self) -> PyResult<&Module> {
        self.design.module(&self.name).ok_or_else(|| {
            PyValueError::new_err(format!("module `{}` is no longer present", self.name))
        })
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
