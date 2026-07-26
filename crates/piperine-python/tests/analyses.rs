//! PY-04/06/12/13 + CP-09 — the native `_Module` analysis surface: each
//! analysis returns its typed result object, `set` stages an override for the
//! next run without mutating the parent design, `_OpResult` reads voltages and
//! currents (and raises on an unknown net), `SolverStats` is exposed on the
//! results, the solver config reaches Newton, and an instance path returns a
//! terminal sub-view.

mod common;

use pyo3::prelude::*;

use common::{loaded_design, ANALYSIS_PHDL};

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
/// root's typed `OpResult::v` readout (uniform-shape proof — the same
/// call a Rust host makes) by extracting the inner result from the
/// returned `_OpResult`.
///
/// Divider math: `mid = 5·r_bot/(r_top+r_bot)`. Default 3 k/2 k → 2.0 V;
/// staging `r_top.r = 2e3` → 2 k/2 k → 2.5 V. The default-vs-staged
/// delta (0.5 V) is the spec-defined outcome (AC12: "each result SHALL
/// reflect that iteration's staged value").
#[test]
fn stage_overrides_next_analysis() -> PyResult<()> {
    use pyo3::types::PyAnyMethods;
    use piperine_api::OpResult as HostOpResult;

    let path = std::env::temp_dir().join("piperine_python_p6_stage_test.phdl");
    std::fs::write(&path, ANALYSIS_PHDL)?;
    let path_str = path
        .to_str()
        .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("non-utf8 temp path"))?;

    let outcome = Python::with_gil(|py| -> PyResult<()> {
        let design = loaded_design(py, path_str)?;
        let module = design.getattr("module")?.call1(("Divider",))?;

        // Helper: run op() and read `mid` through the host readout.
        let mid_voltage = |module: &Bound<'_, PyAny>| -> PyResult<f64> {
            let op_obj = module.getattr("op")?.call0()?;
            let pyref = op_obj.extract::<pyo3::PyRef<'_, piperine_python::results::_OpResult>>()?;
            // `inner` is `Rc<OpResult>`; deref through Rc to call `v`
            // (HOST-23: `"mid"` resolves through `NetRef`'s `Into`
            // ergonomics — no bare `NetRef { name }` needed).
            let v = HostOpResult::v(&pyref.inner, "mid")
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
        module.getattr("set")?.call1(("r_top", "r", 2e3))?;
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
            let pyref = op_obj.extract::<pyo3::PyRef<'_, piperine_python::results::_OpResult>>()?;
            HostOpResult::v(&pyref.inner, "mid")
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
///   `view["p"]` SHALL equal `view.v("p")` (uniform shape — the same
///   `__getitem__ → .v` mapping the parent defines for net names).
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
        // Port-declaration order is preserved. (PY-13 connectivity —
        // renamed from `terminals()` to `terminal_connections()` so the
        // HOST-09 descriptor property can take the `terminals` name.)
        let terminals: Vec<(String, String)> = view
            .getattr("terminal_connections")?
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
        let twf_ref = twf.extract::<pyo3::PyRef<'_, piperine_python::results::_Waveform>>()?;
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
