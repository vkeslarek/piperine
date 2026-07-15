//! `_OpResult`/`_Trace`/`_Waveform`/`_AcTrace`/`_NoiseTrace` — typed Python
//! wrappers over the bench result objects (PY-06/07/08/09/10). P6 landed the
//! shells so [`crate::module::_Module::op`]/`tran`/`ac`/`noise` could return
//! them; P7 adds `.v/.i/__getitem__` to `_OpResult`/`_Trace` and introduces
//! the `_Waveform` wrapper (numpy + stats arrive in P8); P9 adds the AC/noise
//! readouts. Each wrapper owns its bench result by value (the result is
//! `'static`).
//!
//! MD-13 note: the wrappers are pyclasses — every function is a method on
//! the struct. No loose module-level functions.

use pyo3::exceptions::{PyKeyError, PyRuntimeError};
use pyo3::prelude::*;

use piperine_bench::{NetRef, OpResult, Trace, Waveform};

/// Surface a bench readout error as the right Python exception: an
/// unaddressable net reads as `KeyError` (spec edge case — fail loud, never a
/// silent NaN); everything else as `RuntimeError` carrying the diagnostic.
/// Mirrors [`crate::module::_Module::analysis_err`] over the same string
/// contract — bench errors implement `Display` via `thiserror`.
fn readout_err<E: std::fmt::Display>(e: E) -> PyErr {
    let msg = format!("{e}");
    if msg.contains("is not addressable") {
        PyKeyError::new_err(msg)
    } else {
        PyRuntimeError::new_err(msg)
    }
}

/// `_OpResult` — the typed `$op()` result (PY-06). Holds the immutable DC
/// snapshot produced by [`piperine_bench::session::SimSession::run_op`].
/// `.v/.i` (PY-06) and `__getitem__` (PY-11 / spec AC5) resolve nets by name
/// through the bench's own typed readout — the same call the bench makes
/// (uniform-shape proof).
#[pyclass(module = "piperine", unsendable)]
pub struct _OpResult {
    pub(crate) inner: OpResult,
}

impl _OpResult {
    pub(crate) fn new(inner: OpResult) -> Self {
        Self { inner }
    }

    /// Build a [`NetRef`] from a Python `str` — the typed handle every bench
    /// readout takes. Kept as a struct method (MD-13: no loose `fn`).
    fn net(name: &str) -> NetRef {
        NetRef {
            name: name.to_string(),
        }
    }
}

#[pymethods]
impl _OpResult {
    /// Node voltage of `a` minus `b` (ground-referenced when `b` is omitted)
    /// — spec AC4. A digital `Bit`/`Logic` net reads its logic value (0/1,
    /// NaN for X/Z). An unaddressable net raises `KeyError` (fail loud).
    #[pyo3(signature = (a, b=None))]
    fn v(&self, a: &str, b: Option<&str>) -> PyResult<f64> {
        let net_a = Self::net(a);
        let net_b = b.map(Self::net);
        self.inner.v(&net_a, net_b.as_ref()).map_err(readout_err)
    }

    /// Branch current from terminal `a` to `b` (ground-referenced when `b`
    /// is omitted) — spec AC4. Resolves the unique two-terminal instance
    /// connecting the named nets; raises `KeyError` for an unknown net and
    /// `RuntimeError` for an ambiguous branch.
    #[pyo3(signature = (a, b=None))]
    fn i(&self, a: &str, b: Option<&str>) -> PyResult<f64> {
        let net_a = Self::net(a);
        let net_b = b.map(Self::net);
        self.inner.i(&net_a, net_b.as_ref()).map_err(readout_err)
    }

    /// `op[name]` SHALL equal `op.v(name)` (spec AC5 / PY-11). Single-net
    /// voltage; differential + branch-current reads use `.v`/`.i` explicitly.
    fn __getitem__(&self, name: &str) -> PyResult<f64> {
        self.v(name, None)
    }
}

/// `_Trace` — the typed `$tran(...)` result (PY-07). `.v/.i` (PY-07) read
/// out a per-net `_Waveform` over the analysis axis (time, for `$tran`);
/// `__getitem__` (PY-11 / spec AC10) returns the same waveform handle. `.axis`
/// returns the time-axis waveform.
#[pyclass(module = "piperine", unsendable)]
pub struct _Trace {
    pub(crate) inner: Trace,
}

impl _Trace {
    pub(crate) fn new(inner: Trace) -> Self {
        Self { inner }
    }

    /// Build a [`NetRef`] from a Python `str` — the typed handle every bench
    /// readout takes. Kept as a struct method (MD-13).
    fn net(name: &str) -> NetRef {
        NetRef {
            name: name.to_string(),
        }
    }
}

#[pymethods]
impl _Trace {
    /// Net voltage `a` minus `b` (ground-referenced when `b` is omitted) over
    /// time — spec AC7. A digital net reads its logic value per step. An
    /// unaddressable net raises `KeyError` (fail loud).
    #[pyo3(signature = (a, b=None))]
    fn v(&self, a: &str, b: Option<&str>) -> PyResult<_Waveform> {
        let net_a = Self::net(a);
        let net_b = b.map(Self::net);
        let wf = self.inner.v(&net_a, net_b.as_ref()).map_err(readout_err)?;
        Ok(_Waveform::new(wf))
    }

    /// Branch current from `a` to `b` over time — spec AC7.
    #[pyo3(signature = (a, b=None))]
    fn i(&self, a: &str, b: Option<&str>) -> PyResult<_Waveform> {
        let net_a = Self::net(a);
        let net_b = b.map(Self::net);
        let wf = self.inner.i(&net_a, net_b.as_ref()).map_err(readout_err)?;
        Ok(_Waveform::new(wf))
    }

    /// The time-axis `_Waveform` (spec AC7 `.axis()`).
    fn axis(&self) -> _Waveform {
        _Waveform::new(self.inner.axis())
    }

    /// `trace[name]` returns the same `_Waveform` as `trace.v(name)` (spec
    /// AC10 / PY-11). The spec phrases AC10 as returning the `.values` array;
    /// `.values` is the numpy projection (PY-08, P8) over the same waveform
    /// this returns — `trace["mid"].values == trace.v("mid").values` is then
    /// the full AC10 equality, verified end-to-end in P8's test.
    fn __getitem__(&self, name: &str) -> PyResult<_Waveform> {
        self.v(name, None)
    }
}

/// `_Waveform` — a swept series of `(axis, value)` samples for one measured
/// quantity over the analysis axis (PY-08). P7 introduces the wrapper as the
/// return type of [`_Trace::v`]/`_Trace::i`/`_Trace::axis`] and `__getitem__`;
/// P8 lands the numpy projections (`.axis`/`.values` as `np.ndarray`) and the
/// scalar stats (`.at/.rms/.mean/...`) by delegating to the bench
/// [`Waveform`]'s own typed methods (uniform-shape: same reductions the bench
/// computes).
#[pyclass(module = "piperine", unsendable)]
pub struct _Waveform {
    // Read by P8's `.axis/.values` (numpy) + `.at/.rms/...` (stats), and by
    // the P7 trace test (cfg(test)) via PyRef extraction. `allow(dead_code)`
    // keeps the non-test build warning-free until P8 lands.
    #[allow(dead_code)]
    pub(crate) inner: Waveform,
}

impl _Waveform {
    pub(crate) fn new(inner: Waveform) -> Self {
        Self { inner }
    }
}

// `_AcTrace`/`_NoiseTrace` shells kept here — P9 lands their `.v/.axis/.psd/
// .total` readouts. They are constructed by `_Module::ac`/`_Module::noise`
// (P6) and registered with the module (lib.rs) so analysis-end-to-end
// wiring is testable from P6 onward.

/// `_AcTrace` — the typed `$ac(...)` result (PY-09). `.v/.axis` arrive in P9.
#[pyclass(module = "piperine", unsendable)]
pub struct _AcTrace {
    // Read by P9's `.v/.axis`; `allow(dead_code)` until then.
    #[allow(dead_code)]
    pub(crate) inner: piperine_bench::AcTrace,
}

impl _AcTrace {
    pub(crate) fn new(inner: piperine_bench::AcTrace) -> Self {
        Self { inner }
    }
}

/// `_NoiseTrace` — the typed `$noise(...)` result (PY-10). `.psd/.total`
/// arrive in P9.
#[pyclass(module = "piperine", unsendable)]
pub struct _NoiseTrace {
    // Read by P9's `.psd/.total`; `allow(dead_code)` until then.
    #[allow(dead_code)]
    pub(crate) inner: piperine_bench::NoiseTrace,
}

impl _NoiseTrace {
    pub(crate) fn new(inner: piperine_bench::NoiseTrace) -> Self {
        Self { inner }
    }
}
