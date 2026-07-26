//! PY-10 — the native noise surface: `_NoiseTrace.psd()` returns a
//! `_Waveform` of the configured sweep length with non-negative samples, and
//! `total()` returns the non-negative integrated value.

mod common;

use pyo3::prelude::*;

use common::{loaded_design, NOISE_PHDL};

/// PY-10 / spec AC9: `module.noise(...)` → `_NoiseTrace.psd()` returns a
/// `_Waveform` with the configured sweep length; `.total()` returns a
/// non-negative float. Mirrors the johnson-noise example fixture:
/// a `NoisyResistor`
/// with explicit `white_noise` so the PSD is non-zero and the integrated
/// total is observable.
#[test]
fn noise_returns_psd_waveform_and_total() -> PyResult<()> {
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
