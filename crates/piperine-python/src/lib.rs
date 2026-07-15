//! # piperine-python
//!
//! Native PyO3 extension (`_piperine`) that exposes the Piperine bench + POM
//! surface to Python — spec §10 "the uniform host-neutral API". A typed
//! pure-Python facade (`piperine/__init__.py`, landed later) re-exports these
//! native types under idiomatic, annotated aliases; the facade is the public
//! surface, this crate is the engine under it.
//!
//! ## Dual build (design `python-bindings/design.md` — PyO3 dual-build risk)
//!
//! One Cargo feature, [`Self::extension-module`], selects how libpython is
//! linked:
//! - **OFF (default)** — `rlib` linked into the CLI's embedded interpreter
//!   (`piperine run script.py`) plus the test suite. PyO3 links libpython
//!   normally and `auto-initialize` spins up an interpreter on first use.
//! - **ON** — `cdylib` for the importable `_piperine.so` (the maturin wheel);
//!   libpython is provided by the host `python` so the `.so` does not link it.
//!
//! ## MD-13 note
//!
//! PyO3's `#[pymodule]`/`#[pyclass]`/`#[pyfunction]` attribute macros are
//! mandated by the framework (an external dependency, not hand-rolled codegen);
//! every function body still delegates to a struct method so no *logic* lives
//! as a loose module-level function.

use pyo3::exceptions::PyNotImplementedError;
use pyo3::prelude::*;

/// `_Design` — a loaded, elaborated POM design (PY-01/02). The Python facade
/// re-exports this as `Design`. Scaffold only for now; reflection (top/module/
/// modules/const_) lands in P3.
#[pyclass(module = "piperine")]
pub struct _Design;

impl _Design {
    /// Load + elaborate `path` into a `_Design` (PY-01). Implemented in P3;
    /// this stub fails loud so the scaffold never silently returns empty data.
    fn load(_path: &str) -> PyResult<Self> {
        Err(PyNotImplementedError::new_err(
            "_piperine.load() is not implemented yet (lands in P3)",
        ))
    }
}

/// `_piperine.load(path) -> _Design` (PY-01). Thin FFI shim delegating to
/// [`_Design::load`].
#[pyfunction]
fn load(path: &str) -> PyResult<_Design> {
    _Design::load(path)
}

/// The `_piperine` native extension module. Registered by the facade and, for
/// `piperine run`, appended to the embedded interpreter's init table.
#[pymodule]
fn _piperine(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(load, m)?)?;
    m.add_class::<_Design>()?;
    Ok(())
}
