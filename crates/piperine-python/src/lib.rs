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
mod results;
mod value_bridge;

use pyo3::prelude::*;

use design::{_Design, _Node, _Selection};
use module::_Module;
use results::_AcTrace;
use results::_NoiseTrace;
use results::_OpResult;
use results::_Trace;

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
    m.add_class::<_Selection>()?;
    m.add_class::<_Node>()?;
    m.add_class::<_OpResult>()?;
    m.add_class::<_Trace>()?;
    m.add_class::<_AcTrace>()?;
    m.add_class::<_NoiseTrace>()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::_piperine;
    use pyo3::prelude::*;
    use pyo3::types::PyModule;

    /// A tiny self-contained PHDL (declares its own discipline + two modules,
    /// no `use`/prelude dependency) — the P3/P4 reflection fixture. Resistor
    /// carries an `analog` body so behavior reflection is observable.
    const PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod Resistor(inout p: Electrical, inout n: Electrical) {
    param r: Real = 1e3;
}

analog Resistor {
    I(p, n) <+ V(p, n) / r;
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

    /// Build the in-process `_piperine` module under the active interpreter,
    /// load the reflection PHDL, and return the loaded `_Design`.
    fn loaded_design<'py>(py: Python<'py>, path_str: &str) -> PyResult<Bound<'py, PyAny>> {
        let m = PyModule::new(py, "_piperine")?;
        _piperine(&m)?;
        m.getattr("load")?.call1((path_str,))
    }

    /// Sorted list of an iterable's `.name` attribute (objects expose `name`
    /// as a `#[getter]`).
    fn sorted_names(list: Bound<'_, PyAny>) -> PyResult<Vec<String>> {
        let mut names: Vec<String> = list
            .try_iter()?
            .map(|item| {
                let item: Bound<'_, PyAny> = item?;
                item.getattr("name")?.extract::<String>()
            })
            .collect::<PyResult<Vec<String>>>()?;
        names.sort();
        Ok(names)
    }

    /// `(name, module)` pairs for an iterable of `_Instance`.
    fn instance_pairs(list: Bound<'_, PyAny>) -> PyResult<Vec<(String, String)>> {
        list.try_iter()?
            .map(|item| {
                let it: Bound<'_, PyAny> = item?;
                let name = it.getattr("name")?.extract::<String>()?;
                let module = it.getattr("module")?.extract::<String>()?;
                Ok((name, module))
            })
            .collect()
    }

    /// PY-03 / spec AC14: a module reflects its ports, nets, instances,
    /// params, and behaviors as typed lists with their attributes.
    #[test]
    fn module_reflects_structure() -> PyResult<()> {
        let path = std::env::temp_dir().join("piperine_python_p4_reflect_test.phdl");
        std::fs::write(&path, PHDL)?;
        let path_str = path
            .to_str()
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("non-utf8 temp path"))?;

        let outcome = Python::with_gil(|py| -> PyResult<()> {
            let design = loaded_design(py, path_str)?;

            // DividerBoard: 3 nets (gnd, vin, mid), 2 instances (r_bot, r_top),
            // and no ports/params/behaviors of its own.
            let board = design.getattr("module")?.call1(("DividerBoard",))?;
            assert_eq!(
                sorted_names(board.getattr("nets")?.call0()?)?,
                vec!["gnd", "mid", "vin"]
            );
            let pairs = instance_pairs(board.getattr("instances")?.call0()?)?;
            assert_eq!(pairs.len(), 2);
            assert!(pairs.contains(&("r_top".into(), "Resistor".into())));
            assert!(pairs.contains(&("r_bot".into(), "Resistor".into())));
            assert!(board.getattr("ports")?.call0()?.try_iter()?.next().is_none());
            assert!(board.getattr("params")?.call0()?.try_iter()?.next().is_none());
            assert!(board.getattr("behaviors")?.call0()?.try_iter()?.next().is_none());

            // Each net carries its discipline type.
            for item in board.getattr("nets")?.call0()?.try_iter()? {
                let net: Bound<'_, PyAny> = item?;
                assert_eq!(net.getattr("ty")?.extract::<String>()?, "Electrical");
            }

            // Resistor: ports (n, p) both `inout : Electrical`, one param `r`
            // (Real, default 1e3), one `analog` behavior.
            let resistor = design.getattr("module")?.call1(("Resistor",))?;
            assert_eq!(
                sorted_names(resistor.getattr("ports")?.call0()?)?,
                vec!["n", "p"]
            );
            for item in resistor.getattr("ports")?.call0()?.try_iter()? {
                let port: Bound<'_, PyAny> = item?;
                assert_eq!(port.getattr("direction")?.extract::<String>()?, "inout");
                assert_eq!(port.getattr("ty")?.extract::<String>()?, "Electrical");
            }
            let params: Vec<Bound<'_, PyAny>> = resistor
                .getattr("params")?
                .call0()?
                .try_iter()?
                .map(|p| Ok::<Bound<'_, PyAny>, PyErr>(p?))
                .collect::<PyResult<Vec<_>>>()?;
            assert_eq!(params.len(), 1, "Resistor has one param");
            assert_eq!(params[0].getattr("name")?.extract::<String>()?, "r");
            assert_eq!(params[0].getattr("ty")?.extract::<String>()?, "Real");
            assert!((params[0].getattr("default")?.extract::<f64>()? - 1e3).abs() < 1e-6);

            let behaviors: Vec<Bound<'_, PyAny>> = resistor
                .getattr("behaviors")?
                .call0()?
                .try_iter()?
                .map(|b| Ok::<Bound<'_, PyAny>, PyErr>(b?))
                .collect::<PyResult<Vec<_>>>()?;
            assert_eq!(behaviors.len(), 1, "Resistor has one behavior");
            assert_eq!(behaviors[0].getattr("kind")?.extract::<String>()?, "analog");
            Ok(())
        });
        let _ = std::fs::remove_file(&path);
        outcome
    }

    /// PY-14 / spec AC15: `design.select(path)` resolves a hierarchical
    /// selector path to a typed node selection; an unresolved path raises
    /// (fail loud, never an empty-success per spec edge cases).
    ///
    /// The POM selector grammar uses `/`-separated steps with optional
    /// `axis::name` segments (e.g. `/r_top/port::p`): `/r_top` matches the
    /// `r_top` instance under the (inferred) top module; `port::p` walks
    /// that instance's module ports and filters by name `p`. The spec's
    /// dot-notation examples (`"buck.r1.p"`) are an imprecision — the
    /// actual selector grammar (parse.rs) does not accept `.`.
    #[test]
    fn select_resolves_path_and_errors_on_miss() -> PyResult<()> {
        let path = std::env::temp_dir().join("piperine_python_p5_select_test.phdl");
        std::fs::write(&path, PHDL)?;
        let path_str = path
            .to_str()
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("non-utf8 temp path"))?;

        let outcome = Python::with_gil(|py| -> PyResult<()> {
            let design = loaded_design(py, path_str)?;

            // One-step path: `/r_top` resolves to the labelled `r_top`
            // instance under the inferred top module (DividerBoard).
            let sel = design.getattr("select")?.call1(("/r_top",))?;
            assert_eq!(sel.getattr("len")?.call0()?.extract::<usize>()?, 1);
            let nodes: Vec<Bound<'_, PyAny>> = sel
                .getattr("nodes")?
                .call0()?
                .try_iter()?
                .map(|n| Ok::<Bound<'_, PyAny>, PyErr>(n?))
                .collect::<PyResult<Vec<_>>>()?;
            assert_eq!(nodes.len(), 1);
            assert_eq!(nodes[0].getattr("kind")?.extract::<String>()?, "instance");
            assert_eq!(nodes[0].getattr("name")?.extract::<String>()?, "r_top");

            // Two-step path: `/r_top/port::p` descends into the instance's
            // module (Resistor) and resolves port `p`.
            let port_sel = design.getattr("select")?.call1(("/r_top/port::p",))?;
            let port_nodes: Vec<Bound<'_, PyAny>> = port_sel
                .getattr("nodes")?
                .call0()?
                .try_iter()?
                .map(|n| Ok::<Bound<'_, PyAny>, PyErr>(n?))
                .collect::<PyResult<Vec<_>>>()?;
            assert_eq!(port_nodes.len(), 1);
            assert_eq!(port_nodes[0].getattr("kind")?.extract::<String>()?, "port");
            assert_eq!(port_nodes[0].getattr("name")?.extract::<String>()?, "p");

            // Unresolved path → KeyError (fail loud, spec edge case).
            let miss = design
                .getattr("select")?
                .call1(("/does_not_exist",))
                .unwrap_err();
            assert!(
                miss.is_instance_of::<pyo3::exceptions::PyKeyError>(py),
                "unresolved select must raise KeyError, got {miss}"
            );

            // Malformed path → ValueError (parse failure surfaced loudly).
            let bad = design.getattr("select")?.call1(("not:::valid",)).unwrap_err();
            assert!(
                bad.is_instance_of::<pyo3::exceptions::PyValueError>(py),
                "malformed select must raise ValueError, got {bad}"
            );
            Ok(())
        });
        let _ = std::fs::remove_file(&path);
        outcome
    }

    /// A runnable fixture for the analysis tests: a 5 V source driving a
    /// 3 kΩ/2 kΩ resistor divider, so the `mid` node sits at
    /// 5·2/(3+2) = 2.0 V (spec-defined outcome the stage test asserts).
    /// Staging `r_top.r = 2e3` moves `mid` to 5·2/(2+2) = 2.5 V (spec AC12).
    /// Mirrors the bench's own divider circuit shape — the uniform-host proof.
    const ANALYSIS_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod VoltageSource(inout p: Electrical, inout n: Electrical) {
    param voltage: Real = 0.0;
}
analog VoltageSource { V(p, n) <- voltage; }

mod Resistor(inout p: Electrical, inout n: Electrical) {
    param r: Real = 1e3;
}
analog Resistor { I(p, n) <+ V(p, n) / r; }

mod Divider() {
    wire gnd  : Electrical;
    wire vin  : Electrical;
    wire mid  : Electrical;
    src   : VoltageSource (.p = vin, .n = gnd) { .voltage = 5.0 };
    r_top : Resistor      (.p = vin, .n = mid) { .r = 3e3 };
    r_bot : Resistor      (.p = mid, .n = gnd) { .r = 2e3 };
}
";

    /// PY-04 / spec AC3/6/8/9: `module.op/tran/ac/noise` each return the
    /// right typed result object. The Python-side `.v(net)` is P7, so the
    /// analysis shape is checked by type name — the four result pyclasses
    /// exist and are returned (fail loud if any analysis path is unwired).
    #[test]
    fn analyses_return_typed_results() -> PyResult<()> {
        let path = std::env::temp_dir().join("piperine_python_p6_analyses_test.phdl");
        std::fs::write(&path, ANALYSIS_PHDL)?;
        let path_str = path
            .to_str()
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("non-utf8 temp path"))?;

        let outcome = Python::with_gil(|py| -> PyResult<()> {
            let design = loaded_design(py, path_str)?;
            let module = design.getattr("module")?.call1(("Divider",))?;

            let op = module.getattr("op")?.call0()?;
            assert_eq!(
                op.getattr("__class__")?.getattr("__name__")?.extract::<String>()?,
                "_OpResult",
                "op() must return an _OpResult"
            );

            let tran = module.getattr("tran")?.call1((5e-3, 1e-5))?;
            assert_eq!(
                tran.getattr("__class__")?.getattr("__name__")?.extract::<String>()?,
                "_Trace",
                "tran() must return a _Trace"
            );

            let ac = module.getattr("ac")?.call1((1.0, 1e6, 10))?;
            assert_eq!(
                ac.getattr("__class__")?.getattr("__name__")?.extract::<String>()?,
                "_AcTrace",
                "ac() must return an _AcTrace"
            );

            let noise = module.getattr("noise")?.call1(("mid", 1.0, 1e6, 5))?;
            assert_eq!(
                noise.getattr("__class__")?.getattr("__name__")?.extract::<String>()?,
                "_NoiseTrace",
                "noise() must return a _NoiseTrace"
            );
            Ok(())
        });
        let _ = std::fs::remove_file(&path);
        outcome
    }

    /// PY-12 / spec AC11/12: `stage(label, param, value)` overrides the next
    /// analysis; staging is pure (the held `_Design` is not mutated). The
    /// Python `.v()` lands in P7, so the stage effect is read through the
    /// bench's own typed `OpResult::v` readout (uniform-shape proof — same
    /// call the bench makes) by extracting the inner result from the
    /// returned `_OpResult`.
    ///
    /// Divider math: `mid = 5·r_bot/(r_top+r_bot)`. Default 3 k/2 k → 2.0 V;
    /// staging `r_top.r = 2e3` → 2 k/2 k → 2.5 V. The default-vs-staged
    /// delta (0.5 V) is the spec-defined outcome (AC12: "each result SHALL
    /// reflect that iteration's staged value").
    #[test]
    fn stage_overrides_next_analysis() -> PyResult<()> {
        use pyo3::types::PyAnyMethods;
        use piperine_bench::{NetRef, OpResult as BenchOpResult};

        let path = std::env::temp_dir().join("piperine_python_p6_stage_test.phdl");
        std::fs::write(&path, ANALYSIS_PHDL)?;
        let path_str = path
            .to_str()
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("non-utf8 temp path"))?;

        let outcome = Python::with_gil(|py| -> PyResult<()> {
            let design = loaded_design(py, path_str)?;
            let module = design.getattr("module")?.call1(("Divider",))?;

            // Helper: run op() and read `mid` through the bench readout.
            let mid_voltage = |module: &Bound<'_, PyAny>| -> PyResult<f64> {
                let op_obj = module.getattr("op")?.call0()?;
                let pyref = op_obj.extract::<pyo3::PyRef<'_, super::_OpResult>>()?;
                let mid_ref = NetRef {
                    name: "mid".to_string(),
                };
                let v = BenchOpResult::v(&pyref.inner, &mid_ref, None)
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{e}")))?;
                Ok(v)
            };

            // Default divider: mid = 5 · 2/(3+2) = 2.0 V (spec-defined).
            let v_default = mid_voltage(&module)?;
            assert!(
                (v_default - 2.0).abs() < 1e-6,
                "default mid voltage should be 2.0 V, got {v_default}"
            );

            // Stage r_top.r = 2e3 → mid = 5 · 2/(2+2) = 2.5 V (spec AC12).
            module.getattr("stage")?.call1(("r_top", "r", 2e3))?;
            let v_staged = mid_voltage(&module)?;
            assert!(
                (v_staged - 2.5).abs() < 1e-6,
                "staged mid voltage should be 2.5 V, got {v_staged}"
            );

            // Staging is pure: the held _Design's reflection is unchanged
            // (no structural mutation, AC11). Re-loading and re-running op
            // without staging returns the default 2.0 V — the stage did not
            // leak into the parent design.
            let fresh = loaded_design(py, path_str)?;
            let fresh_module = fresh.getattr("module")?.call1(("Divider",))?;
            let v_fresh = {
                let op_obj = fresh_module.getattr("op")?.call0()?;
                let pyref = op_obj.extract::<pyo3::PyRef<'_, super::_OpResult>>()?;
                let mid_ref = NetRef {
                    name: "mid".to_string(),
                };
                BenchOpResult::v(&pyref.inner, &mid_ref, None)
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{e}")))?
            };
            assert!(
                (v_fresh - 2.0).abs() < 1e-6,
                "staging must not leak: a fresh load's mid should still be 2.0 V, got {v_fresh}"
            );
            Ok(())
        });
        let _ = std::fs::remove_file(&path);
        outcome
    }
}
