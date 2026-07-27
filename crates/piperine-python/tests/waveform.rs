//! PY-07/08 — the native `_Trace`/`_Waveform` surface: `v(net)`/`axis()`/
//! `trace[net]` return waveforms over the time axis (unknown net raises), and
//! a `_Waveform` exposes real numpy `values`/`axis` plus the
//! `at/rms/mean/min/max/peak_to_peak/len` reductions.

mod common;

use pyo3::prelude::*;

use common::{loaded_design, ANALYSIS_PHDL};

/// PY-07 / spec AC7/10: `Trace.v(net)` returns a Waveform over the time
/// axis; `Trace["net"]` SHALL return the same Waveform (AC10 — the
/// `.values` array equality is verified in P8 once numpy lands; here we
/// assert the wrapper equivalence via the inner host waveform).
/// `Trace.axis()` returns the time-axis Waveform. An unknown net on `.v`
/// raises `KeyError` (fail loud).
///
/// Divider mid is a DC 2.0 V, so the transient `mid` waveform is flat at
/// 2.0 V across the recorded time grid (spec-defined outcome derived from
/// the DC operating point). P7 doesn't expose `_Waveform.at/.values` to
/// Python yet (that's P8); the value is read through the host `Waveform`
/// readout on the extracted inner — the uniform-shape check (same call
/// the host makes).
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
        // dynamics). Read via the host `Waveform::points` on the
        // extracted inner — same data the host exposes (uniform-shape).
        // (`at` is ambiguous between the real + complex inherent impls;
        // `points` is defined once on `impl<T: Copy>`.)
        let wf_ref = wf.extract::<pyo3::PyRef<'_, piperine_python::results::_Waveform>>()?;
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
        // verified through the inner host readout — `.values` array
        // equality is P8's numpy assertion).
        let item_wf = trace.getattr("__getitem__")?.call1(("mid",))?;
        let item_ref = item_wf.extract::<pyo3::PyRef<'_, piperine_python::results::_Waveform>>()?;
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
/// operating point; uniform-shape — same reductions the host Waveform
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

        // Stats — uniform-shape (PY-17): same reductions the host
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
