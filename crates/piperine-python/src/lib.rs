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

mod design;
mod module;

use pyo3::prelude::*;

use design::_Design;
use module::_Module;

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
    m.add_class::<_Module>()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::_piperine;
    use pyo3::prelude::*;
    use pyo3::types::PyModule;

    /// A tiny self-contained PHDL (declares its own discipline + two modules,
    /// no `use`/prelude dependency) — the P3 reflection fixture.
    const PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod Resistor(inout p: Electrical, inout n: Electrical) {
    param r: Real = 1e3;
}

mod DividerBoard() {
    wire gnd : Electrical;
    wire vin : Electrical;
    wire mid : Electrical;
    r_top : Resistor(.p = vin, .n = mid) { .r = 3e3 };
    r_bot : Resistor(.p = mid, .n = gnd) { .r = 2e3 };
}
";

    /// PY-01/02: `load` returns a `_Design` whose `modules()` lists every
    /// elaborated module; `module(name)` returns that module and raises when
    /// the name is unknown.
    #[test]
    fn load_returns_reflected_design() -> PyResult<()> {
        let path = std::env::temp_dir().join("piperine_python_p3_load_test.phdl");
        std::fs::write(&path, PHDL)?;
        let path_str = path
            .to_str()
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("non-utf8 temp path"))?;

        let outcome = Python::with_gil(|py| -> PyResult<()> {
            let m = PyModule::new(py, "_piperine")?;
            _piperine(&m)?;

            let design = m.getattr("load")?.call1((path_str,))?;

            // Spec edge case: a nonexistent path raises (FileNotFoundError /
            // ValueError), never a silent success.
            assert!(
                m.getattr("load")?
                    .call1(("/nonexistent/piperine_missing.phdl",))
                    .is_err(),
                "loading a missing file must raise"
            );

            // modules() lists every elaborated module.
            let modules = design.getattr("modules")?.call0()?;
            let mut names: Vec<String> = modules
                .try_iter()?
                .map(|item| Ok::<String, PyErr>(item?.getattr("name")?.extract::<String>()?))
                .collect::<PyResult<Vec<String>>>()?;
            names.sort();
            assert!(
                names.contains(&"Resistor".to_string()),
                "Resistor should be reflected, got {names:?}"
            );
            assert!(
                names.contains(&"DividerBoard".to_string()),
                "DividerBoard should be reflected, got {names:?}"
            );

            // module(name) returns the named module; missing → raises.
            let r = design
                .getattr("module")?
                .call1(("Resistor",))?
                .getattr("name")?
                .extract::<String>()?;
            assert_eq!(r, "Resistor");
            assert!(
                design.getattr("module")?.call1(("DoesNotExist",)).is_err(),
                "looking up a missing module must raise"
            );
            Ok(())
        });
        let _ = std::fs::remove_file(&path);
        outcome
    }
}
