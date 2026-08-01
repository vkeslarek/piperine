//! T27 — the PyO3 bridge (DVT-22): a Python object implementing `pip.Device`
//! (duck typing — only the methods it defines are dispatched, mirroring
//! `HostElement`'s own defaulted shape) is wrapped as a `HostElement` and
//! presented to the solver through the existing `HostAdapter` blanket
//! (T14), exactly like a Rust host device.
//!
//! ## Dispatch on method presence
//!
//! [`PyHostElement::new`] probes `hasattr` once, at construction — never per
//! call. A Python class that defines only `load_dc` pays for exactly that;
//! every other `HostElement` method stays at its inert default.
//!
//! ## The one catch point
//!
//! Every dispatched Python call funnels through [`PyHostElement::call`].
//! Python raising an exception becomes a [`PySolveError`] (the message plus
//! the simulation time it happened at) and is propagated as a Rust panic —
//! never a `PyErr` threaded back through `HostElement`'s infallible ABI
//! (`load_dc`/`accept_timestep`/... return `()`, not `Result`). The single
//! place that catches it is [`run_guarded`]: an analysis driver wraps the
//! whole solve in it, so the panic never unwinds uncontrolled through
//! solver internals — the analysis stops cleanly with one typed error,
//! never scattered `try`/`except`-shaped recovery at each dispatch site.
//!
//! ## GIL cost — a property of the Python tier specifically
//!
//! Every dispatched call here acquires the GIL (`Python::with_gil`). For a
//! device that overrides `load_dc`/`load_transient`, that cost lands **once
//! per Newton iteration** — the same ceiling `ideal.md` §5's cost table
//! states for "any host call inside Newton". It is a cost of *this* bridge,
//! not of the host-device mechanism in general: `piperine-signal`'s level-1
//! catalog (`Wave`/`Behavior`/`Measure`, exposed unchanged as `pip.*` — see
//! `device.rs`'s sibling in the facade) evaluates natively in Rust once
//! installed, so a Python device that reaches for `pip.Wave`/`pip.Behavior`
//! instead of overriding `load_dc` itself pays **zero** host calls inside
//! Newton, exactly as the Rust path does.

use std::panic::UnwindSafe;
use std::sync::Mutex;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use piperine_api::device::{BehaviorStamp, Ctx, HostElement, StampSink, StateView, WaveStamp};
use piperine_solver::abi::{AnalogReference, ElementCapabilities};

use crate::signal_bridge::_Builder;

/// A Python exception raised from inside a dispatched `HostElement` method
/// (DVT-22): the message plus the simulation time it happened at. The one
/// typed error this bridge ever produces — never a bare `PyErr` threaded
/// through solver internals.
#[derive(Debug, Clone)]
pub struct PySolveError {
    pub message: String,
    pub time: f64,
}

impl std::fmt::Display for PySolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "python device raised at t={}: {}", self.time, self.message)
    }
}
impl std::error::Error for PySolveError {}

/// Run `f`, catching a [`PySolveError`] panic raised anywhere underneath it
/// (the one catch point, DVT-22) and turning it back into a typed
/// `Result` — the analysis stops cleanly instead of unwinding past its
/// caller. Any *other* panic (a genuine Rust bug, not a Python exception)
/// is re-raised unchanged; this function only ever intercepts the payload
/// shape it itself produces.
pub fn run_guarded<T>(f: impl FnOnce() -> T + UnwindSafe) -> Result<T, PySolveError> {
    match std::panic::catch_unwind(f) {
        Ok(v) => Ok(v),
        Err(payload) => match payload.downcast::<PySolveError>() {
            Ok(err) => Err(*err),
            Err(other) => std::panic::resume_unwind(other),
        },
    }
}

/// Which optional `HostElement` methods the wrapped Python object actually
/// defines — probed once (`hasattr`) at construction, never per call
/// (duck-typed dispatch on presence, DVT-22).
#[derive(Default, Clone, Copy)]
struct Methods {
    load_dc: bool,
    load_transient: bool,
    accept_timestep: bool,
}

/// A Python object bridged as a [`HostElement`]. `name`/`caps` are read
/// once at construction (`name()`/`caps` mirror the two methods `Element`
/// requires unconditionally); every other method dispatches only when
/// [`Methods`] says the Python object defines it.
pub struct PyHostElement {
    obj: Py<PyAny>,
    name: String,
    caps: ElementCapabilities,
    methods: Methods,
    /// The simulation time of the call currently in flight — read by
    /// [`Self::call`] if the dispatched method raises, so the resulting
    /// [`PySolveError`] carries it.
    current_t: Mutex<f64>,
    /// `build(self, b)`'s installed `Wave`/`Behavior` stamps (T28) —
    /// collected **once**, at construction, and stamped by this struct's
    /// own Rust `load_dc`/`load_transient` below, never by dispatching
    /// back into Python. This is what makes a Python device's level-1
    /// catalog usage genuinely zero-host-call inside Newton, not merely
    /// spelled the same as Rust's.
    waves: Vec<WaveStamp>,
    behaviors: Vec<BehaviorStamp>,
}

impl PyHostElement {
    /// Build a bridge over `obj`. `obj.name` (optional, falls back to the
    /// Python class name) and `obj.caps` (optional, falls back to no
    /// capabilities — a device that declares none does nothing, safely)
    /// are read once here, never per call. If `obj` defines `build(self,
    /// b)`, it is called exactly once, here, and its installed `Wave`/
    /// `Behavior` stamps become native Rust state (T28).
    pub fn new(py: Python<'_>, obj: Py<PyAny>) -> PyResult<Self> {
        let bound = obj.bind(py);
        let name: String = match bound.getattr("name") {
            Ok(v) if !v.is_none() => v.extract()?,
            _ => bound.get_type().name()?.to_string(),
        };
        let caps = match bound.getattr("caps") {
            Ok(v) if !v.is_none() => ElementCapabilities::from_bits_truncate(v.extract()?),
            _ => ElementCapabilities::empty(),
        };
        let methods = Methods {
            load_dc: bound.hasattr("load_dc")?,
            load_transient: bound.hasattr("load_transient")?,
            accept_timestep: bound.hasattr("accept_timestep")?,
        };
        let (waves, behaviors) = if bound.hasattr("build")? {
            let builder = Py::new(py, _Builder::default())?;
            bound.call_method1("build", (builder.clone_ref(py),))?;
            let mut b = builder.borrow_mut(py);
            (std::mem::take(&mut b.waves), std::mem::take(&mut b.behaviors))
        } else {
            (Vec::new(), Vec::new())
        };
        Ok(Self { obj, name, caps, methods, current_t: Mutex::new(0.0), waves, behaviors })
    }

    /// The one catch point (DVT-22 — see this module's header). `f` runs
    /// with the GIL held; a `PyErr` it returns becomes a [`PySolveError`]
    /// carrying `self.current_t` and is propagated as a panic, caught only
    /// by [`run_guarded`].
    fn call<R>(&self, f: impl FnOnce(Python<'_>, &Bound<'_, PyAny>) -> PyResult<R>) -> R {
        Python::with_gil(|py| {
            let bound = self.obj.bind(py);
            match f(py, bound) {
                Ok(r) => r,
                Err(e) => {
                    let message = e.to_string();
                    let time = *self.current_t.lock().unwrap();
                    std::panic::panic_any(PySolveError { message, time });
                }
            }
        })
    }

    fn dispatch_stamp(&self, method: &'static str, ctx: &Ctx<'_>, sink: &mut StampSink) {
        *self.current_t.lock().unwrap() = ctx.t();
        // SAFETY: `ctx`/`sink` are borrowed for exactly the duration of
        // this call (T15's contract). The pyclasses below hold raw
        // pointers erased to `usize` and are invalidated (`ptr = 0`)
        // before this function returns, so nothing a Python object
        // retains past the call can dereference a stale pointer without
        // hitting the `ptr == 0` guard first.
        let ctx_ptr = ctx as *const Ctx<'_> as usize;
        let sink_ptr = sink as *mut StampSink as usize;
        self.call(|py, obj| {
            let py_ctx = Py::new(py, _DeviceCtx { ptr: ctx_ptr })?;
            let py_sink = Py::new(py, _DeviceStampSink { ptr: sink_ptr })?;
            obj.call_method1(method, (py_ctx.clone_ref(py), py_sink.clone_ref(py)))?;
            py_ctx.borrow_mut(py).ptr = 0;
            py_sink.borrow_mut(py).ptr = 0;
            Ok(())
        })
    }
}

impl HostElement for PyHostElement {
    fn name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> ElementCapabilities {
        self.caps
    }

    fn load_dc(&mut self, ctx: &Ctx<'_>, sink: &mut StampSink) {
        for w in &self.waves {
            w.stamp(ctx, sink);
        }
        for b in &self.behaviors {
            b.stamp(ctx, sink);
        }
        if self.methods.load_dc {
            self.dispatch_stamp("load_dc", ctx, sink);
        }
    }

    fn load_transient(&mut self, ctx: &Ctx<'_>, sink: &mut StampSink) {
        for w in &self.waves {
            w.stamp(ctx, sink);
        }
        for b in &self.behaviors {
            b.stamp(ctx, sink);
        }
        if self.methods.load_transient {
            self.dispatch_stamp("load_transient", ctx, sink);
        }
    }

    fn next_breakpoints(&self, from: f64, horizon: f64) -> Vec<f64> {
        self.waves.iter().flat_map(|w| w.breakpoints(from, horizon)).collect()
    }

    fn accept_timestep(&mut self, state: &StateView<'_>, t: f64) {
        if !self.methods.accept_timestep {
            return;
        }
        *self.current_t.lock().unwrap() = t;
        // SAFETY: see `dispatch_stamp` — same scoped-pointer, invalidate-
        // after-call discipline.
        let state_ptr = state as *const StateView<'_> as usize;
        self.call(|py, obj| {
            let py_state = Py::new(py, _DeviceStateView { ptr: state_ptr })?;
            obj.call_method1("accept_timestep", (py_state.clone_ref(py), t))?;
            py_state.borrow_mut(py).ptr = 0;
            Ok(())
        })
    }
}

/// An opaque, `Clone`-able net handle passed to a Python device's
/// constructor (mirrors a Rust `HostElement`'s `AnalogReference` fields) —
/// safe to retain for the device's lifetime, unlike the scoped wrappers
/// below.
#[pyclass(module = "piperine")]
#[derive(Clone)]
pub struct _NetRef(pub AnalogReference);

fn stale_err(what: &str) -> PyErr {
    PyRuntimeError::new_err(format!(
        "{what} is only valid for the duration of the call that handed it out — do not retain it"
    ))
}

/// Python-facing `StampSink` (T15's FFI boundary): a raw pointer scoped to
/// one `load_dc`/`load_transient` call, invalidated (`ptr = 0`)
/// immediately after that call returns.
#[pyclass(unsendable, module = "piperine")]
pub struct _DeviceStampSink {
    ptr: usize,
}

#[pymethods]
impl _DeviceStampSink {
    fn g(&self, row: &_NetRef, col: &_NetRef, value: f64) -> PyResult<()> {
        if self.ptr == 0 {
            return Err(stale_err("this stamp sink"));
        }
        // SAFETY: non-zero only within the call that constructed it (see
        // `PyHostElement::dispatch_stamp`).
        let sink = unsafe { &mut *(self.ptr as *mut StampSink) };
        sink.g(&row.0, &col.0, value);
        Ok(())
    }

    fn i(&self, row: &_NetRef, value: f64) -> PyResult<()> {
        if self.ptr == 0 {
            return Err(stale_err("this stamp sink"));
        }
        let sink = unsafe { &mut *(self.ptr as *mut StampSink) };
        sink.i(&row.0, value);
        Ok(())
    }
}

/// Python-facing `Ctx` (T15): scoped identically to [`_DeviceStampSink`].
#[pyclass(unsendable, module = "piperine")]
pub struct _DeviceCtx {
    ptr: usize,
}

#[pymethods]
impl _DeviceCtx {
    fn v(&self, r: &_NetRef) -> PyResult<f64> {
        if self.ptr == 0 {
            return Err(stale_err("this ctx"));
        }
        let ctx = unsafe { &*(self.ptr as *const Ctx<'_>) };
        Ok(ctx.v(&r.0))
    }

    fn t(&self) -> PyResult<f64> {
        if self.ptr == 0 {
            return Err(stale_err("this ctx"));
        }
        Ok(unsafe { &*(self.ptr as *const Ctx<'_>) }.t())
    }

    fn dt(&self) -> PyResult<f64> {
        if self.ptr == 0 {
            return Err(stale_err("this ctx"));
        }
        Ok(unsafe { &*(self.ptr as *const Ctx<'_>) }.dt())
    }
}

/// Python-facing `StateView` (T15): scoped identically to
/// [`_DeviceStampSink`], handed to `accept_timestep`.
#[pyclass(unsendable, module = "piperine")]
pub struct _DeviceStateView {
    ptr: usize,
}

#[pymethods]
impl _DeviceStateView {
    fn v(&self, r: &_NetRef) -> PyResult<f64> {
        if self.ptr == 0 {
            return Err(stale_err("this state view"));
        }
        let state = unsafe { &*(self.ptr as *const StateView<'_>) };
        Ok(state.v(&r.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use piperine_api::device::HostAdapter;
    use piperine_solver::abi::{AnalogVariable, Context, DcAnalysisState, NodeIdentifier};
    use piperine_solver::prelude::CircuitBuilder;
    use pyo3::types::PyModule;
    use std::ffi::CString;
    use std::panic::AssertUnwindSafe;
    use std::sync::Arc;

    fn node(i: usize) -> AnalogReference {
        AnalogReference::new(Arc::new(AnalogVariable::Node(NodeIdentifier::Anonymous(i))), i)
    }

    /// Build a Python class object from inline source (no facade import —
    /// T27 is tested against the raw bridge, independent of T28's Python
    /// surface).
    fn make_class<'py>(py: Python<'py>, src: &str, attr: &str) -> Bound<'py, PyAny> {
        let src = CString::new(src).unwrap();
        let module = PyModule::from_code(
            py,
            &src,
            &CString::new("t27_fixture.py").unwrap(),
            &CString::new("t27_fixture").unwrap(),
        )
        .unwrap();
        module.getattr(attr).unwrap()
    }

    const R_SRC: &str = "\
class R:
    caps = 1 | 8
    def __init__(self, p, n, r):
        self.p, self.n, self.g = p, n, 1.0 / r
    def load_dc(self, ctx, m):
        m.g(self.p, self.p, self.g); m.g(self.p, self.n, -self.g)
        m.g(self.n, self.p, -self.g); m.g(self.n, self.n, self.g)
";

    /// DVT-22: a Python class implementing only `load_dc` is dispatched on
    /// method presence — `load_transient`/`accept_timestep` stay at their
    /// inert defaults, never called, never erroring for being absent.
    #[test]
    fn dispatch_is_on_method_presence_only() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let cls = make_class(py, R_SRC, "R");
            let p = Py::new(py, _NetRef(node(0))).unwrap();
            let n = Py::new(py, _NetRef(node(1))).unwrap();
            let obj = cls.call1((p, n, 1e3)).unwrap();
            let bridge = PyHostElement::new(py, obj.unbind()).unwrap();
            assert!(bridge.methods.load_dc, "load_dc is defined");
            assert!(!bridge.methods.load_transient, "load_transient is not defined on this class");
            assert!(!bridge.methods.accept_timestep, "accept_timestep is not defined on this class");
        });
    }

    /// DVT-22: a Python exception raised from `load_dc` is caught at the
    /// one catch point and surfaces through `run_guarded` as a typed
    /// `PySolveError` carrying the simulation time it happened at. Driven
    /// through a real circuit solve (`Ctx`/`StampSink` are `pub(crate)` to
    /// `piperine-api`, so this exercises the bridge exactly as a live
    /// solve would, rather than constructing them directly).
    #[test]
    fn a_python_exception_mid_solve_becomes_a_typed_error_with_simulation_time() {
        pyo3::prepare_freethreaded_python();
        const BOOM_SRC: &str = "\
class Boom:
    caps = 1 | 8
    def load_dc(self, ctx, m):
        raise ValueError('deliberate failure')
";
        let mut builder = CircuitBuilder::new("boom");
        let gnd = builder.ground();
        let a = builder.node("a");
        Python::with_gil(|py| {
            let cls = make_class(py, BOOM_SRC, "Boom");
            let obj = cls.call0().unwrap();
            let bridge = PyHostElement::new(py, obj.unbind()).unwrap();
            builder.element(Box::new(HostAdapter(bridge)));
        });
        let _ = &a;
        let _ = &gnd;
        let mut circuit = builder.build().expect("builds despite the device that will raise on load_dc");

        let result = run_guarded(AssertUnwindSafe(|| {
            let mut dc = circuit.dc(Context::default()).expect("dc setup");
            dc.solve()
        }));
        let err = result.expect_err("a raised Python exception must surface as an Err, not unwind past run_guarded");
        assert_eq!(err.time, 0.0, "DC's evaluation coordinate is t=0 — the typed error carries it");
        assert!(err.message.contains("deliberate failure"), "the message names the Python exception: {}", err.message);
    }

    /// DVT-22: a Python device and a Rust device coexist in one circuit,
    /// stamping the same real solve — 3k/2k divider from 5V, one leg Rust
    /// (`HostAdapter<HostResistor>`), the other Python (`PyHostElement`).
    #[test]
    fn a_python_device_and_a_rust_device_coexist_in_one_circuit() {
        pyo3::prepare_freethreaded_python();

        struct HostResistor {
            p: AnalogReference,
            n: AnalogReference,
            g: f64,
        }
        impl HostElement for HostResistor {
            fn name(&self) -> &str {
                "rust_r"
            }
            fn capabilities(&self) -> ElementCapabilities {
                ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC
            }
            fn load_dc(&mut self, _ctx: &Ctx<'_>, sink: &mut StampSink) {
                sink.g(&self.p, &self.p, self.g);
                sink.g(&self.p, &self.n, -self.g);
                sink.g(&self.n, &self.p, -self.g);
                sink.g(&self.n, &self.n, self.g);
            }
        }

        let mut builder = CircuitBuilder::new("py_and_rust_divider");
        let gnd = builder.ground();
        let vin = builder.node("vin");
        let mid = builder.node("mid");

        struct Vsrc {
            p: AnalogReference,
            n: AnalogReference,
            branch: Option<AnalogReference>,
            v: f64,
        }
        impl piperine_solver::abi::AnalogDevice for Vsrc {
            fn allocate_unknowns(&mut self, alloc: &mut piperine_solver::abi::UnknownAllocator<'_>) {
                self.branch = Some(alloc.branch("vsrc", "i"));
            }
            fn load_dc(
                &mut self,
                _state: &DcAnalysisState<'_>,
                _ctx: &Context,
            ) -> Vec<piperine_solver::abi::Stamp<AnalogReference, f64>> {
                let b = self.branch.clone().unwrap();
                use piperine_solver::abi::Stamp;
                vec![
                    Stamp::Matrix(self.p.clone(), b.clone(), 1.0),
                    Stamp::Matrix(self.n.clone(), b.clone(), -1.0),
                    Stamp::Matrix(b.clone(), self.p.clone(), 1.0),
                    Stamp::Matrix(b.clone(), self.n.clone(), -1.0),
                    Stamp::Rhs(b, self.v),
                ]
            }
        }
        impl piperine_solver::abi::DigitalDevice for Vsrc {}
        impl piperine_solver::abi::Introspect for Vsrc {}
        impl piperine_solver::abi::Element for Vsrc {
            fn name(&self) -> &str {
                "vsrc"
            }
            fn capabilities(&self) -> ElementCapabilities {
                ElementCapabilities::ANALOG
                    | ElementCapabilities::LOADS_DC
                    | ElementCapabilities::HAS_INTERNAL_UNKNOWNS
            }
        }

        builder.element(Box::new(Vsrc { p: vin.clone(), n: gnd.clone(), branch: None, v: 5.0 }));
        builder.element(Box::new(HostAdapter(HostResistor { p: vin.clone(), n: mid.clone(), g: 1.0 / 3e3 })));

        Python::with_gil(|py| {
            let cls = make_class(py, R_SRC, "R");
            let p = Py::new(py, _NetRef(mid.clone())).unwrap();
            let n = Py::new(py, _NetRef(gnd.clone())).unwrap();
            let obj = cls.call1((p, n, 2e3)).unwrap();
            let bridge = PyHostElement::new(py, obj.unbind()).unwrap();
            builder.element(Box::new(HostAdapter(bridge)));
        });

        let mut circuit = builder.build().expect("a python device builds into a real circuit alongside a rust one");
        let mut dc = circuit.dc(Context::default()).expect("dc setup");
        let result = run_guarded(AssertUnwindSafe(|| dc.solve()))
            .expect("no python exception fired")
            .expect("dc solves with both a python and a rust host device stamping it");
        let v_mid = result.get(mid.clone()).expect("mid is a solved unknown");
        assert!(
            (v_mid - 2.0).abs() < 1e-9,
            "a 3k(rust)/2k(python) divider from 5V gives 2.0V at mid, got {v_mid}"
        );
    }
}
