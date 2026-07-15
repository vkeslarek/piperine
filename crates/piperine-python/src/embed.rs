//! Embedded CPython execution for `piperine run script.py` (PY-15 / spec
//! AC16/17). Registers the native `_piperine` extension + the typed
//! pure-Python facade `piperine` in the interpreter, then runs the user's
//! script with `import piperine` available — no `pip install` required.
//!
//! Uniform shape (PY-17): the facade is the same `piperine/__init__.py` the
//! wheel ships; embedding just materializes it from `include_str!` instead of
//! from disk. The script sees the identical public surface either way.

use std::ffi::CString;

use pyo3::prelude::*;
use pyo3::types::PyModule;

// Bring the `#[pymodule]` initializer into scope — `run_script` builds the
// native module in-process by calling it directly (see registration note
// below).
use crate::_piperine;

/// The typed pure-Python facade, embedded at compile time. Materialized as
/// the `piperine` module in the embedded interpreter so `import piperine`
/// resolves without a pip install (spec AC16).
const FACADE_SRC: &str = include_str!("../python/piperine/__init__.py");

/// Embedded-interpreter runner (PY-15). Registers `_piperine` + the
/// `piperine` facade in `sys.modules`, then runs the Python script at
/// `path`. `import piperine` works with no pip install (spec AC16). A
/// Python exception propagates as a `PyErr` — the CLI surfaces it to
/// stderr + non-zero exit (spec AC17).
///
/// **Registration:** the design proposed `append_to_inittab` +
/// `prepare_freethreaded_python`. PyO3's `auto-initialize` feature
/// (required for the test suite's `Python::with_gil`) initializes the
/// interpreter before `append_to_inittab` can run, so we register
/// `_piperine` directly in `sys.modules` instead — functionally equivalent
/// for `import _piperine`, and it works whether or not the interpreter is
/// pre-initialized. The CLI path calls this exactly once at the top of
/// `run`.
// SPEC_DEVIATION: design proposed append_to_inittab + prepare_freethreaded_python
// Reason: pyo3's auto-initialize feature inits the interpreter before
// append_to_inittab can run; sys.modules registration is functionally
// equivalent and works in both CLI (fresh) and test (pre-initialized) paths.
pub fn run_script(path: &str) -> PyResult<()> {
    Python::with_gil(|py| {
        // 1. Build + register `_piperine` in sys.modules so the facade's
        //    `import _piperine` resolves (no pip install, spec AC16).
        let native = PyModule::new(py, "_piperine")?;
        _piperine(&native)?;
        let modules = py.import("sys")?.getattr("modules")?;
        modules.set_item("_piperine", &native)?;

        // 2. Materialize the facade as `piperine` and register it in
        //    sys.modules so the user's `import piperine` resolves. The
        //    facade re-exports the native classes + adds the config
        //    dataclasses (uniform surface, PY-16).
        let facade_src = CString::new(FACADE_SRC)
            .map_err(|_| pyo3::exceptions::PyValueError::new_err("facade source contains nul bytes"))?;
        let facade = PyModule::from_code(py, &facade_src, c"piperine/__init__.py", c"piperine")?;
        modules.set_item("piperine", facade)?;

        // 3. Read + run the user's script. A Python exception propagates as
        //    a `PyErr` (spec AC17 — fail loud, no silent swallow).
        let script = std::fs::read_to_string(path).map_err(|e| {
            pyo3::exceptions::PyValueError::new_err(format!("failed to read `{path}`: {e}"))
        })?;
        let script_cstr = CString::new(script)
            .map_err(|_| pyo3::exceptions::PyValueError::new_err("script contains nul bytes"))?;
        py.run(&script_cstr, None, None)?;
        Ok(())
    })
}
