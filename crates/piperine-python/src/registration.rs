//! T28 (DVT-20/21 Python half): `@pip.device` registers a Python host
//! device — reusing T17's registration point (`HostDeviceRegistry::
//! register`/`register_sequencer`), not a parallel inventory. The Python
//! facade's `device(type_id)` decorator (already shipping for the
//! plugin-glue case, PLG-05/06) is the same table this reads from
//! (`_python_devices()`, a non-destructive read — see the facade's
//! docstring on it).
//!
//! A decorated class is dispatched to one of two `HostElement` shapes
//! purely by **method presence**, mirroring T27's own rule: `hasattr(cls,
//! "run")` builds a [`Sequencer<PyProgram>`] (registered via
//! [`HostDeviceRegistry::register_sequencer`], so D13's AC refusal applies
//! to it); otherwise a plain [`PyHostElement`].

use std::rc::Rc;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use piperine_api::device::{HostAdapter, HostDeviceRegistry, HostDeviceSpec};
use piperine_api::{NetRef, Session, SolverConfig};
use piperine_codegen::device::PortBinding;
use piperine_lang::{parse_and_elaborate_seeded, seed_device_port_schemas};
use piperine_solver::abi::Element;

use crate::device::{PyHostElement, _NetRef};
use crate::sequencer_bridge::{build_python_sequencer, _DigitalNetRef};

/// Build a Python object argument list from a [`HostDeviceSpec`]'s ports:
/// each analog port becomes a [`_NetRef`], each digital port a
/// [`_DigitalNetRef`] — passed to the decorated class as `cls(ports,
/// params)`.
fn ports_to_py(py: Python<'_>, ports: &[PortBinding]) -> PyResult<Py<PyList>> {
    let items: PyResult<Vec<Py<PyAny>>> = ports
        .iter()
        .map(|p| match p {
            PortBinding::Analog(r) => Ok(Py::new(py, _NetRef(r.clone()))?.into_any()),
            PortBinding::Digital(net) => Ok(Py::new(py, _DigitalNetRef(*net))?.into_any()),
        })
        .collect();
    Ok(PyList::new(py, items?)?.unbind())
}

fn params_to_py(py: Python<'_>, params: &[(String, piperine_lang::Value)]) -> PyResult<Py<PyDict>> {
    let dict = PyDict::new(py);
    for (name, value) in params {
        let v = match value {
            piperine_lang::Value::Real(r) => *r,
            piperine_lang::Value::Int(i) => *i as f64,
            piperine_lang::Value::Nat(n) => *n as f64,
            _ => continue,
        };
        dict.set_item(name, v)?;
    }
    Ok(dict.unbind())
}

fn build_element(py: Python<'_>, cls: &Py<PyAny>, spec: &HostDeviceSpec) -> PyResult<Box<dyn Element>> {
    let ports = ports_to_py(py, &spec.ports)?;
    let params = params_to_py(py, &spec.params)?;
    let obj = cls.bind(py).call1((ports, params))?;
    if obj.hasattr("run")? {
        let seq = build_python_sequencer(py, spec.instance_label.clone(), obj.unbind(), 0.0, f64::INFINITY)?;
        Ok(Box::new(HostAdapter(seq)))
    } else {
        let bridge = PyHostElement::new(py, obj.unbind())?;
        Ok(Box::new(HostAdapter(bridge)))
    }
}

/// Register every class declared via `@pip.device(type_id)` (read from the
/// facade's `_python_devices()`) into `registry` — the Python half of
/// T17's `HostDeviceRegistry`. `py` must have the `piperine` facade module
/// already registered (`embed::register_modules`).
pub fn register_python_devices(py: Python<'_>, registry: &HostDeviceRegistry) -> PyResult<()> {
    let facade = py.import("piperine")?;
    let devices: Py<PyDict> = facade.getattr("_python_devices")?.call0()?.extract()?;
    for (type_id, cls) in devices.bind(py).iter() {
        let type_id: String = type_id.extract()?;
        let cls: Py<PyAny> = cls.unbind();
        let is_sequencer = cls.bind(py).hasattr("run")?;
        let cls_for_factory = cls.clone_ref(py);
        let factory = move |spec: HostDeviceSpec| -> Result<Box<dyn Element>, String> {
            Python::with_gil(|py| build_element(py, &cls_for_factory, &spec).map_err(|e| e.to_string()))
        };
        if is_sequencer {
            registry.register_sequencer(type_id, factory);
        } else {
            registry.register(type_id, factory);
        }
    }
    Ok(())
}

/// Test/script seam for `@pip.device` end to end (T28): elaborate `phdl`,
/// build a [`HostDeviceRegistry`] from every currently-declared
/// `@pip.device(type_id)` class (the class(es) must already be defined —
/// e.g. by a preceding `exec`/`import` in the same script), compile a
/// [`Session`] for `top` through it, run an operating-point analysis, and
/// return `v(net)`. One native call so a Python script (the project's
/// established test idiom — `run_script`, `facade_lock`/`register_modules`
/// stay `pub(crate)`) can drive the whole path without needing those
/// crate-private embedding primitives itself.
#[pyfunction]
pub fn _solve_with_python_devices(py: Python<'_>, phdl: &str, top: &str, net: &str) -> PyResult<f64> {
    let design = parse_and_elaborate_seeded(phdl, &piperine_lang::SourceMap::dummy(), seed_device_port_schemas)
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    let registry = Rc::new(HostDeviceRegistry::new());
    register_python_devices(py, &registry)?;
    let mut session = Session::builder(&design, top)
        .provider(registry)
        .compile()
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    let op = session.op(&SolverConfig::default(), None).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    op.v(NetRef { name: net.to_string() }).map_err(|e| PyRuntimeError::new_err(e.to_string()))
}
