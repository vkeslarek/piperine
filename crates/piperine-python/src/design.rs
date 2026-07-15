//! `_Design` — the loaded, elaborated POM root exposed to Python
//! (PY-01 load, PY-02 reflection).

use std::path::Path;
use std::rc::Rc;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use piperine_lang::{parse_and_elaborate, Design, SourceMap};

use crate::module::_Module;
use crate::value_bridge::PyValue;

/// `_Design` — a loaded, elaborated POM design. Owns a shared (refcounted)
/// `Design` so child `_Module` views can re-look it up on each call without
/// FFI lifetime fights (design `python-bindings/design.md` — POM borrow-
/// lifetime risk). The Python facade re-exports this as `Design`.
///
/// `unsendable`: `Design` carries `Rc<RefCell<…>>` internally (the staging
/// area), so it is not `Sync`; the binding is single-interpreter, so the
/// `unsendable` pyclass (usable only from the interpreter's thread) is the
/// honest fit.
#[pyclass(module = "piperine", unsendable)]
pub struct _Design {
    design: Rc<Design>,
}

impl _Design {
    /// Load + elaborate the PHDL at `path` into a `_Design` (PY-01).
    ///
    /// The `SourceMap` is project-aware: when a `Piperine.toml` root is found
    /// above `path`, dependency namespaces + the prelude resolve as the CLI
    /// resolves them; otherwise a dummy map is used (self-contained designs
    /// still elaborate). Parse/elaboration failures surface as `ValueError`
    /// carrying the diagnostic; a missing/unreadable file surfaces the same way.
    pub(crate) fn load(path: &str) -> PyResult<Self> {
        let source = std::fs::read_to_string(path)
            .map_err(|e| PyValueError::new_err(format!("failed to read `{path}`: {e}")))?;
        let source_map = match Path::new(path)
            .parent()
            .and_then(piperine_project::find_project_root)
        {
            Some(root) => piperine_project::project_source_map(&root),
            None => SourceMap::dummy(),
        };
        let design = parse_and_elaborate(&source, &source_map)
            .map_err(|e| PyValueError::new_err(format!("{e}")))?;
        Ok(Self {
            design: Rc::new(design),
        })
    }

    /// A shared handle to the underlying POM — `_Module` borrows it per call.
    pub(crate) fn shared(&self) -> Rc<Design> {
        Rc::clone(&self.design)
    }
}

#[pymethods]
impl _Design {
    /// The elaborated top module, if one is set (PY-02).
    fn top(&self) -> Option<_Module> {
        self.design
            .top()
            .map(|m| _Module::new(self.shared(), m.name().to_string()))
    }

    /// Look up a module by name; raises `ValueError` if absent (PY-02).
    fn module(&self, name: &str) -> PyResult<_Module> {
        if self.design.module(name).is_some() {
            Ok(_Module::new(self.shared(), name.to_string()))
        } else {
            Err(PyValueError::new_err(format!("module `{name}` not found")))
        }
    }

    /// Every elaborated module (PY-02).
    fn modules(&self) -> Vec<_Module> {
        self.design
            .modules()
            .map(|m| _Module::new(self.shared(), m.name().to_string()))
            .collect()
    }

    /// A global constant by name — scalars map to native Python values, other
    /// value kinds fall back to their string form, and an unknown name yields
    /// `None`. Read-only reflection starter (PY-02).
    fn const_(&self, py: Python<'_>, name: &str) -> PyResult<PyObject> {
        match self.design.const_(name) {
            Some(value) => PyValue(value).to_object(py),
            None => Ok(py.None()),
        }
    }
}
