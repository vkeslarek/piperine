//! Bridge the POM value layer (`piperine_lang::Value`) to Python objects.

use pyo3::prelude::*;
use pyo3::types::PyBool;
use pyo3::IntoPyObject;

use piperine_lang::Value;

/// By-reference adapter that converts a POM [`Value`] into its Python
/// equivalent. Used by `_Design::const_` (PY-02) and `_Param.default`
/// (PY-03). Scalar kinds become native Python scalars; collection/object
/// kinds fall back to their string form; `Unit` becomes `None`.
pub(crate) struct PyValue<'a>(pub &'a Value);

impl<'a> PyValue<'a> {
    /// Convert the held value into a fresh Python object.
    pub(crate) fn to_object(&self, py: Python<'_>) -> PyResult<PyObject> {
        let obj: Bound<'_, PyAny> = match self.0 {
            Value::Real(v) => (*v).into_pyobject(py)?.into_any(),
            Value::Int(v) => (*v).into_pyobject(py)?.into_any(),
            Value::Nat(v) => (*v).into_pyobject(py)?.into_any(),
            Value::Str(v) => v.clone().into_pyobject(py)?.into_any(),
            Value::Bool(v) => {
                // Python `bool` is a singleton; widen via the owned clone.
                let singleton = (*v).into_pyobject(py)?;
                <Bound<'_, PyBool> as Clone>::clone(&singleton).into_any()
            }
            Value::Unit => return Ok(py.None()),
            other => other.to_string().into_pyobject(py)?.into_any(),
        };
        Ok(obj.unbind())
    }
}
