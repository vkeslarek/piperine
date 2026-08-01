//! T28 — `pip.Sequencer` (DVT-11 Python half, `ideal.md` §4/§6): a Python
//! generator drives [`Sequencer<PyProgram>`](piperine_api::Sequencer)
//! **unchanged** — `PyProgram` implements
//! [`SequencerProgram`](piperine_api::SequencerProgram) by calling
//! `.send(None)` on the Python generator, so every rule `driver.rs` already
//! enforces (resume strictly after acceptance/D11, the zero-advance bound,
//! the panic→finding conversion, T22's posture handling) applies to a
//! Python coroutine exactly as it does to a Rust one — nothing here
//! reimplements that machinery.
//!
//! `t` (the `SequencerControl` handle a running generator drives side
//! effects through) is `_SeqCtl`: a **persistent** pyclass (one per
//! program, not reconstructed per call, since a Python program is expected
//! to hold `t` across many `yield`s) whose raw pointer into the live
//! `&mut SequencerControl` is scoped to exactly one `.send()` call —
//! invalidated immediately after, the same discipline `device.rs` uses for
//! `StampSink`/`Ctx`.
//!
//! `t.advance(dt)` / `t.wait(edge(...))` / `t.wait(above(...))` are the
//! three wake forms (`ideal.md` §4.1), spelled identically to the Rust
//! `Wake` enum's shape. `pip.edge`/`pip.above` build the wait *specs*
//! `t.wait` dispatches on.

use pyo3::exceptions::PyStopIteration;
use pyo3::prelude::*;

use piperine_api::sequencer::EdgeDirection;
use piperine_api::sequencer::SequencerControl;
use piperine_api::{Sequencer, SequencerProgram, Wake};
use piperine_signal::events::CrossDir;
use piperine_solver::abi::{AnalogReference, DigitalNet};

use crate::device::_NetRef;

fn edge_dir(s: &str) -> PyResult<EdgeDirection> {
    match s {
        "rising" | "RISING" => Ok(EdgeDirection::Rising),
        "falling" | "FALLING" => Ok(EdgeDirection::Falling),
        "either" | "EITHER" => Ok(EdgeDirection::Either),
        other => Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
            "unknown edge direction {other:?} — use rising/falling/either"
        ))),
    }
}

fn cross_dir(s: &str) -> PyResult<CrossDir> {
    match s {
        "rising" | "RISING" => Ok(CrossDir::Rising),
        "falling" | "FALLING" => Ok(CrossDir::Falling),
        "either" | "EITHER" => Ok(CrossDir::Either),
        other => Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
            "unknown crossing direction {other:?} — use rising/falling/either"
        ))),
    }
}

/// An opaque digital-net handle (mirrors [`_NetRef`] for the digital
/// domain) — a Python device obtains one the same way a Rust `HostElement`
/// obtains a `DigitalNet`: from its resolved port bindings at construction.
#[pyclass(module = "piperine", name = "DigitalNetRef")]
#[derive(Clone)]
pub struct _DigitalNetRef(pub DigitalNet);

#[pymethods]
impl _DigitalNetRef {
    #[new]
    fn new(index: usize) -> Self {
        Self(DigitalNet(index))
    }
}

/// `pip.edge(net, direction)` — a `t.wait(...)` spec for a digital
/// settled-edge wake (T23's post-settle subscriber, DVT-27).
#[pyclass(module = "piperine", name = "EdgeSpec")]
#[derive(Clone)]
pub struct _EdgeSpec(DigitalNet, EdgeDirection);

/// `pip.edge(net, direction)` — a `t.wait(...)` spec for a digital settled-edge wake.
#[pyfunction]
#[pyo3(name = "edge")]
pub fn edge_fn(net: &_DigitalNetRef, direction: &str) -> PyResult<_EdgeSpec> {
    Ok(_EdgeSpec(net.0, edge_dir(direction)?))
}

/// `pip.above(net, level, direction="either")` — a `t.wait(...)` spec for
/// an analog tier-2 crossing wake (T24).
#[pyclass(module = "piperine", name = "CrossingSpec")]
#[derive(Clone)]
pub struct _CrossingSpec(AnalogReference, f64, CrossDir);

/// `pip.above(net, level, direction="either")` — a `t.wait(...)` spec for an analog crossing wake.
#[pyfunction]
#[pyo3(name = "above")]
#[pyo3(signature = (net, level, direction="either"))]
pub fn above_fn(net: &_NetRef, level: f64, direction: &str) -> PyResult<_CrossingSpec> {
    Ok(_CrossingSpec(net.0.clone(), level, cross_dir(direction)?))
}

/// What a Python generator yields — `t.advance`/`t.wait` build one of
/// these; [`PyProgram::step`] converts it to the Rust [`Wake`] the driver
/// already knows how to apply.
#[pyclass(module = "piperine", name = "Wake")]
#[derive(Clone)]
pub struct _PyWake(WakeKind);

#[derive(Clone)]
enum WakeKind {
    Advance(f64),
    Edge(DigitalNet, EdgeDirection),
    Crossing(AnalogReference, f64, CrossDir),
}

fn to_wake(kind: &WakeKind) -> Wake {
    match kind {
        WakeKind::Advance(dt) => Wake::Advance(*dt),
        WakeKind::Edge(net, dir) => Wake::WaitEdge(*net, *dir),
        WakeKind::Crossing(net, level, dir) => Wake::WaitCrossing(net.clone(), *level, *dir),
    }
}

/// `t` (DVT-25/29/31/33/34): the mutable handle a running Python generator
/// drives side effects through. One `_SeqCtl` is built per program and
/// reused across every `yield` — its pointer into the live
/// `&mut SequencerControl` is valid only for the duration of one
/// `PyProgram::step` call (scoped identically to `device.rs`'s wrappers).
#[pyclass(unsendable, module = "piperine", name = "SeqCtl")]
pub struct _SeqCtl {
    ptr: usize,
    /// The instant `t` was last resumed at (`0.0` before the first step —
    /// `SequencerProgram::start` itself carries no time, the same
    /// limitation the underlying Rust trait has).
    current_t: f64,
    instance: String,
}

fn ctl_mut(ptr: usize) -> PyResult<&'static mut SequencerControl> {
    if ptr == 0 {
        return Err(pyo3::exceptions::PyRuntimeError::new_err(
            "this sequencer control (`t`) is only valid for the duration of the resume that handed \
             it a yield point — do not call its methods from outside a `run(self, t)` generator step",
        ));
    }
    // SAFETY: non-zero only within `PyProgram::step`'s call, which
    // invalidates it (`ptr = 0`) immediately after `.send()` returns.
    Ok(unsafe { &mut *(ptr as *mut SequencerControl) })
}

#[pymethods]
impl _SeqCtl {
    /// `t.advance(dt)` — wake at `dt` from now.
    fn advance(&self, dt: f64) -> _PyWake {
        _PyWake(WakeKind::Advance(dt))
    }

    /// `t.wait(spec)` — `spec` is a [`_EdgeSpec`] (from `pip.edge`) or a
    /// [`_CrossingSpec`] (from `pip.above`).
    fn wait(&self, spec: &Bound<'_, PyAny>) -> PyResult<_PyWake> {
        if let Ok(e) = spec.extract::<_EdgeSpec>() {
            return Ok(_PyWake(WakeKind::Edge(e.0, e.1)));
        }
        if let Ok(c) = spec.extract::<_CrossingSpec>() {
            return Ok(_PyWake(WakeKind::Crossing(c.0, c.1, c.2)));
        }
        Err(pyo3::exceptions::PyTypeError::new_err(
            "t.wait(...) expects a pip.edge(...) or pip.above(...) spec",
        ))
    }

    /// `t.expect(cond, msg)` (DVT-29): a failing `cond` records an `Error`
    /// finding with `t`/instance provenance — never a print.
    fn expect(&self, cond: bool, msg: &str) -> PyResult<()> {
        ctl_mut(self.ptr)?.expect(cond, msg.to_string(), self.current_t, &self.instance);
        Ok(())
    }

    /// `t.warn(msg)` — always records a `Warning`, never aborts.
    fn warn(&self, msg: &str) -> PyResult<()> {
        ctl_mut(self.ptr)?.warn(msg.to_string(), self.current_t, &self.instance);
        Ok(())
    }

    /// The current simulated time, as of the most recent resume.
    #[getter]
    fn t(&self) -> f64 {
        self.current_t
    }
}

/// [`SequencerProgram`] over a Python generator (`obj.run(t)`): `start`/
/// `resume` both funnel through [`Self::step`], which lazily creates the
/// generator on first use (calling a Python generator function does not
/// run its body — only `.send`/`next` does, so `start` and any later
/// `resume` are the same operation: "advance the generator once").
pub struct PyProgram {
    device: Py<PyAny>,
    ctl: Py<_SeqCtl>,
    generator: Option<Py<PyAny>>,
}

impl PyProgram {
    /// `label` is the sequencer's own name — threaded into `t.expect`/
    /// `warn`'s instance provenance, matching every other `HostElement`'s
    /// findings.
    pub fn new(py: Python<'_>, device: Py<PyAny>, label: String) -> PyResult<Self> {
        let ctl = Py::new(py, _SeqCtl { ptr: 0, current_t: 0.0, instance: label })?;
        Ok(Self { device, ctl, generator: None })
    }

    fn step(&mut self, t: f64, ctl: &mut SequencerControl) -> Wake {
        Python::with_gil(|py| {
            {
                let mut c = self.ctl.borrow_mut(py);
                c.current_t = t;
                c.ptr = ctl as *mut SequencerControl as usize;
            }
            let generator_ref = match &self.generator {
                Some(g) => g.clone_ref(py),
                None => {
                    let g = match self.device.bind(py).call_method1("run", (self.ctl.clone_ref(py),)) {
                        Ok(g) => g.unbind(),
                        Err(e) => panic!("sequencer `run(self, t)` raised before yielding: {e}"),
                    };
                    self.generator = Some(g.clone_ref(py));
                    g
                }
            };
            let result = generator_ref.bind(py).call_method1("send", (py.None(),));
            self.ctl.borrow_mut(py).ptr = 0;
            match result {
                Ok(yielded) => match yielded.extract::<_PyWake>() {
                    Ok(w) => to_wake(&w.0),
                    Err(_) => panic!(
                        "sequencer `run(self, t)` yielded something other than `t.advance(...)`/`t.wait(...)`"
                    ),
                },
                Err(e) if e.is_instance_of::<PyStopIteration>(py) => Wake::Done,
                Err(e) => panic!("{e}"),
            }
        })
    }
}

impl SequencerProgram for PyProgram {
    fn start(&mut self, ctl: &mut SequencerControl) -> Wake {
        self.step(0.0, ctl)
    }
    fn resume(&mut self, t: f64, ctl: &mut SequencerControl) -> Wake {
        self.step(t, ctl)
    }
}

/// Test/script seam (T28): drive `obj.run(t)` through exactly two steps —
/// `start()` then `resume(t1, ...)` — and report the two wakes' `Advance`
/// durations plus whatever `t.expect`/`t.warn` recorded, as plain Python
/// values a script (the project's established test idiom) can assert on
/// without needing `SequencerControl`/`PyProgram` themselves, which are
/// not part of the public FFI surface.
/// One recorded finding's `(message, time, severity)`, as plain Python
/// values.
type FindingTuple = (String, f64, String);

#[pyfunction]
pub fn _drive_sequencer_twice(
    py: Python<'_>,
    obj: Py<PyAny>,
    t1: f64,
) -> PyResult<(f64, f64, Option<FindingTuple>)> {
    let mut program = PyProgram::new(py, obj, "t0".to_string())?;
    let mut ctl = SequencerControl::default();
    let advance_of = |w: Wake| -> f64 {
        match w {
            Wake::Advance(dt) => dt,
            other => panic!("expected Wake::Advance, got a different wake: {other:?}"),
        }
    };
    let first = advance_of(SequencerProgram::start(&mut program, &mut ctl));
    let second = advance_of(SequencerProgram::resume(&mut program, t1, &mut ctl));
    let finding = ctl.findings().to_report().map(|r| {
        let f = &r.findings()[0];
        (f.message.clone(), f.time, format!("{:?}", f.severity))
    });
    Ok((first, second, finding))
}

/// Build a `Sequencer<PyProgram>` (T28) — the Python-generator twin of any
/// Rust `Sequencer::new(...)` call site; wrapped in `HostAdapter` by the
/// caller exactly like any other `HostElement`.
pub fn build_python_sequencer(
    py: Python<'_>,
    label: impl Into<String>,
    device: Py<PyAny>,
    start_time: f64,
    stop_time: f64,
) -> PyResult<Sequencer<PyProgram>> {
    let label = label.into();
    let program = PyProgram::new(py, device, label.clone())?;
    Ok(Sequencer::new(label, program, start_time, stop_time))
}
