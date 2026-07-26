//! # piperine-python
//!
//! Native PyO3 extension (`_piperine`) that exposes the Piperine host + POM
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
pub mod results;
pub mod scripted;
mod value_bridge;

use pyo3::prelude::*;

use design::{_Design, _Node, _Selection};
use instance::{_InstanceView, _ModelDescriptor, _ObservableDescriptor, _ParamDescriptor, _Terminal, _TerminalDescriptor};
use live::{_Grid, _Session, _Sweep};
use module::_Module;
use module::{_Behavior, _Instance, _Net, _Param, _Port};
use results::_AcTrace;
use results::_ComplexWaveform;
use results::_FourierComponent;
use results::_FourierResult;
use results::_NoiseTrace;
use results::_SolverStats;
use results::_LimitingReport;
use results::_NoiseContribution;
use results::_OpResult;
use results::_TfResult;
use results::_Trace;
use results::_Waveform;
use scripted::{_Ctx, _Staging};

/// `_piperine.load(path) -> _Design` (PY-01). Thin FFI shim delegating to
/// [`_Design::load`].
#[pyfunction]
fn load(path: &str) -> PyResult<_Design> {
    _Design::load(path)
}

/// `_piperine.load_str(src) -> _Design` (HOST-24). Thin FFI shim
/// delegating to [`_Design::load_str`] — elaborates `src` directly, no
/// filesystem read.
#[pyfunction]
fn load_str(src: &str) -> PyResult<_Design> {
    _Design::load_str(src)
}

/// The `_piperine` native extension module. Registered by the facade and, for
/// `piperine run`, appended to the embedded interpreter's init table
/// ([`embed::run_script`], PY-15).
#[pymodule]
pub fn _piperine(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(load, m)?)?;
    m.add_function(wrap_pyfunction!(load_str, m)?)?;
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
    m.add_class::<_FourierComponent>()?;
    m.add_class::<_FourierResult>()?;
    m.add_class::<_AcTrace>()?;
    m.add_class::<_NoiseTrace>()?;
    m.add_class::<_SolverStats>()?;
    m.add_class::<_LimitingReport>()?;
    m.add_class::<_NoiseContribution>()?;
    m.add_class::<_InstanceView>()?;
    m.add_class::<_Terminal>()?;
    m.add_class::<_ModelDescriptor>()?;
    m.add_class::<_TerminalDescriptor>()?;
    m.add_class::<_ObservableDescriptor>()?;
    m.add_class::<_ParamDescriptor>()?;
    m.add_class::<_Session>()?;
    m.add_class::<_Sweep>()?;
    m.add_class::<_Grid>()?;
    m.add_class::<_TfResult>()?;
    m.add_class::<_Ctx>()?;
    m.add_class::<_Staging>()?;
    Ok(())
}
