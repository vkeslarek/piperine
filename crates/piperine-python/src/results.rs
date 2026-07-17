//! `_OpResult`/`_Trace`/`_Waveform`/`_AcTrace`/`_NoiseTrace` — typed Python
//! wrappers over the bench result objects (PY-06/07/08/09/10). P6 landed the
//! shells so [`crate::module::_Module::op`]/`tran`/`ac`/`noise` could return
//! them; P7 adds `.v/.i/__getitem__` to `_OpResult`/`_Trace` and introduces
//! the `_Waveform` wrapper (numpy + stats arrive in P8); P9 adds the AC/noise
//! readouts; PY-13 (Batch 3 Task A) extends `__getitem__` to route instance
//! paths to a terminal sub-view (`_InstanceView`).
//!
//! `_OpResult`/`_Trace` hold their bench result behind `Rc` so a sub-view can
//! cheaply share the parent without cloning the (potentially large) result
//! data; the sub-view holds an `Rc::clone` of the same underlying snapshot.
//!
//! MD-13 note: the wrappers are pyclasses — every function is a method on
//! the struct. No loose module-level functions.

use std::rc::Rc;

use pyo3::exceptions::{PyKeyError, PyRuntimeError};
use pyo3::prelude::*;

use num_complex::Complex64;
use piperine::{AcTrace, ComplexWaveform, NetRef, NoiseTrace, OpResult, Trace, Waveform};
use piperine_solver::abi::SolverStats;

use crate::instance::InstanceResolver;

/// `_SolverStats` — per-analysis convergence + performance diagnostics
/// (CP-09). Every field from the solver's `SolverStats` is exposed as a
/// typed Python attribute.
#[pyclass(module = "piperine")]
pub struct _SolverStats {
    #[pyo3(get)]
    pub newton_iterations: usize,
    #[pyo3(get)]
    pub converged: bool,
    #[pyo3(get)]
    pub steps_accepted: usize,
    #[pyo3(get)]
    pub steps_rejected: usize,
    #[pyo3(get)]
    pub dt_min_floor_hits: usize,
    #[pyo3(get)]
    pub dt_min: f64,
    #[pyo3(get)]
    pub dt_max: f64,
    #[pyo3(get)]
    pub bypass_hits: usize,
    #[pyo3(get)]
    pub bypass_misses: usize,
    #[pyo3(get)]
    pub homotopy_strategy: Option<String>,
    #[pyo3(get)]
    pub homotopy_levels: usize,
    #[pyo3(get)]
    pub assembly_time_ns: u64,
    #[pyo3(get)]
    pub solve_time_ns: u64,
}

impl _SolverStats {
    fn from_solver(s: &SolverStats) -> Self {
        Self {
            newton_iterations: s.newton_iterations,
            converged: s.converged,
            steps_accepted: s.steps_accepted,
            steps_rejected: s.steps_rejected,
            dt_min_floor_hits: s.dt_min_floor_hits,
            dt_min: s.dt_min,
            dt_max: s.dt_max,
            bypass_hits: s.bypass_hits,
            bypass_misses: s.bypass_misses,
            homotopy_strategy: s.homotopy_strategy.clone(),
            homotopy_levels: s.homotopy_levels,
            assembly_time_ns: s.assembly_time_ns,
            solve_time_ns: s.solve_time_ns,
        }
    }
}

/// Surface a bench readout error as the right Python exception: an
/// unaddressable net reads as `KeyError` (spec edge case — fail loud, never a
/// silent NaN); everything else as `RuntimeError` carrying the diagnostic.
/// Mirrors [`crate::module::_Module::analysis_err`] over the same string
/// contract — bench errors implement `Display` via `thiserror`.
pub(crate) fn readout_err<E: std::fmt::Display>(e: E) -> PyErr {
    let msg = format!("{e}");
    if msg.contains("is not addressable") {
        PyKeyError::new_err(msg)
    } else {
        PyRuntimeError::new_err(msg)
    }
}

/// `_OpResult` — the typed `$op()` result (PY-06). Holds the immutable DC
/// snapshot produced by [`piperine::session::SimSession::run_op`] behind
/// `Rc` so a PY-13 instance sub-view can share it cheaply. `.v/.i` (PY-06) and
/// `__getitem__` (PY-11 / spec AC5) resolve nets by name through the bench's
/// own typed readout — the same call the bench makes (uniform-shape proof).
///
/// When constructed via [`crate::module::_Module::op`], the result also
/// carries an [`InstanceResolver`] so `__getitem__` can detect instance paths
/// (PY-13 / spec AC13: `op["r_top"]` returns a terminal sub-view). The
/// resolver is `None` for results built outside `_Module` (existing unit
/// tests that wrap a bench `OpResult` directly).
#[pyclass(module = "piperine", unsendable)]
pub struct _OpResult {
    pub(crate) inner: Rc<OpResult>,
    resolver: Option<InstanceResolver>,
}

impl _OpResult {
    pub(crate) fn new(inner: OpResult) -> Self {
        Self {
            inner: Rc::new(inner),
            resolver: None,
        }
    }

    /// Attach the instance-path resolver (PY-13). Called by
    /// [`crate::module::_Module::op`] after construction so `__getitem__`
    /// can detect instance labels and route to a terminal sub-view.
    pub(crate) fn with_resolver(mut self, resolver: InstanceResolver) -> Self {
        self.resolver = Some(resolver);
        self
    }

    /// A shared handle to the underlying snapshot — the sub-view clones this
    /// `Rc` rather than copying the result data.
    pub(crate) fn shared(&self) -> Rc<OpResult> {
        Rc::clone(&self.inner)
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

    /// Per-analysis convergence + performance statistics.
    #[getter]
    fn stats(&self) -> _SolverStats {
        _SolverStats::from_solver(self.inner.stats())
    }

    /// `op[name]` (spec AC5 / PY-11 + PY-13):
    /// - **Net name** → `op.v(name)` (a `float`); AC5.
    /// - **Instance path** (a `.`/`/`-separated path, or a bare label that
    ///   matches one of the module's instances) → a terminal sub-view
    ///   exposing that instance's terminal voltages + branch current; AC13.
    ///
    /// Instance-path detection lives in [`InstanceResolver::looks_like_instance`];
    /// unresolved paths raise `KeyError` (spec edge case — fail loud).
    fn __getitem__(&self, py: Python<'_>, name: &str) -> PyResult<PyObject> {
        if let Some(resolver) = &self.resolver
            && resolver.looks_like_instance(name)
        {
            let label = resolver.resolve_label(name)?;
            let view = crate::instance::_InstanceView::new_op(
                self.shared(),
                resolver.shared(),
                label,
            );
            return Ok(Py::new(py, view)?.into_any());
        }
        let f = self.v(name, None)?;
        Ok(f.into_pyobject(py)?.into_any().unbind())
    }
}

/// `_Trace` — the typed `$tran(...)` result (PY-07). `.v/.i` (PY-07) read
/// out a per-net `_Waveform` over the analysis axis (time, for `$tran`);
/// `__getitem__` (PY-11 / spec AC10) returns the same waveform handle.
/// `.axis` returns the time-axis waveform. The snapshot lives behind `Rc` so
/// a PY-13 instance sub-view can share it (mirrors `_OpResult`).
#[pyclass(module = "piperine", unsendable)]
pub struct _Trace {
    pub(crate) inner: Rc<Trace>,
    resolver: Option<InstanceResolver>,
}

impl _Trace {
    pub(crate) fn new(inner: Trace) -> Self {
        Self {
            inner: Rc::new(inner),
            resolver: None,
        }
    }

    /// Attach the instance-path resolver (PY-13). Called by
    /// [`crate::module::_Module::tran`] after construction so `__getitem__`
    /// can detect instance labels and route to a terminal sub-view.
    pub(crate) fn with_resolver(mut self, resolver: InstanceResolver) -> Self {
        self.resolver = Some(resolver);
        self
    }

    /// A shared handle to the underlying trace — the sub-view clones this
    /// `Rc` rather than copying the result data.
    pub(crate) fn shared(&self) -> Rc<Trace> {
        Rc::clone(&self.inner)
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

    /// Per-analysis convergence + performance statistics.
    #[getter]
    fn stats(&self) -> _SolverStats {
        _SolverStats::from_solver(self.inner.stats())
    }

    /// `trace[name]` (spec AC10 / PY-11 + PY-13):
    /// - **Net name** → `trace.v(name)` (a `_Waveform`); AC10.
    /// - **Instance path** (a `.`/`/`-separated path, or a bare label that
    ///   matches one of the module's instances) → a terminal sub-view whose
    ///   `.v/.i` return `_Waveform`s of the connected nets; AC13.
    ///
    /// The spec phrases AC10 as returning the `.values` array; `.values` is
    /// the numpy projection (PY-08, P8) over the same waveform this returns —
    /// `trace["mid"].values == trace.v("mid").values` is then the full AC10
    /// equality, verified end-to-end in P8's test.
    fn __getitem__(&self, py: Python<'_>, name: &str) -> PyResult<PyObject> {
        if let Some(resolver) = &self.resolver
            && resolver.looks_like_instance(name)
        {
            let label = resolver.resolve_label(name)?;
            let view = crate::instance::_InstanceView::new_trace(
                self.shared(),
                resolver.shared(),
                label,
            );
            return Ok(Py::new(py, view)?.into_any());
        }
        let wf = self.v(name, None)?;
        Ok(Py::new(py, wf)?.into_any())
    }
}

/// `_Waveform` — a swept series of `(axis, value)` samples for one measured
/// quantity over the analysis axis (PY-08). `.values` and `.axis` are real
/// `np.ndarray`s of equal length (PY-08 / spec AC7); the scalar stats
/// (`.at/.rms/.mean/.min/.max/.peak_to_peak/.len`) delegate to the bench
/// [`Waveform`]'s own typed reductions — uniform-shape: same values the
/// bench computes (PY-17). P7 introduced the wrapper; P8 lands numpy + stats.
#[pyclass(module = "piperine", unsendable)]
pub struct _Waveform {
    pub(crate) inner: Waveform,
}

impl _Waveform {
    pub(crate) fn new(inner: Waveform) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl _Waveform {
    /// The values as a real `np.ndarray` (PY-08 / spec AC7). Built zero-copy
    /// via `PyArray1::from_vec` from the bench `Waveform.points()`.
    #[getter]
    fn values(&self, py: Python<'_>) -> PyResult<PyObject> {
        let vec: Vec<f64> = self.inner.points().iter().map(|&(_, v)| v).collect();
        Ok(numpy::PyArray1::from_vec(py, vec).into_any().unbind())
    }

    /// The axis (time, for `$tran`) as a real `np.ndarray` (PY-08 / spec
    /// AC7). Equal length to `.values`.
    #[getter]
    fn axis(&self, py: Python<'_>) -> PyResult<PyObject> {
        let vec: Vec<f64> = self.inner.points().iter().map(|&(t, _)| t).collect();
        Ok(numpy::PyArray1::from_vec(py, vec).into_any().unbind())
    }

    /// Linear interpolation at `x` (clamps outside range) — uniform-shape
    /// (bench `Waveform::at`).
    fn at(&self, x: f64) -> f64 {
        self.inner.at(x)
    }

    /// Time-weighted RMS over the recorded grid — uniform-shape (bench
    /// `Waveform::rms`).
    fn rms(&self) -> f64 {
        self.inner.rms()
    }

    /// Time-weighted mean over the recorded grid — uniform-shape.
    fn mean(&self) -> f64 {
        self.inner.mean()
    }

    /// Minimum sample value.
    fn min(&self) -> f64 {
        self.inner.min()
    }

    /// Maximum sample value.
    fn max(&self) -> f64 {
        self.inner.max()
    }

    /// `max - min` over the recorded grid.
    fn peak_to_peak(&self) -> f64 {
        self.inner.peak_to_peak()
    }

    /// First axis value where the waveform crosses `level` in direction
    /// `dir` (`"Rising"`/`"Falling"`/`"Either"`), or `None`. Uniform-shape
    /// (bench `Waveform::cross`).
    #[pyo3(signature = (level, dir="Either"))]
    fn cross(&self, level: f64, dir: &str) -> Option<f64> {
        self.inner.cross(level, dir)
    }

    /// Number of samples — equal to `.values` length.
    fn len(&self) -> usize {
        self.inner.len()
    }

    /// Python `len(waveform)` — same as [`len`](Self::len).
    fn __len__(&self) -> usize {
        self.inner.len()
    }

    /// `true` when there are no samples (spec edge case: an empty waveform
    /// exposes empty arrays, not None — `is_empty()` is the honest reflection).
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// `_ComplexWaveform` — the `$ac` sample surface (PY-09): a series of
/// `(frequency, Complex64)` samples. `.values` is a complex `np.ndarray`
/// (complex128); `.mag/.phase/.db` project onto real [`_Waveform`]s
/// (uniform-shape: same projections the bench `ComplexWaveform` computes).
/// `.axis` is the frequency grid as a real `np.ndarray` (mirrors
/// `_Waveform.axis`).
#[pyclass(module = "piperine", unsendable)]
pub struct _ComplexWaveform {
    inner: ComplexWaveform,
}

impl _ComplexWaveform {
    pub(crate) fn new(inner: ComplexWaveform) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl _ComplexWaveform {
    /// The complex values as a `np.ndarray` (complex128) — PY-09 / spec AC8.
    #[getter]
    fn values(&self, py: Python<'_>) -> PyResult<PyObject> {
        let vec: Vec<Complex64> = self.inner.points().iter().map(|&(_, v)| v).collect();
        Ok(numpy::PyArray1::from_vec(py, vec).into_any().unbind())
    }

    /// The frequency axis as a real `np.ndarray` (PY-09 / spec AC8). Equal
    /// length to `.values`.
    #[getter]
    fn axis(&self, py: Python<'_>) -> PyResult<PyObject> {
        let vec: Vec<f64> = self.inner.points().iter().map(|&(t, _)| t).collect();
        Ok(numpy::PyArray1::from_vec(py, vec).into_any().unbind())
    }

    /// Magnitude projection `|c|` per sample — uniform-shape (bench
    /// `ComplexWaveform::mag`). Returns a real `_Waveform`. Exposed as a
    /// property to match spec AC8 (`.mag` not `.mag()`).
    #[getter]
    fn mag(&self) -> _Waveform {
        _Waveform::new(self.inner.mag())
    }

    /// Phase projection `arg(c)` (radians) per sample — uniform-shape. Property
    /// per spec AC8.
    #[getter]
    fn phase(&self) -> _Waveform {
        _Waveform::new(self.inner.phase())
    }

    /// Decibel projection `20·log10|c|` per sample — uniform-shape. Property
    /// per spec AC8.
    #[getter]
    fn db(&self) -> _Waveform {
        _Waveform::new(self.inner.db())
    }

    /// Nearest sample to `x` (no complex interpolation) — uniform-shape.
    /// Returns a Python `complex`.
    fn at(&self, x: f64) -> Complex64 {
        self.inner.at(x)
    }

    /// Number of samples — equal to `.values` length.
    fn len(&self) -> usize {
        self.inner.len()
    }

    /// Python `len(waveform)` — same as [`len`](Self::len).
    fn __len__(&self) -> usize {
        self.inner.len()
    }

    /// `true` when there are no samples (spec edge case: empty → empty arrays).
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// `_AcTrace` — the typed `$ac(...)` result (PY-09). `.v(net)` returns a
/// [`_ComplexWaveform`] over the AC frequency sweep; `.axis()` returns the
/// frequency-axis real `_Waveform`. Net resolution + error handling mirror
/// `_OpResult::v`.
#[pyclass(module = "piperine", unsendable)]
pub struct _AcTrace {
    pub(crate) inner: AcTrace,
}

impl _AcTrace {
    pub(crate) fn new(inner: AcTrace) -> Self {
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
impl _AcTrace {
    /// Net voltage `a` minus `b` (ground-referenced when `b` is omitted) over
    /// the AC frequency sweep — spec AC8. An unaddressable net raises
    /// `KeyError` (fail loud).
    #[pyo3(signature = (a, b=None))]
    fn v(&self, a: &str, b: Option<&str>) -> PyResult<_ComplexWaveform> {
        let net_a = Self::net(a);
        let net_b = b.map(Self::net);
        let cw = self.inner.v(&net_a, net_b.as_ref()).map_err(readout_err)?;
        Ok(_ComplexWaveform::new(cw))
    }

    /// The frequency-axis `_Waveform` (spec AC8 `.axis()`).
    fn axis(&self) -> _Waveform {
        _Waveform::new(self.inner.axis())
    }
}

/// `_NoiseTrace` — the typed `$noise(...)` result (PY-10). `.psd()` returns
/// the output-referred noise PSD as a real `_Waveform` (V²/Hz over
/// frequency); `.total()` returns the integrated RMS noise as a float.
/// Uniform-shape: same values the bench `NoiseTrace` computes.
#[pyclass(module = "piperine", unsendable)]
pub struct _NoiseTrace {
    pub(crate) inner: NoiseTrace,
}

impl _NoiseTrace {
    pub(crate) fn new(inner: NoiseTrace) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl _NoiseTrace {
    /// Output-referred noise PSD as a real `_Waveform` (V²/Hz) — spec AC9.
    fn psd(&self) -> _Waveform {
        _Waveform::new(self.inner.psd())
    }

    /// Integrated total noise (RMS) — spec AC9.
    fn total(&self) -> f64 {
        self.inner.total()
    }
}
