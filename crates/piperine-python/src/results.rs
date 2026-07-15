//! `_OpResult`/`_Trace`/`_AcTrace`/`_NoiseTrace` — typed Python wrappers
//! over the bench result objects (PY-06/07/09/10). P6 lands the shells so
//! [`crate::module::_Module::op`]/`tran`/`ac`/`noise` can return them; P7
//! adds `.v/.i/__getitem__` to `_OpResult`/`_Trace`, and P9 adds the AC/noise
//! readouts. Each wrapper owns its bench result by value (the result is
//! `'static`).
//!
//! MD-13 note: the wrappers are pyclasses — every function is a method on
//! the struct. No loose module-level functions.

use pyo3::prelude::*;

use piperine_bench::{AcTrace, NoiseTrace, OpResult, Trace};

/// `_OpResult` — the typed `$op()` result (PY-06). Holds the immutable DC
/// snapshot produced by [`piperine_bench::session::SimSession::run_op`].
/// `.v/.i` arrive in P7; the bench result is exposed `pub(crate)` so the
/// P6 stage-effect test can read it through the uniform bench readout before
/// the Python `.v()` exists.
#[pyclass(module = "piperine", unsendable)]
pub struct _OpResult {
    // Read by P7's `.v/.i/__getitem__` and the P6 stage-effect test (cfg(test));
    // `allow(dead_code)` keeps the non-test build warning-free until P7 lands.
    #[allow(dead_code)]
    pub(crate) inner: OpResult,
}

impl _OpResult {
    pub(crate) fn new(inner: OpResult) -> Self {
        Self { inner }
    }
}

/// `_Trace` — the typed `$tran(...)` result (PY-07). `.v/.i/.axis` arrive in
/// P7.
#[pyclass(module = "piperine", unsendable)]
pub struct _Trace {
    // Read by P7's `.v/.i/.axis/__getitem__`; `allow(dead_code)` until then.
    #[allow(dead_code)]
    pub(crate) inner: Trace,
}

impl _Trace {
    pub(crate) fn new(inner: Trace) -> Self {
        Self { inner }
    }
}

/// `_AcTrace` — the typed `$ac(...)` result (PY-09). `.v/.axis` arrive in P9.
#[pyclass(module = "piperine", unsendable)]
pub struct _AcTrace {
    // Read by P9's `.v/.axis`; `allow(dead_code)` until then.
    #[allow(dead_code)]
    pub(crate) inner: AcTrace,
}

impl _AcTrace {
    pub(crate) fn new(inner: AcTrace) -> Self {
        Self { inner }
    }
}

/// `_NoiseTrace` — the typed `$noise(...)` result (PY-10). `.psd/.total`
/// arrive in P9.
#[pyclass(module = "piperine", unsendable)]
pub struct _NoiseTrace {
    // Read by P9's `.psd/.total`; `allow(dead_code)` until then.
    #[allow(dead_code)]
    pub(crate) inner: NoiseTrace,
}

impl _NoiseTrace {
    pub(crate) fn new(inner: NoiseTrace) -> Self {
        Self { inner }
    }
}
