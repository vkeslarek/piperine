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

pub mod embed;
mod design;
mod instance;
mod live;
mod module;
mod results;
mod value_bridge;

use pyo3::prelude::*;

use design::{_Design, _Node, _Selection};
use instance::{_InstanceView, _Terminal};
use live::_LiveSession;
use module::_Module;
use module::{_Behavior, _Instance, _Net, _Param, _Port};
use results::_AcTrace;
use results::_ComplexWaveform;
use results::_NoiseTrace;
use results::_SolverStats;
use results::_OpResult;
use results::_Trace;
use results::_Waveform;

/// `_piperine.load(path) -> _Design` (PY-01). Thin FFI shim delegating to
/// [`_Design::load`].
#[pyfunction]
fn load(path: &str) -> PyResult<_Design> {
    _Design::load(path)
}

/// The `_piperine` native extension module. Registered by the facade and, for
/// `piperine run`, appended to the embedded interpreter's init table
/// ([`embed::run_script`], PY-15).
#[pymodule]
pub(crate) fn _piperine(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(load, m)?)?;
    m.add_class::<_Design>()?;
    m.add_class::<_Module>()?;
    m.add_class::<_Port>()?;
    m.add_class::<_Net>()?;
    m.add_class::<_Instance>()?;
    m.add_class::<_Param>()?;
    m.add_class::<_Behavior>()?;
    m.add_class::<_Selection>()?;
    m.add_class::<_Node>()?;
    m.add_class::<_OpResult>()?;
    m.add_class::<_Trace>()?;
    m.add_class::<_Waveform>()?;
    m.add_class::<_ComplexWaveform>()?;
    m.add_class::<_AcTrace>()?;
    m.add_class::<_NoiseTrace>()?;
    m.add_class::<_SolverStats>()?;
    m.add_class::<_InstanceView>()?;
    m.add_class::<_Terminal>()?;
    m.add_class::<_LiveSession>()?;
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
        use piperine::{NetRef, OpResult as HostOpResult};

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
                // `inner` is `Rc<OpResult>`; deref through Rc to call `v`.
                let v = HostOpResult::v(&*pyref.inner, &mid_ref, None)
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
                HostOpResult::v(&*pyref.inner, &mid_ref, None)
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

    /// PY-06 / spec AC4/5: `OpResult.v(net)` returns the node voltage as a
    /// float; `.v(a, b)` returns the differential `a - b`; `.i(a, b)` returns
    /// the branch current from `a` to `b`. `op["net"]` SHALL equal
    /// `op.v("net")` (AC5). An unknown net raises `KeyError` (fail loud, spec
    /// edge case).
    ///
    /// Divider math (ANALYSIS_PHDL): vin = 5 V driven through r_top = 3 kΩ
    /// into r_bot = 2 kΩ to gnd → mid = 5·2/(3+2) = 2.0 V. So `v(mid)=2.0`,
    /// `v(vin, mid) = 3.0` (drop across r_top), and `i(vin, mid) = 1 mA`
    /// (current through r_top, vin→mid).
    #[test]
    fn op_result_reads_voltages_and_currents() -> PyResult<()> {
        let path = std::env::temp_dir().join("piperine_python_p7_op_test.phdl");
        std::fs::write(&path, ANALYSIS_PHDL)?;
        let path_str = path
            .to_str()
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("non-utf8 temp path"))?;

        let outcome = Python::with_gil(|py| -> PyResult<()> {
            let design = loaded_design(py, path_str)?;
            let module = design.getattr("module")?.call1(("Divider",))?;
            let op = module.getattr("op")?.call0()?;

            // AC4: .v(net) returns the node voltage (float).
            let v_mid = op.getattr("v")?.call1(("mid",))?.extract::<f64>()?;
            assert!(
                (v_mid - 2.0).abs() < 1e-6,
                "op.v(mid) should be 2.0 V, got {v_mid}"
            );
            let v_vin = op.getattr("v")?.call1(("vin",))?.extract::<f64>()?;
            assert!(
                (v_vin - 5.0).abs() < 1e-6,
                "op.v(vin) should be 5.0 V, got {v_vin}"
            );
            let v_gnd = op.getattr("v")?.call1(("gnd",))?.extract::<f64>()?;
            assert!(v_gnd.abs() < 1e-9, "op.v(gnd) should be 0.0 V, got {v_gnd}");

            // AC4: .v(a, b) returns the differential a - b.
            let v_diff = op.getattr("v")?.call1(("vin", "mid"))?.extract::<f64>()?;
            assert!(
                (v_diff - 3.0).abs() < 1e-6,
                "op.v(vin, mid) should be 3.0 V, got {v_diff}"
            );

            // AC4: .i(a, b) returns the branch current from a to b.
            let i_rtop = op.getattr("i")?.call1(("vin", "mid"))?.extract::<f64>()?;
            assert!(
                (i_rtop - 1e-3).abs() < 1e-9,
                "op.i(vin, mid) should be 1 mA through r_top, got {i_rtop}"
            );

            // AC5: op["net"] == op.v("net").
            let item_mid = op.getattr("__getitem__")?.call1(("mid",))?.extract::<f64>()?;
            assert!(
                (item_mid - v_mid).abs() < 1e-12,
                "op['mid'] should equal op.v('mid'), got {item_mid} vs {v_mid}"
            );

            // Spec edge case: an unknown net raises KeyError (fail loud).
            let miss = op.getattr("v")?.call1(("does_not_exist",)).unwrap_err();
            assert!(
                miss.is_instance_of::<pyo3::exceptions::PyKeyError>(py),
                "unknown net must raise KeyError, got {miss}"
            );
            let miss_item = op.getattr("__getitem__")?.call1(("nope",)).unwrap_err();
            assert!(
                miss_item.is_instance_of::<pyo3::exceptions::PyKeyError>(py),
                "op['nope'] must raise KeyError, got {miss_item}"
            );
            Ok(())
        });
        let _ = std::fs::remove_file(&path);
        outcome
    }

    /// CP-09 / spec: SolverStats exposed via `op.stats` / `trace.stats`.
    /// The stats carry per-analysis convergence + performance diagnostics
    /// (Newton iterations, step counts, dt range). On the divider (3 k/2 k,
    /// 5 V → mid = 2 V), Newton converges in ≥1 iteration, and a tran records
    /// ≥1 accepted step with a non-zero dt_max.
    #[test]
    fn stats_exposed_on_results() -> PyResult<()> {
        let path = std::env::temp_dir().join("piperine_python_stats_test.phdl");
        std::fs::write(&path, ANALYSIS_PHDL)?;
        let path_str = path
            .to_str()
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("non-utf8 temp path"))?;

        let outcome = Python::with_gil(|py| -> PyResult<()> {
            let design = loaded_design(py, path_str)?;
            let module = design.getattr("module")?.call1(("Divider",))?;

            // op.stats.newton_iterations > 0 (DC converged in ≥1 iteration).
            let op = module.getattr("op")?.call0()?;
            let stats = op.getattr("stats")?;
            let newton_iters = stats.getattr("newton_iterations")?.extract::<usize>()?;
            assert!(
                newton_iters > 0,
                "op.stats.newton_iterations should be > 0, got {newton_iters}"
            );
            let converged = stats.getattr("converged")?.extract::<bool>()?;
            assert!(converged, "op.stats.converged should be true");

            // trace.stats.steps_accepted > 0 (tran ran ≥1 step).
            let trace = module.getattr("tran")?.call1((5e-3, 1e-5))?;
            let tstats = trace.getattr("stats")?;
            let steps = tstats.getattr("steps_accepted")?.extract::<usize>()?;
            assert!(
                steps > 0,
                "trace.stats.steps_accepted should be > 0, got {steps}"
            );
            let dt_max = tstats.getattr("dt_max")?.extract::<f64>()?;
            assert!(dt_max > 0.0, "trace.stats.dt_max should be > 0, got {dt_max}");
            Ok(())
        });
        let _ = std::fs::remove_file(&path);
        outcome
    }

    /// Solver-config threading: the facade's `Solver` dataclass reaches the
    /// Newton loop (duck-typed attribute read — any object with the prelude
    /// `bundle Solver` fields works). `max_iter = 1` must fail loud on a
    /// circuit whose damped Newton needs several iterations; the defaults
    /// must converge. Also: `op(nodeset=...)` is accepted (seeds the guess).
    #[test]
    fn solver_config_reaches_newton() -> PyResult<()> {
        let path = std::env::temp_dir().join("piperine_python_solvercfg_test.phdl");
        std::fs::write(&path, ANALYSIS_PHDL)?;
        let path_str = path
            .to_str()
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("non-utf8 temp path"))?;

        let outcome = Python::with_gil(|py| -> PyResult<()> {
            let design = loaded_design(py, path_str)?;
            let module = design.getattr("module")?.call1(("Divider",))?;

            let ns = py.import("types")?.getattr("SimpleNamespace")?;
            let kwargs = pyo3::types::PyDict::new(py);
            kwargs.set_item("temperature", 300.15)?;
            kwargs.set_item("reltol", 1e-3)?;
            kwargs.set_item("abstol", 1e-12)?;
            kwargs.set_item("gmin", 1e-12)?;
            kwargs.set_item("max_iter", 1usize)?;
            let starved = ns.call((), Some(&kwargs))?;

            // max_iter = 1 starves Newton (and every homotopy fallback).
            assert!(
                module.getattr("op")?.call1((py.None(), starved)).is_err(),
                "op with max_iter=1 must fail loud"
            );

            // Defaults (solver = None) converge; nodeset is accepted.
            let nodeset = pyo3::types::PyDict::new(py);
            nodeset.set_item("mid", 2.0)?;
            let op = module.getattr("op")?.call1((nodeset, py.None()))?;
            let v = op.getattr("v")?.call1(("mid",))?.extract::<f64>()?;
            assert!((v - 2.0).abs() < 1e-6, "divider mid should be 2.0, got {v}");
            Ok(())
        });
        let _ = std::fs::remove_file(&path);
        outcome
    }

    /// PY-13 / spec AC13: `op["instance"]` (or `trace["instance"]`) returns a
    /// terminal sub-view exposing that instance's terminal quantities —
    /// terminal voltages via `.v(port)` and the branch current via
    /// `.i(port_a, port_b)`, resolved through the POM hierarchy. Unresolved
    /// instance raises `KeyError` (spec edge case — fail loud).
    ///
    /// Divider (ANALYSIS_PHDL): `r_top : Resistor(.p = vin, .n = mid)` with
    /// `r = 3 kΩ`. At the DC operating point (vin = 5 V, mid = 2 V), the
    /// terminal sub-view of `r_top` reads:
    /// - `.v("p")` == `op.v("vin")` == 5.0 V (the connected net's voltage);
    /// - `.v("n")` == `op.v("mid")` == 2.0 V;
    /// - `.v("p", "n")` == `op.v("vin", "mid")` == 3.0 V (drop across r_top);
    /// - `.i("p", "n")` == `op.i("vin", "mid")` == 1 mA (branch current).
    /// `view["p"]` SHALL equal `view.v("p")` (uniform shape — the same
    /// `__getitem__ → .v` mapping the parent defines for net names).
    #[test]
    fn instance_path_returns_terminal_subview() -> PyResult<()> {
        let path = std::env::temp_dir().join("piperine_python_py13_instance_test.phdl");
        std::fs::write(&path, ANALYSIS_PHDL)?;
        let path_str = path
            .to_str()
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("non-utf8 temp path"))?;

        let outcome = Python::with_gil(|py| -> PyResult<()> {
            let design = loaded_design(py, path_str)?;
            let module = design.getattr("module")?.call1(("Divider",))?;

            // AC13: op["instance"] returns an _InstanceView.
            let op = module.getattr("op")?.call0()?;
            let view = op.getattr("__getitem__")?.call1(("r_top",))?;
            assert_eq!(
                view.getattr("__class__")?.getattr("__name__")?.extract::<String>()?,
                "_InstanceView",
                "op['r_top'] must return an _InstanceView"
            );
            assert_eq!(
                view.getattr("label")?.extract::<String>()?,
                "r_top",
                "view.label must be the instance label"
            );

            // Terminals: Resistor declares (p, n); r_top binds p→vin, n→mid.
            // Port-declaration order is preserved.
            let terminals: Vec<(String, String)> = view
                .getattr("terminals")?
                .call0()?
                .try_iter()?
                .map(|t| {
                    let t: Bound<'_, PyAny> = t?;
                    let port = t.getattr("port")?.extract::<String>()?;
                    let net = t.getattr("net")?.extract::<String>()?;
                    Ok::<(String, String), PyErr>((port, net))
                })
                .collect::<PyResult<Vec<_>>>()?;
            assert_eq!(
                terminals,
                vec![("p".to_string(), "vin".to_string()), ("n".to_string(), "mid".to_string())],
                "terminals must map port→connected net in declaration order"
            );

            // AC13 terminal voltages: .v(port) reads the connected net.
            let v_p = view.getattr("v")?.call1(("p",))?.extract::<f64>()?;
            assert!(
                (v_p - 5.0).abs() < 1e-6,
                "view.v('p') should be vin = 5.0 V, got {v_p}"
            );
            let v_n = view.getattr("v")?.call1(("n",))?.extract::<f64>()?;
            assert!(
                (v_n - 2.0).abs() < 1e-6,
                "view.v('n') should be mid = 2.0 V, got {v_n}"
            );
            let v_diff = view.getattr("v")?.call1(("p", "n"))?.extract::<f64>()?;
            assert!(
                (v_diff - 3.0).abs() < 1e-6,
                "view.v('p', 'n') should be the 3.0 V drop across r_top, got {v_diff}"
            );

            // AC13 branch current: .i(p, n) is the current through r_top.
            let i_rtop = view.getattr("i")?.call1(("p", "n"))?.extract::<f64>()?;
            assert!(
                (i_rtop - 1e-3).abs() < 1e-9,
                "view.i('p', 'n') should be 1 mA through r_top, got {i_rtop}"
            );

            // Uniform shape: view[port] == view.v(port).
            let item_v = view.getattr("__getitem__")?.call1(("p",))?.extract::<f64>()?;
            assert!(
                (item_v - v_p).abs() < 1e-12,
                "view['p'] should equal view.v('p'), got {item_v} vs {v_p}"
            );

            // Spec edge case: an unknown instance raises KeyError (fail loud).
            let miss = op.getattr("__getitem__")?.call1(("no_such_instance",)).unwrap_err();
            assert!(
                miss.is_instance_of::<pyo3::exceptions::PyKeyError>(py),
                "op['no_such_instance'] must raise KeyError, got {miss}"
            );

            // AC13 (trace variant): trace["instance"] returns an _InstanceView
            // whose .v(port) is a _Waveform over the connected net.
            let trace = module.getattr("tran")?.call1((5e-3, 1e-5))?;
            let tview = trace.getattr("__getitem__")?.call1(("r_top",))?;
            assert_eq!(
                tview.getattr("__class__")?.getattr("__name__")?.extract::<String>()?,
                "_InstanceView",
                "trace['r_top'] must return an _InstanceView"
            );
            let twf = tview.getattr("v")?.call1(("n",))?;
            assert_eq!(
                twf.getattr("__class__")?.getattr("__name__")?.extract::<String>()?,
                "_Waveform",
                "trace['r_top'].v('n') must return a _Waveform over mid"
            );
            // mid is DC 2.0 V — the transient is flat at 2.0 V (spec-defined).
            let twf_ref = twf.extract::<pyo3::PyRef<'_, super::_Waveform>>()?;
            let pts = twf_ref.inner.points();
            assert!(!pts.is_empty());
            assert!(
                (pts[0].1 - 2.0).abs() < 1e-3,
                "trace['r_top'].v('n').points[0].1 should be ~2.0 V (mid), got {}",
                pts[0].1
            );
            Ok(())
        });
        let _ = std::fs::remove_file(&path);
        outcome
    }

    /// PY-07 / spec AC7/10: `Trace.v(net)` returns a Waveform over the time
    /// axis; `Trace["net"]` SHALL return the same Waveform (AC10 — the
    /// `.values` array equality is verified in P8 once numpy lands; here we
    /// assert the wrapper equivalence via the inner bench waveform).
    /// `Trace.axis()` returns the time-axis Waveform. An unknown net on `.v`
    /// raises `KeyError` (fail loud).
    ///
    /// Divider mid is a DC 2.0 V, so the transient `mid` waveform is flat at
    /// 2.0 V across the recorded time grid (spec-defined outcome derived from
    /// the DC operating point). P7 doesn't expose `_Waveform.at/.values` to
    /// Python yet (that's P8); the value is read through the bench `Waveform`
    /// readout on the extracted inner — the uniform-shape check (same call
    /// the bench makes).
    #[test]
    fn trace_reads_waveforms_and_axis() -> PyResult<()> {
        let path = std::env::temp_dir().join("piperine_python_p7_trace_test.phdl");
        std::fs::write(&path, ANALYSIS_PHDL)?;
        let path_str = path
            .to_str()
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("non-utf8 temp path"))?;

        let outcome = Python::with_gil(|py| -> PyResult<()> {
            let design = loaded_design(py, path_str)?;
            let module = design.getattr("module")?.call1(("Divider",))?;
            let trace = module.getattr("tran")?.call1((5e-3, 1e-5))?;

            // AC7: trace.v(net) returns a _Waveform.
            let wf = trace.getattr("v")?.call1(("mid",))?;
            assert_eq!(
                wf.getattr("__class__")?.getattr("__name__")?.extract::<String>()?,
                "_Waveform",
                "trace.v(net) must return a _Waveform"
            );

            // The DC divider's mid sits at 2.0 V — the transient is flat at
            // 2.0 V (a linear DC source + R divider has no startup
            // dynamics). Read via the bench `Waveform::points` on the
            // extracted inner — same data the bench exposes (uniform-shape).
            // (`at` is ambiguous between the real + complex inherent impls;
            // `points` is defined once on `impl<T: Copy>`.)
            let wf_ref = wf.extract::<pyo3::PyRef<'_, super::_Waveform>>()?;
            let pts = wf_ref.inner.points();
            assert!(!pts.is_empty(), "tran waveform should not be empty");
            let v_first = pts[0].1;
            assert!(
                (v_first - 2.0).abs() < 1e-3,
                "trace.v(mid).points[0].1 should be ~2.0 V, got {v_first}"
            );

            // AC7: trace.axis() returns the time-axis _Waveform.
            let axis = trace.getattr("axis")?.call0()?;
            assert_eq!(
                axis.getattr("__class__")?.getattr("__name__")?.extract::<String>()?,
                "_Waveform",
                "trace.axis() must return a _Waveform"
            );

            // AC10: trace["net"] returns the same waveform (equivalence
            // verified through the inner bench readout — `.values` array
            // equality is P8's numpy assertion).
            let item_wf = trace.getattr("__getitem__")?.call1(("mid",))?;
            let item_ref = item_wf.extract::<pyo3::PyRef<'_, super::_Waveform>>()?;
            let item_pts = item_ref.inner.points();
            let item_at0 = item_pts[0].1;
            assert!(
                (item_at0 - v_first).abs() < 1e-12,
                "trace['mid'] should match trace.v('mid'): {item_at0} vs {v_first}"
            );

            // Spec edge case: an unknown net raises KeyError (fail loud).
            let miss = trace.getattr("v")?.call1(("nope",)).unwrap_err();
            assert!(
                miss.is_instance_of::<pyo3::exceptions::PyKeyError>(py),
                "trace.v('nope') must raise KeyError, got {miss}"
            );
            Ok(())
        });
        let _ = std::fs::remove_file(&path);
        outcome
    }

    /// PY-08 / spec AC7/edge: `_Waveform.values` and `.axis` are real
    /// `np.ndarray`s of equal length; `.axis` is the time grid. Stats
    /// (`.at/.rms/.mean/.min/.max/.peak_to_peak/.len`) return correct floats.
    ///
    /// Divider mid is DC 2.0 V — the transient is flat at 2.0 V across the
    /// recorded grid, so `min == max == mean == rms == 2.0` and
    /// `peak_to_peak == 0.0` (spec-defined outcome derived from the DC
    /// operating point; uniform-shape — same reductions the bench Waveform
    /// computes).
    #[test]
    fn waveform_exposes_numpy_and_stats() -> PyResult<()> {
        let path = std::env::temp_dir().join("piperine_python_p8_waveform_test.phdl");
        std::fs::write(&path, ANALYSIS_PHDL)?;
        let path_str = path
            .to_str()
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("non-utf8 temp path"))?;

        let outcome = Python::with_gil(|py| -> PyResult<()> {
            let design = loaded_design(py, path_str)?;
            let module = design.getattr("module")?.call1(("Divider",))?;
            let trace = module.getattr("tran")?.call1((5e-3, 1e-5))?;
            let wf = trace.getattr("v")?.call1(("mid",))?;

            // AC7: .values is a real np.ndarray (float64, not None).
            let values_obj = wf.getattr("values")?;
            let np = py.import("numpy")?;
            let ndarray_ty = np.getattr("ndarray")?;
            assert!(
                values_obj.is_instance(&ndarray_ty)?,
                ".values must be a numpy.ndarray"
            );
            let values_dtype = values_obj.getattr("dtype")?.getattr("name")?.extract::<String>()?;
            assert_eq!(
                values_dtype, "float64",
                ".values must be real (float64), got {values_dtype}"
            );

            // Extract as a typed readonly array for value/length assertions.
            let values = values_obj.extract::<numpy::PyReadonlyArray1<'_, f64>>()?;
            let values_slice = values.as_array();
            assert!(
                !values_slice.is_empty(),
                ".values must not be empty on a non-empty tran"
            );
            assert!(
                values_slice.iter().all(|v| (v - 2.0).abs() < 1e-3),
                "flat 2.0 V transient: every sample ≈ 2.0 V, got {:?}",
                values_slice
            );

            // AC7: .axis is the time grid, equal length to .values.
            let axis_obj = wf.getattr("axis")?;
            assert!(
                axis_obj.is_instance(&ndarray_ty)?,
                ".axis must be a numpy.ndarray"
            );
            let axis = axis_obj.extract::<numpy::PyReadonlyArray1<'_, f64>>()?;
            let axis_slice = axis.as_array();
            assert_eq!(
                axis_slice.len(),
                values_slice.len(),
                ".axis and .values must be equal length"
            );
            assert!(
                axis_slice.iter().all(|t| *t >= 0.0),
                "time axis must be non-negative"
            );
            // The tran was run with stop=5e-3 — the recorded axis ends at
            // (or very near) 5e-3.
            let t_end = axis_slice.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            assert!(
                (t_end - 5e-3).abs() < 1e-4,
                "axis end should be ~5e-3 (the tran stop), got {t_end}"
            );

            // Stats — uniform-shape (PY-17): same reductions the bench
            // Waveform computes. The flat 2.0 V transient gives every
            // reduction the value 2.0 (mean/rms/min/max/at), peak_to_peak 0.
            let len = wf.getattr("len")?.call0()?.extract::<usize>()?;
            assert_eq!(len, values_slice.len(), ".len() must equal .values length");
            let at0 = wf.getattr("at")?.call1((0.0,))?.extract::<f64>()?;
            let at_mid = wf.getattr("at")?.call1((2.5e-3,))?.extract::<f64>()?;
            let min = wf.getattr("min")?.call0()?.extract::<f64>()?;
            let max = wf.getattr("max")?.call0()?.extract::<f64>()?;
            let mean = wf.getattr("mean")?.call0()?.extract::<f64>()?;
            let rms = wf.getattr("rms")?.call0()?.extract::<f64>()?;
            let ptp = wf.getattr("peak_to_peak")?.call0()?.extract::<f64>()?;
            for (label, v) in [
                ("at(0)", at0),
                ("at(2.5e-3)", at_mid),
                ("min", min),
                ("max", max),
                ("mean", mean),
                ("rms", rms),
            ] {
                assert!(
                    (v - 2.0).abs() < 1e-3,
                    "flat 2.0 V transient: {label} should be ~2.0, got {v}"
                );
            }
            assert!(ptp.abs() < 1e-9, "flat waveform peak_to_peak should be 0, got {ptp}");
            Ok(())
        });
        let _ = std::fs::remove_file(&path);
        outcome
    }

    /// PY-09 / spec AC8: `module.ac(...)` → `_AcTrace.v(net)` returns a
    /// `_ComplexWaveform` whose `.values` is a complex `np.ndarray`;
    /// `.mag/.phase/.db` return real `_Waveform`s. `_AcTrace.axis()` returns
    /// the frequency-axis `_Waveform`.
    ///
    /// Mirrors the root suite's AC low-pass smoke (tests/spice_smoke.rs):
    /// 1 A of `ac_stim` current into
    /// a 1 kΩ resistor to gnd → |V_out| = 1 A × 1 kΩ = 1000 V at every
    /// frequency (purely resistive, flat). The spec-defined expected outcome
    /// (PY-17 uniform-shape — same call the bench makes).
    #[test]
    fn ac_returns_complex_waveform_with_projections() -> PyResult<()> {
        // Dedicated AC fixture: an `ac_stim` current source driving a 1 kΩ
        // resistor to ground. `ac_stim(mag)` is the small-signal injection
        // (bench spec §5); `-ac_stim(1.0)` means 1 A flows out of `p` into
        // the source (the bench convention).
        const AC_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod AcSource(inout p: Electrical, inout n: Electrical) { }
analog AcSource { I(p, n) <+ -ac_stim(1.0); }

mod Resistor(inout p: Electrical, inout n: Electrical) {
    param r: Real = 1e3;
}
analog Resistor { I(p, n) <+ V(p, n) / r; }

mod AcTest() {
    wire gnd : Electrical;
    wire out : Electrical;
    stim : AcSource (.p = out, .n = gnd);
    r1   : Resistor (.p = out, .n = gnd) { .r = 1e3 };
}
";
        let path = std::env::temp_dir().join("piperine_python_p9_ac_test.phdl");
        std::fs::write(&path, AC_PHDL)?;
        let path_str = path
            .to_str()
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("non-utf8 temp path"))?;

        let outcome = Python::with_gil(|py| -> PyResult<()> {
            let design = loaded_design(py, path_str)?;
            let module = design.getattr("module")?.call1(("AcTest",))?;
            let ac = module.getattr("ac")?.call1((1.0, 1e6, 10))?;

            // AC8: ac.v(net) returns a _ComplexWaveform.
            let cw = ac.getattr("v")?.call1(("out",))?;
            assert_eq!(
                cw.getattr("__class__")?.getattr("__name__")?.extract::<String>()?,
                "_ComplexWaveform",
                "ac.v(net) must return a _ComplexWaveform"
            );

            // AC8: .values is a complex np.ndarray (complex128).
            let values_obj = cw.getattr("values")?;
            let np = py.import("numpy")?;
            let ndarray_ty = np.getattr("ndarray")?;
            assert!(
                values_obj.is_instance(&ndarray_ty)?,
                ".values must be a numpy.ndarray"
            );
            let values_dtype = values_obj.getattr("dtype")?.getattr("name")?.extract::<String>()?;
            assert_eq!(
                values_dtype, "complex128",
                ".values must be complex (complex128), got {values_dtype}"
            );
            let values =
                values_obj.extract::<numpy::PyReadonlyArray1<'_, num_complex::Complex64>>()?;
            assert_eq!(values.as_array().len(), 10, "AC sweep had 10 points");

            // 1 A × 1 kΩ = 1000 V at every frequency (resistive, flat).
            // (PY-17 uniform-shape — same magnitude the bench asserts in
            // `ac_stim_drives_a_low_pass_response` for the passband.)
            for (i, c) in values.as_array().iter().enumerate() {
                assert!(
                    (c.norm() - 1000.0).abs() < 1.0,
                    "AC |v_out| at point {i} should be ~1000 V (1 A × 1 kΩ), got {}",
                    c.norm()
                );
            }

            // AC8: .mag/.phase/.db return real _Waveforms (properties per spec).
            for proj in ["mag", "phase", "db"] {
                let w = cw.getattr(proj)?;
                assert_eq!(
                    w.getattr("__class__")?.getattr("__name__")?.extract::<String>()?,
                    "_Waveform",
                    "{proj} must return a _Waveform"
                );
                let w_vals = w.getattr("values")?.extract::<numpy::PyReadonlyArray1<'_, f64>>()?;
                assert_eq!(w_vals.as_array().len(), 10, "{proj} length must match AC sweep");
            }
            // .mag value ≈ 1000 (matches the complex magnitude above).
            let mag_at_first = cw.getattr("mag")?.getattr("at")?.call1((1.0,))?.extract::<f64>()?;
            assert!(
                (mag_at_first - 1000.0).abs() < 1.0,
                "ac.v('out').mag.at(fstart) should be ~1000, got {mag_at_first}"
            );

            // AC8: ac.axis() returns the frequency-axis _Waveform.
            let axis = ac.getattr("axis")?.call0()?;
            assert_eq!(
                axis.getattr("__class__")?.getattr("__name__")?.extract::<String>()?,
                "_Waveform",
                "ac.axis() must return a _Waveform"
            );
            let axis_vals = axis.getattr("values")?.extract::<numpy::PyReadonlyArray1<'_, f64>>()?;
            assert_eq!(axis_vals.as_array().len(), 10, "axis length must match AC sweep");
            assert!(
                axis_vals.as_array().iter().all(|f| *f >= 1.0 && *f <= 1e6),
                "log-sweep from 1 Hz to 1 MHz"
            );
            Ok(())
        });
        let _ = std::fs::remove_file(&path);
        outcome
    }

    /// PY-10 / spec AC9: `module.noise(...)` → `_NoiseTrace.psd()` returns a
    /// `_Waveform` with the configured sweep length; `.total()` returns a
    /// non-negative float. Mirrors the johnson-noise example fixture:
    /// a `NoisyResistor`
    /// with explicit `white_noise` so the PSD is non-zero and the integrated
    /// total is observable.
    #[test]
    fn noise_returns_psd_waveform_and_total() -> PyResult<()> {
        const NOISE_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod NoisyResistor(inout p: Electrical, inout n: Electrical) {
    param r: Real = 1e3;
}
analog NoisyResistor { I(p, n) <+ V(p, n) / r + white_noise(4 * 8.617e-5 * 300.15 / r); }

mod NoiseTest() {
    wire gnd : Electrical;
    wire out : Electrical;
    nr : NoisyResistor (.p = out, .n = gnd) { .r = 1e3 };
}
";
        let path = std::env::temp_dir().join("piperine_python_p9_noise_test.phdl");
        std::fs::write(&path, NOISE_PHDL)?;
        let path_str = path
            .to_str()
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("non-utf8 temp path"))?;

        let outcome = Python::with_gil(|py| -> PyResult<()> {
            let design = loaded_design(py, path_str)?;
            let module = design.getattr("module")?.call1(("NoiseTest",))?;
            let noise = module.getattr("noise")?.call1(("out", 1.0, 1e6, 5))?;

            // AC9: psd() returns a _Waveform with the configured sweep length.
            let psd = noise.getattr("psd")?.call0()?;
            assert_eq!(
                psd.getattr("__class__")?.getattr("__name__")?.extract::<String>()?,
                "_Waveform",
                "noise.psd() must return a _Waveform"
            );
            let psd_vals = psd.getattr("values")?.extract::<numpy::PyReadonlyArray1<'_, f64>>()?;
            assert_eq!(
                psd_vals.as_array().len(),
                5,
                "psd length must match noise sweep points"
            );
            // PSD is non-negative (V²/Hz).
            assert!(
                psd_vals.as_array().iter().all(|v| *v >= 0.0),
                "PSD samples must be non-negative (V²/Hz)"
            );

            // AC9: total() returns a non-negative float (integrated RMS).
            let total = noise.getattr("total")?.call0()?.extract::<f64>()?;
            assert!(
                total >= 0.0,
                "integrated noise total must be non-negative, got {total}"
            );
            Ok(())
        });
        let _ = std::fs::remove_file(&path);
        outcome
    }
}
