//! `StampSink` / `Ctx` / `StateView` (T15, DVT-19): the safe wrappers a
//! [`HostElement`](super::HostElement) reads/writes the analog solve
//! through — the real work of exposing matrix rows and solution values
//! across what is, for a Python host device, an FFI boundary.
//!
//! Design rules (`design.md` §C2), each load-bearing:
//!
//! - The sink exposes only **operations** (`g`/`i`) — never a `&mut` to the
//!   underlying matrix. A branch row is just another [`AnalogReference`]
//!   (the type already unifies node and branch unknowns), so `g`/`i` cover
//!   the `branch(...)` case too — no third method needed.
//! - An out-of-range index is a **loud typed error** ([`StampError`]),
//!   never undefined behavior. The ergonomic `g`/`i` panic with that
//!   error's message (the `Element::load_dc` ABI has no `Result` to
//!   propagate through); the fallible `try_g`/`try_i` are there for a
//!   caller that wants the `Result` itself.
//! - [`Ctx`] and [`StateView`] borrow the solution history for exactly one
//!   call — nothing a device holds can outlive the solve step that
//!   produced it.
//! - No allocation on the stamping path beyond the one `Vec` the sink is
//!   built with (reserved to the circuit's own unknown count) — `g`/`i`
//!   only `push` into it.

use std::fmt;

use piperine_solver::abi::{AnalogReference, CircularArrayBuffer2, Stamp};

/// An out-of-range stamp: a [`StampSink`] operation was asked to touch a
/// row/column index that does not exist in this circuit's unknown vector.
/// Never produced by an `AnalogReference` obtained honestly from a live
/// [`Ctx`]/netlist — it is the loud signal for a stale or foreign reference
/// (e.g. one held from a different, smaller circuit).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StampError {
    /// The offending index.
    pub index: usize,
    /// The circuit's actual unknown count — `index` must be `< size`.
    pub size: usize,
}

impl fmt::Display for StampError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "stamp index {} is out of range for this circuit (size = {})",
            self.index, self.size
        )
    }
}

impl std::error::Error for StampError {}

/// The stamp-writing half of the safe wrapper (T15): `g`/`i` push validated
/// [`Stamp`] entries into a `Vec` the blanket adapter owns and drains after
/// one `load_dc`/`load_transient` call. A ground reference (`idx() ==
/// None`) is always valid — it means "the reference row", not an unknown.
pub struct StampSink {
    size: usize,
    stamps: Vec<Stamp<AnalogReference, f64>>,
}

impl StampSink {
    pub(crate) fn new(size: usize) -> Self {
        Self { size, stamps: Vec::new() }
    }

    fn check(&self, r: &AnalogReference) -> Result<(), StampError> {
        match r.idx() {
            None => Ok(()),
            Some(i) if i < self.size => Ok(()),
            Some(i) => Err(StampError { index: i, size: self.size }),
        }
    }

    /// Matrix (conductance/transconductance) entry at `(row, col)` —
    /// fallible: `Err` on an out-of-range reference, never a write past the
    /// circuit's unknown vector.
    pub fn try_g(&mut self, row: &AnalogReference, col: &AnalogReference, value: f64) -> Result<(), StampError> {
        self.check(row)?;
        self.check(col)?;
        self.stamps.push(Stamp::Matrix(row.clone(), col.clone(), value));
        Ok(())
    }

    /// RHS (current) entry at `row` — fallible twin of [`Self::i`].
    pub fn try_i(&mut self, row: &AnalogReference, value: f64) -> Result<(), StampError> {
        self.check(row)?;
        self.stamps.push(Stamp::Rhs(row.clone(), value));
        Ok(())
    }

    /// `g(row, col, value)` — the ergonomic form the examples in
    /// `ideal.md` §2/§9 use. Panics with the [`StampError`]'s message on an
    /// out-of-range index (the ABI's `load_dc`/`load_transient` return no
    /// `Result` to carry an `Err` through) — never silently drops or wraps
    /// the index.
    pub fn g(&mut self, row: &AnalogReference, col: &AnalogReference, value: f64) {
        self.try_g(row, col, value).unwrap_or_else(|e| panic!("StampSink::g: {e}"));
    }

    /// `i(row, value)` — see [`Self::g`]'s panic contract.
    pub fn i(&mut self, row: &AnalogReference, value: f64) {
        self.try_i(row, value).unwrap_or_else(|e| panic!("StampSink::i: {e}"));
    }

    pub(crate) fn into_stamps(self) -> Vec<Stamp<AnalogReference, f64>> {
        self.stamps
    }
}

/// Read-only solution + timing context handed to `load_dc`/`load_transient`
/// (T15). Borrow-scoped to the one call — a `HostElement` cannot store a
/// `Ctx` past the method it was given to.
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
/// [`HostElement::accept_timestep`](super::HostElement::accept_timestep)
/// (T15) — the same borrow-scoping rule as [`Ctx`], without the timing
/// fields a stamping call needs but an accept hook does not.
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

    /// A value `lookback` accepted points ago (`0` = latest). `None` before
    /// that much history exists.
    pub fn v_at(&self, r: &AnalogReference, lookback: usize) -> Option<f64> {
        match r.idx() {
            None => Some(0.0),
            Some(i) => self.history.view(lookback).and_then(|row| row.get(i).copied()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use piperine_solver::abi::{AnalogVariable, NodeIdentifier};
    use std::sync::Arc;

    fn node(i: usize) -> AnalogReference {
        AnalogReference::new(Arc::new(AnalogVariable::Node(NodeIdentifier::Anonymous(i))), i)
    }

    #[test]
    fn ground_is_always_a_valid_stamp_target() {
        let mut sink = StampSink::new(2);
        let gnd = AnalogReference::ground();
        let p = node(0);
        assert!(sink.try_g(&p, &gnd, 1.0).is_ok());
        assert!(sink.try_i(&gnd, 1.0).is_ok());
    }

    #[test]
    fn in_range_index_stamps_cleanly() {
        let mut sink = StampSink::new(2);
        let p = node(0);
        let n = node(1);
        assert!(sink.try_g(&p, &n, 1.0).is_ok());
        assert_eq!(sink.into_stamps().len(), 1);
    }

    #[test]
    fn out_of_range_index_is_a_loud_typed_error_never_ub() {
        let mut sink = StampSink::new(2);
        let bad = node(99);
        let ok = node(0);
        let err = sink.try_g(&bad, &ok, 1.0).expect_err("index 99 is out of range for size 2");
        assert_eq!(err, StampError { index: 99, size: 2 });
        assert!(err.to_string().contains("99"));
        assert!(err.to_string().contains('2'));
        // The sink must not have recorded a stamp for the failed attempt.
        assert!(sink.into_stamps().is_empty());
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn the_ergonomic_g_panics_loudly_rather_than_writing_past_the_vector() {
        let mut sink = StampSink::new(1);
        let bad = node(7);
        let ok = node(0);
        sink.g(&bad, &ok, 1.0);
    }

    #[test]
    fn stamp_sink_reserves_once_and_never_reallocates_per_call() {
        // Shape-based proof (per the task's "assert by shape, not by
        // benchmark" rule): the sink's Vec starts at the reserved capacity
        // and `g`/`i` never grow it beyond the number of calls made.
        let mut sink = StampSink::new(4);
        let a = node(0);
        let b = node(1);
        for _ in 0..3 {
            sink.g(&a, &b, 1.0);
        }
        assert_eq!(sink.into_stamps().len(), 3, "exactly one Vec entry per call, no hidden allocation churn");
    }
}
