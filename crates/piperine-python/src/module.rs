//! `_Module` — a read-only reflected view of one POM module (PY-03).

use std::rc::Rc;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use piperine_lang::Design;

/// `_Module` — a reflected view of a named module in a shared [`Design`].
/// Stores `(Rc<Design>, name)` and re-looks the module up on each call so the
/// GIL-bound lifetime never fights the POM borrow (design
/// `python-bindings/design.md` — POM borrow-lifetime risk). The full
/// reflection surface (ports/nets/instances/params/behaviors) lands in P4; P3
/// exposes the identity only.
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
}

#[pymethods]
impl _Module {
    /// The module's declared name (re-resolved against the live POM). A
    /// property — `module.name` — since it is an attribute, not an action.
    #[getter]
    fn name(&self) -> PyResult<String> {
        let m = self.design.module(&self.name).ok_or_else(|| {
            PyValueError::new_err(format!("module `{}` is no longer present", self.name))
        })?;
        Ok(m.name().to_string())
    }
}
