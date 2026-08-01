//! T28 — the Python surface's level-1 catalog (DVT-11 Python half):
//! `pip.Wave`/`pip.Behavior`/`pip.Measure`, reachable with **identical
//! spelling** to `piperine-signal`'s Rust catalog (`ideal.md` §3.7), and
//! **closed** — there is no Python-side way to add a new kind. `Wave.const`
//! is the one deliberate name divergence: `const` is a Rust keyword, so the
//! Rust catalog spells it `constant` (same precedent as `Design::const_` /
//! `Ctx.const`, `value_bridge.rs`).
//!
//! `_Builder` (`pip`'s `b` in `def build(self, b): …`) is how a Python
//! device reaches zero-host-call level 1: `b.wave(...)`/`b.behavior(...)`
//! install a [`WaveStamp`]/[`BehaviorStamp`] **once**, at construction —
//! [`PyHostElement`](crate::device::PyHostElement)'s own Rust `load_dc`/
//! `load_transient` stamp them natively afterward, never dispatching back
//! into Python for it. This is what makes `ideal.md` examples B/C/D's
//! "zero host calls" claim true from Python, not just from Rust.
//!
//! `pip.Measure` (T11/T13's lifecycle, Python half) is a standalone armed
//! handle — `Measure.peak()` etc. **arms** immediately (T13's "arm creates
//! a fresh accumulator" rule), matching the Rust `Measure::arm` contract
//! 1:1. A Python device feeds it from its own `accept_timestep` (level 0
//! cost — `t.arm(...)`-style automatic per-net feeding from inside a
//! running `Sequencer` coroutine is not part of this batch's delivered
//! Rust mechanism either: `SequencerControl` has no `Measure` slot to feed
//! today, so this is named here as a scoped gap rather than a silent
//! omission).

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use piperine_api::device::{BehaviorStamp, WaveStamp};
use piperine_signal::behavior::kinds::{
    Comparator, Hysteresis, Limiter as BehaviorLimiter, Scaled as BScaled, Sum as BSum, Switch, Vccs, Vco, Vcvs,
};
use piperine_signal::behavior::table::Table;
use piperine_signal::behavior::BehaviorKind;
use piperine_signal::measure::handle::Measure;
use piperine_signal::measure::kinds::{Average, Count, Cross, Delay, Integral, Min, Peak, Ripple, Rms, Settle};
use piperine_signal::measure::MeasureKind;
use piperine_signal::wave::Wave;

use crate::device::_NetRef;

/// `Wave` has no `Clone` impl upstream (its `FromFn` arm's `Arc<dyn Fn>`
/// *is* cloneable, but the derive was never added; `piperine-signal` is
/// frozen for this batch, MD-13-adjacent scope rule). Every other arm is
/// plain data, so a manual, exhaustive clone is correct and cheap. A free
/// function, not an inherent impl, because `Wave` is a foreign type here
/// (Rust's orphan rule).
fn clone_wave(w: &Wave) -> Wave {
    match w {
        Wave::Const { v } => Wave::Const { v: *v },
        Wave::Ramp { v0, v1, dur } => Wave::Ramp { v0: *v0, v1: *v1, dur: *dur },
        Wave::Pulse { lo, hi, delay, rise, width, fall, period } => {
            Wave::Pulse { lo: *lo, hi: *hi, delay: *delay, rise: *rise, width: *width, fall: *fall, period: *period }
        }
        Wave::Sin { offset, amp, freq, phase, damping } => {
            Wave::Sin { offset: *offset, amp: *amp, freq: *freq, phase: *phase, damping: *damping }
        }
        Wave::Exp { v0, v1, tau, delay } => Wave::Exp { v0: *v0, v1: *v1, tau: *tau, delay: *delay },
        Wave::Sigmoid { lo, hi, t0, tau } => Wave::Sigmoid { lo: *lo, hi: *hi, t0: *t0, tau: *tau },
        Wave::Pwl { points } => Wave::Pwl { points: points.clone() },
        Wave::Sampled { t, v } => Wave::Sampled { t: t.clone(), v: v.clone() },
        Wave::FromFn(f) => Wave::FromFn(f.clone()),
        Wave::Sum(a, b) => Wave::Sum(Box::new(clone_wave(a)), Box::new(clone_wave(b))),
        Wave::Scaled(w, k) => Wave::Scaled(Box::new(clone_wave(w)), *k),
        Wave::Shifted(w, dt) => Wave::Shifted(Box::new(clone_wave(w)), *dt),
    }
}

// ── Wave ─────────────────────────────────────────────────────────────────

/// `pip.Wave` (DVT-07/08 Python half): the closed value-from-time catalog.
/// Every kind but `from_fn` evaluates natively in Rust — zero host calls —
/// once installed via `b.wave(...)`.
#[pyclass(module = "piperine", name = "Wave")]
pub struct _Wave(pub Wave);

impl _Wave {
    fn wave_clone(&self) -> Wave {
        clone_wave(&self.0)
    }
}

#[pymethods]
impl _Wave {
    /// `Wave.const(v)` — a constant. (Rust: `Wave::constant`, `const` being
    /// a Rust keyword — the one deliberate spelling divergence in this
    /// catalog.)
    #[staticmethod]
    #[pyo3(name = "const")]
    fn const_(v: f64) -> Self {
        Self(Wave::constant(v))
    }
    /// `Wave.ramp(v0, v1, dur)` — linear from `v0` to `v1` over `dur`.
    #[staticmethod]
    fn ramp(v0: f64, v1: f64, dur: f64) -> Self {
        Self(Wave::ramp(v0, v1, dur))
    }
    /// `Wave.pulse(lo, hi, delay, rise, width, fall, period)` — the ngspice-faithful PULSE shape.
    #[staticmethod]
    #[allow(clippy::too_many_arguments)]
    fn pulse(lo: f64, hi: f64, delay: f64, rise: f64, width: f64, fall: f64, period: f64) -> Self {
        Self(Wave::pulse(lo, hi, delay, rise, width, fall, period))
    }
    /// `Wave.sin(offset, amp, freq, phase, damping)` — a damped sinusoid.
    #[staticmethod]
    fn sin(offset: f64, amp: f64, freq: f64, phase: f64, damping: f64) -> Self {
        Self(Wave::sin(offset, amp, freq, phase, damping))
    }
    /// `Wave.exp(v0, v1, tau, delay)` — a single-stage exponential approach.
    #[staticmethod]
    fn exp(v0: f64, v1: f64, tau: f64, delay: f64) -> Self {
        Self(Wave::exp(v0, v1, tau, delay))
    }
    /// `Wave.sigmoid(lo, hi, t0, tau)` — a logistic S-curve.
    #[staticmethod]
    fn sigmoid(lo: f64, hi: f64, t0: f64, tau: f64) -> Self {
        Self(Wave::sigmoid(lo, hi, t0, tau))
    }
    /// `Wave.pwl([(t, v), ...])` — piecewise-linear, authored inline.
    #[staticmethod]
    fn pwl(points: Vec<(f64, f64)>) -> Self {
        Self(Wave::pwl(points))
    }
    /// `Wave.sampled(t, v)` — the same zero-host-call shape as `pwl`, for
    /// an arbitrary precomputed (e.g. numpy) array.
    #[staticmethod]
    fn sampled(t: Vec<f64>, v: Vec<f64>) -> Self {
        Self(Wave::sampled(t, v))
    }
    /// `Wave.from_fn(f)` — the one kind that costs a host call, once per
    /// evaluation (`ideal.md` §5's cost table).
    #[staticmethod]
    fn from_fn(f: Py<PyAny>) -> Self {
        let arc: std::sync::Arc<dyn Fn(f64) -> f64 + Send + Sync> = std::sync::Arc::new(move |t: f64| {
            Python::with_gil(|py| match f.bind(py).call1((t,)).and_then(|r| r.extract::<f64>()) {
                Ok(v) => v,
                Err(e) => std::panic::panic_any(crate::device::PySolveError { message: e.to_string(), time: t }),
            })
        });
        Self(Wave::from_fn(arc))
    }
    /// `Wave.sum(a, b)` — `a(t) + b(t)`.
    #[staticmethod]
    fn sum(a: &_Wave, b: &_Wave) -> Self {
        Self(Wave::sum(a.wave_clone(), b.wave_clone()))
    }
    /// `Wave.scaled(w, k)` — `k * w(t)`.
    #[staticmethod]
    fn scaled(w: &_Wave, k: f64) -> Self {
        Self(Wave::scaled(w.wave_clone(), k))
    }
    /// `Wave.shifted(w, dt)` — `w(t - dt)`.
    #[staticmethod]
    fn shifted(w: &_Wave, dt: f64) -> Self {
        Self(Wave::shifted(w.wave_clone(), dt))
    }

    /// The value at `t` — exposed so a Python device can read a `Wave` it
    /// holds without going through `b.wave(...)` (e.g. a device that wants
    /// the DC operating point of its own drive shape).
    fn value(&self, t: f64) -> f64 {
        self.0.value(t)
    }
}

// ── Behavior ─────────────────────────────────────────────────────────────

/// An inert placeholder kind (`value` and every Jacobian entry `0.0`) — what
/// a `_HostBehavior` is left holding after [`_HostBehavior::sum`]/`scaled`/
/// `composed` move its real kind out (see those methods' doc comments).
/// Using the Python-side object again after that point reads as "always
/// zero", never undefined behavior.
struct Moved;
impl BehaviorKind for Moved {
    fn value(&self, _inputs: &[f64]) -> f64 {
        0.0
    }
    fn jacobian(&self, _inputs: &[f64], out: &mut [f64]) {
        out.fill(0.0);
    }
}

/// `pip.Behavior` (DVT-09/10/11 Python half): the closed value-from-circuit-
/// state catalog. Every kind contributes a closed-form value **and**
/// Jacobian — zero host calls, once installed via `b.behavior(...)`.
#[pyclass(module = "piperine", name = "HostBehavior")]
pub struct _HostBehavior(pub Box<dyn BehaviorKind>);

impl _HostBehavior {
    /// Move this object's kind out, leaving [`Moved`] behind — `sum`/
    /// `scaled`/`composed`/`b.behavior(...)` all *consume* their operand
    /// kind (mirroring `piperine-signal`'s own `Wave::sum(a, b)`, which
    /// takes its operands by value); `Box<dyn BehaviorKind>` has no `Clone`
    /// bound to preserve (frozen crate, this batch), so ownership transfer
    /// via `mem::replace` is the sound alternative to borrowing into a
    /// temporary Python object that may not outlive the call.
    fn take(&mut self) -> Box<dyn BehaviorKind> {
        std::mem::replace(&mut self.0, Box::new(Moved))
    }
}

#[pymethods]
impl _HostBehavior {
    /// `Behavior.comparator(lo, hi, gain)` — a smooth (tanh-shaped) two-terminal comparator.
    #[staticmethod]
    fn comparator(lo: f64, hi: f64, gain: f64) -> Self {
        Self(Box::new(Comparator { lo, hi, gain }))
    }
    /// `Behavior.hysteresis(vth_hi, vth_lo)` — a Schmitt-trigger comparator.
    #[staticmethod]
    fn hysteresis(vth_hi: f64, vth_lo: f64) -> Self {
        Self(Box::new(Hysteresis::new(vth_hi, vth_lo)))
    }
    /// `Behavior.switch(ron, roff, vth)` — a two-state resistive switch.
    #[staticmethod]
    fn switch(ron: f64, roff: f64, vth: f64) -> Self {
        Self(Box::new(Switch { ron, roff, vth }))
    }
    /// `Behavior.limiter(lo, hi)` — a soft clamp between `lo` and `hi`.
    #[staticmethod]
    fn limiter(lo: f64, hi: f64) -> Self {
        Self(Box::new(BehaviorLimiter { lo, hi }))
    }
    /// `Behavior.vcvs(gain)` — a voltage-controlled voltage source.
    #[staticmethod]
    fn vcvs(gain: f64) -> Self {
        Self(Box::new(Vcvs { gain }))
    }
    /// `Behavior.vccs(gm)` — a voltage-controlled current source.
    #[staticmethod]
    fn vccs(gm: f64) -> Self {
        Self(Box::new(Vccs { gm }))
    }
    /// `Behavior.vco(f0, kvco)` — a voltage-controlled oscillator frequency.
    #[staticmethod]
    fn vco(f0: f64, kvco: f64) -> Self {
        Self(Box::new(Vco { f0, kvco }))
    }
    /// `Behavior.table(xs, ys)` — the pure-piecewise kind whose Jacobian is
    /// the segment slope (`ideal.md` §3.9 C): free, and cannot be wrong.
    #[staticmethod]
    fn table(xs: Vec<f64>, ys: Vec<f64>) -> PyResult<Self> {
        Table::new(xs, ys).map(|t| Self(Box::new(t))).map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }
    /// `Behavior.sum(a, b)` — consumes `a`/`b` (see [`_HostBehavior::take`]).
    #[staticmethod]
    fn sum(a: &mut _HostBehavior, b: &mut _HostBehavior) -> Self {
        Self(Box::new(BSum { a: a.take(), b: b.take() }))
    }
    /// `Behavior.scaled(a, k)` — consumes `a`.
    #[staticmethod]
    fn scaled(a: &mut _HostBehavior, k: f64) -> Self {
        Self(Box::new(BScaled { inner: a.take(), k }))
    }
    /// `Behavior.composed(f, g)` — `f(g(inputs))`; consumes both (the PWM
    /// shape, `ideal.md` §3.3: `g` a `Hysteresis`, `f` a `Switch`).
    #[staticmethod]
    fn composed(f: &mut _HostBehavior, g: &mut _HostBehavior) -> Self {
        Self(Box::new(piperine_signal::behavior::kinds::Composed { f: f.take(), g: g.take() }))
    }

    /// The closed-form value at `inputs` — for a Python device composing a
    /// `Behavior` inside its own level-0 `load_dc` (the deliberate opt-in
    /// case; `build(b)` + `b.behavior(...)` never calls this).
    fn value(&self, inputs: Vec<f64>) -> f64 {
        self.0.value(&inputs)
    }

    /// The closed-form Jacobian at `inputs` — see [`Self::value`]'s note.
    fn jacobian(&self, inputs: Vec<f64>) -> Vec<f64> {
        let mut out = vec![0.0; inputs.len()];
        self.0.jacobian(&inputs, &mut out);
        out
    }

    /// The catalog is **closed** (`ideal.md` §3.7): a Python author who
    /// needs a new kind implements Rust's `BehaviorKind` trait — this
    /// method exists solely to refuse, loudly and by name, an attempt to
    /// extend the catalog from Python.
    #[staticmethod]
    fn register_kind(_name: &str) -> PyResult<()> {
        Err(PyRuntimeError::new_err(
            "pip.Behavior's catalog is closed (ideal.md \u{a7}3.7): a new kind is added by \
             implementing `piperine_signal::behavior::BehaviorKind` in Rust, not by registering \
             a kind from Python — level 0 (a Python `load_dc`) is always available as the escape \
             hatch for a one-off case that does not warrant a new Rust kind",
        ))
    }
}

// ── Measure ──────────────────────────────────────────────────────────────

/// A [`MeasureKind`] behind one dynamic dispatch, so a single `_Measure`
/// pyclass can wrap any catalog kind. `MeasureKind` is object-safe (T28's
/// prerequisite for exposing it to Python without a generic per-kind type).
struct DynKind(Box<dyn MeasureKind + Send + Sync>);
impl MeasureKind for DynKind {
    fn update(&mut self, t: f64, v: f64, lte: f64) {
        self.0.update(t, v, lte)
    }
    fn value(&self) -> f64 {
        self.0.value()
    }
    fn error_bound(&self) -> f64 {
        self.0.error_bound()
    }
}

/// `pip.Measure` (DVT-13/15/16/18 Python half): the closed streaming-
/// reduction catalog. Every factory **arms immediately** (T13's "arm
/// creates a fresh accumulator" rule) — `Measure.peak()` is already a live,
/// independent handle, matching `Measure::arm`'s Rust contract.
#[pyclass(unsendable, module = "piperine", name = "Measure")]
pub struct _Measure(Measure<DynKind>);

#[pymethods]
impl _Measure {
    /// `Measure.peak()` — the running maximum.
    #[staticmethod]
    fn peak() -> Self {
        Self(Measure::arm(DynKind(Box::new(Peak::new()))))
    }
    /// `Measure.min()` — the running minimum.
    #[staticmethod]
    fn min() -> Self {
        Self(Measure::arm(DynKind(Box::new(Min::new()))))
    }
    /// `Measure.average()` — the running mean.
    #[staticmethod]
    fn average() -> Self {
        Self(Measure::arm(DynKind(Box::new(Average::new()))))
    }
    /// `Measure.rms()` — the running root-mean-square.
    #[staticmethod]
    fn rms() -> Self {
        Self(Measure::arm(DynKind(Box::new(Rms::new()))))
    }
    /// `Measure.ripple()` — the running peak-to-peak spread.
    #[staticmethod]
    fn ripple() -> Self {
        Self(Measure::arm(DynKind(Box::new(Ripple::new()))))
    }
    /// `Measure.integral()` — the running time integral.
    #[staticmethod]
    fn integral() -> Self {
        Self(Measure::arm(DynKind(Box::new(Integral::new()))))
    }
    /// `Measure.cross(level, dir)` — the first accepted-point crossing of `level`.
    #[staticmethod]
    fn cross(level: f64, dir: &str) -> PyResult<Self> {
        Ok(Self(Measure::arm(DynKind(Box::new(Cross::new(level, cross_dir(dir)?))))))
    }
    /// `Measure.settle(target, tol)` — the instant the signal settles within `tol` of `target`.
    #[staticmethod]
    fn settle(target: f64, tol: f64) -> Self {
        Self(Measure::arm(DynKind(Box::new(Settle::new(target, tol)))))
    }
    /// `Measure.count()` — the running number of accepted samples.
    #[staticmethod]
    fn count() -> Self {
        Self(Measure::arm(DynKind(Box::new(Count::new()))))
    }

    /// Fold in one accepted `(t, v, lte)` sample.
    fn feed(&mut self, t: f64, v: f64, lte: f64) {
        self.0.feed(t, v, lte);
    }

    /// The reduction's current value — loud (never `NaN`/`0.0`/nominal)
    /// before the first feed (T13).
    #[getter]
    fn value(&self) -> PyResult<f64> {
        self.0.value().map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// The reduction's error bound — same loudness rule as `value`.
    #[getter]
    fn error_bound(&self) -> PyResult<f64> {
        self.0.error_bound().map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Freeze the accumulator: `feed` becomes a no-op, `.value` stays
    /// readable.
    fn disarm(&mut self) {
        self.0.disarm();
    }

    /// See [`_HostBehavior::register_kind`]'s twin refusal.
    #[staticmethod]
    fn register_kind(_name: &str) -> PyResult<()> {
        Err(PyRuntimeError::new_err(
            "pip.Measure's catalog is closed (ideal.md \u{a7}3.7): a new reduction is added by \
             implementing `piperine_signal::measure::MeasureKind` in Rust, not by registering a \
             kind from Python — level 0 (a Python `load_dc`/`accept_timestep`) is always \
             available as the escape hatch for a one-off case that does not warrant a new Rust \
             kind",
        ))
    }
}

/// `Measure.delay` (DVT-13/15/16 Python half): two-channel — `Delay` is
/// deliberately **not** a `MeasureKind` upstream (its `feed(t, a, b)` takes
/// a second signal; `piperine-signal`'s own header on `Delay` explains why
/// widening `MeasureKind` for one kind is the worse trade). A separate
/// pyclass mirrors that shape exactly rather than forcing it through
/// `DynKind`.
#[pyclass(module = "piperine", name = "DelayMeasure")]
pub struct _DelayMeasure(Delay);

#[pymethods]
impl _DelayMeasure {
    /// `DelayMeasure.delay(level, dir)` — the time between two channels each crossing `level`.
    #[staticmethod]
    fn delay(level: f64, dir: &str) -> PyResult<Self> {
        Ok(Self(Delay::new(level, cross_dir(dir)?)))
    }

    /// Fold in one accepted `(t, a, b)` sample.
    fn feed(&mut self, t: f64, a: f64, b: f64) {
        self.0.feed(t, a, b);
    }

    /// `t_b_cross - t_a_cross`, once both channels have crossed —
    /// `f64::NAN` before that (the kind's own convention; `Delay` predates
    /// `MeasureKind`'s "loud, never NaN" rule precisely because it is not
    /// that trait).
    #[getter]
    fn value(&self) -> f64 {
        self.0.value()
    }

    #[getter]
    fn error_bound(&self) -> f64 {
        self.0.error_bound()
    }
}

fn cross_dir(s: &str) -> PyResult<piperine_signal::events::CrossDir> {
    match s {
        "rising" | "RISING" => Ok(piperine_signal::events::CrossDir::Rising),
        "falling" | "FALLING" => Ok(piperine_signal::events::CrossDir::Falling),
        "either" | "EITHER" => Ok(piperine_signal::events::CrossDir::Either),
        other => Err(PyRuntimeError::new_err(format!("unknown crossing direction {other:?} — use rising/falling/either"))),
    }
}

// ── Builder ──────────────────────────────────────────────────────────────

/// `b` in `def build(self, b): …` (T28): the once-at-construction install
/// site for zero-host-call level 1. Collected by
/// [`PyHostElement::new`](crate::device::PyHostElement::new) into the
/// device's own native stamp lists.
#[pyclass(unsendable, module = "piperine", name = "Builder")]
#[derive(Default)]
pub struct _Builder {
    pub waves: Vec<WaveStamp>,
    pub behaviors: Vec<BehaviorStamp>,
}

#[pymethods]
impl _Builder {
    /// Install a `Wave`-driven two-terminal force through a declared series
    /// impedance (`rout`; `0.0` is a legal, explicit ideal force — never
    /// defaulted, DVT-33).
    fn wave(&mut self, p: &_NetRef, n: &_NetRef, wave: &_Wave, rout: f64) {
        self.waves.push(WaveStamp::new(wave.wave_clone(), p.0.clone(), n.0.clone(), rout));
    }

    /// Install a `Behavior`-driven two-terminal force reacting to
    /// `controls`, in order. Consumes `behavior` (see [`_HostBehavior::take`]).
    fn behavior(
        &mut self,
        force_p: &_NetRef,
        force_n: &_NetRef,
        controls: Vec<PyRef<'_, _NetRef>>,
        behavior: &mut _HostBehavior,
    ) {
        let ctrl_refs: Vec<piperine_solver::abi::AnalogReference> = controls.iter().map(|c| c.0.clone()).collect();
        let kind = behavior.take();
        self.behaviors.push(BehaviorStamp::new(kind, ctrl_refs, force_p.0.clone(), force_n.0.clone()));
    }
}
