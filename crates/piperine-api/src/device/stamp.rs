//! `StampSink` / `Ctx` / `StateView` (T14 groundwork, hardened in T15,
//! DVT-19): the wrappers a [`HostElement`](super::HostElement) reads/writes
//! the analog solve through, so the blanket adapter never hands device code
//! a raw solver type. This file lands the shape T14 needs to prove a host
//! device stamps into a real circuit; T15 adds the index-validation
//! (`StampError`, `try_g`/`try_i`) that makes it the safe boundary design.md
//! §C2 describes.

use piperine_solver::abi::{AnalogReference, CircularArrayBuffer2, Stamp};

/// The stamp-writing half of the wrapper: `g`/`i` push [`Stamp`] entries
/// into a `Vec` the blanket adapter owns and drains after one
/// `load_dc`/`load_transient` call. A branch row is just another
/// `AnalogReference` (the type already unifies node and branch unknowns),
/// so `g`/`i` cover the `branch(...)` case too — no third method needed.
pub struct StampSink {
    stamps: Vec<Stamp<AnalogReference, f64>>,
}

impl StampSink {
    pub(crate) fn new() -> Self {
        Self { stamps: Vec::new() }
    }

    /// Matrix (conductance/transconductance) entry at `(row, col)`.
    pub fn g(&mut self, row: &AnalogReference, col: &AnalogReference, value: f64) {
        self.stamps.push(Stamp::Matrix(row.clone(), col.clone(), value));
    }

    /// RHS (current) entry at `row`.
    pub fn i(&mut self, row: &AnalogReference, value: f64) {
        self.stamps.push(Stamp::Rhs(row.clone(), value));
    }

    pub(crate) fn into_stamps(self) -> Vec<Stamp<AnalogReference, f64>> {
        self.stamps
    }
}

/// Read-only solution + timing context handed to `load_dc`/`load_transient`.
/// Borrow-scoped to the one call — a `HostElement` cannot store a `Ctx`
/// past the method it was given to.
pub struct Ctx<'a> {
    history: &'a CircularArrayBuffer2<f64>,
    t: f64,
    dt: f64,
}

impl<'a> Ctx<'a> {
    pub(crate) fn new(history: &'a CircularArrayBuffer2<f64>, t: f64, dt: f64) -> Self {
        Self { history, t, dt }
    }

    /// The latest accepted/candidate voltage (or branch current) at `r`.
    /// Ground (`idx() == None`) reads as `0.0`.
    pub fn v(&self, r: &AnalogReference) -> f64 {
        match r.idx() {
            None => 0.0,
            Some(i) => self.history.latest().and_then(|row| row.get(i).copied()).unwrap_or(0.0),
        }
    }

    /// The current analysis time (`0.0` at a DC operating point).
    pub fn t(&self) -> f64 {
        self.t
    }

    /// The current step size (`0.0` at DC, where there is none).
    pub fn dt(&self) -> f64 {
        self.dt
    }
}

/// Read-only accepted-solution view handed to
/// [`HostElement::update`](super::HostElement::update) and
/// [`HostElement::accept_timestep`](super::HostElement::accept_timestep) —
/// the same borrow-scoping rule as [`Ctx`], without the timing fields a
/// stamping call needs but an accept hook does not.
pub struct StateView<'a> {
    history: &'a CircularArrayBuffer2<f64>,
}

impl<'a> StateView<'a> {
    pub(crate) fn new(history: &'a CircularArrayBuffer2<f64>) -> Self {
        Self { history }
    }

    /// The latest accepted value at `r`. Ground reads as `0.0`.
    pub fn v(&self, r: &AnalogReference) -> f64 {
        match r.idx() {
            None => 0.0,
            Some(i) => self.history.latest().and_then(|row| row.get(i).copied()).unwrap_or(0.0),
        }
    }
}
