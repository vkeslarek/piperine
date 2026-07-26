//! PY-09 — the native AC surface: `_AcTrace.v(net)` returns a
//! `_ComplexWaveform` whose `values` is a complex numpy array, its
//! `mag`/`phase`/`db` projections are real `_Waveform`s, and `axis()` is the
//! frequency grid.

mod common;

use pyo3::prelude::*;

use common::{loaded_design, AC_PHDL};

/// PY-09 / spec AC8: `module.ac(...)` → `_AcTrace.v(net)` returns a
/// `_ComplexWaveform` whose `.values` is a complex `np.ndarray`;
/// `.mag/.phase/.db` return real `_Waveform`s. `_AcTrace.axis()` returns
/// the frequency-axis `_Waveform`.
///
/// Mirrors the root suite's AC low-pass smoke (tests/spice_smoke.rs):
/// 1 A of `ac_stim` current into
/// a 1 kΩ resistor to gnd → |V_out| = 1 A × 1 kΩ = 1000 V at every
/// frequency (purely resistive, flat). The spec-defined expected outcome
/// (PY-17 uniform-shape — the same call a Rust host makes).
#[test]
fn ac_returns_complex_waveform_with_projections() -> PyResult<()> {
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
        // (PY-17 uniform-shape — same magnitude the root suite asserts in
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
