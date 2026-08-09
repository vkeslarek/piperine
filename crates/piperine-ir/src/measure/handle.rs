//! `Measure` lifecycle: arm, read, disarm (`ideal.md` §4.3, DVT-14/18).
//!
//! "Arm and read" leaves four real questions open, each answered here:
//! - **`arm` creates a fresh accumulator**, starting at the arm instant.
//!   Arming the same measure twice yields two independent handles — a
//!   measure is a handle, not a name in a global table (there is no
//!   global table here at all).
//! - **It accumulates on accepted points only.** This crate never sees a
//!   rejected step — that filtering is the caller's job (`ideal.md` §7);
//!   what this type guarantees is that nothing it was never `feed`-ed
//!   contributes.
//! - **Reading before any accepted point has fed it is loud** — not
//!   `NaN`, not zero, not the nominal. The same applies to a gated measure
//!   whose window never opened: "never sampled", named, not `0.0`.
//! - **`disarm()` freezes it.** After disarming, `feed` becomes a no-op,
//!   so the underlying accumulator's state — and therefore `.value()` —
//!   stops moving but stays readable.

use super::gate::{Gate, GateTracker};
use super::MeasureKind;

/// Why `.value()`/`.error_bound()` refused to answer. `ideal.md` §4.3
/// names both causes identically — "never sampled" — because a gated
/// measure whose window never opened has, from the accumulator's point of
/// view, the same fact as a plain measure that was never fed: zero
/// accepted points reached it. A gate that opened but then closed still
/// left `fed > 0` while it was open, so that case never reaches here — see
/// [`Measure::feed`]: a gate only lets a sample through while open, so
/// `fed == 0` already *is* "never sampled, for whatever reason."
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasureError {
    /// No accepted point ever reached the accumulator — either nothing
    /// was fed at all, or every feed arrived while a gate was closed.
    NeverSampled,
}

impl std::fmt::Display for MeasureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MeasureError::NeverSampled => write!(f, "never sampled: no accepted point has fed this measure yet"),
        }
    }
}

impl std::error::Error for MeasureError {}

/// A `MeasureKind` with its lifecycle: armed fresh, fed on accepted
/// points, optionally gated, and disarmable.
pub struct Measure<K> {
    kind: K,
    gate: Option<GateTracker>,
    fed: u64,
    disarmed: bool,
}

impl<K: MeasureKind> Measure<K> {
    /// A fresh, ungated accumulator.
    pub fn arm(kind: K) -> Self {
        Self { kind, gate: None, fed: 0, disarmed: false }
    }

    /// A fresh accumulator gated by `gate` (T12).
    pub fn arm_gated(kind: K, gate: Gate) -> Self {
        Self { kind, gate: Some(GateTracker::new(gate)), fed: 0, disarmed: false }
    }

    /// Feed one accepted `(t, v, lte)`. A no-op once [`disarm`](Self::disarm)
    /// has been called, or while a gate is present and closed.
    pub fn feed(&mut self, t: f64, v: f64, lte: f64) {
        if self.disarmed {
            return;
        }
        if let Some(g) = &mut self.gate {
            g.note_time(t);
            if !g.is_open() {
                return;
            }
        }
        self.kind.update(t, v, lte);
        self.fed += 1;
    }

    /// Update an analog gate's controlling signal (no-op unless armed with
    /// [`Gate::Analog`]).
    pub fn note_analog_gate(&mut self, t: f64, gate_v: f64) {
        if let Some(g) = &mut self.gate {
            g.note_analog(t, gate_v);
        }
    }

    /// Set a digital gate's **settled** state (no-op unless armed with
    /// [`Gate::Digital`]) — see `gate.rs`'s module header for why this
    /// takes only a settled bit.
    pub fn note_digital_gate(&mut self, high: bool) {
        if let Some(g) = &mut self.gate {
            g.note_digital(high);
        }
    }

    /// Freeze this measure: `feed` becomes a no-op from here on, so the
    /// accumulator's (and therefore `.value()`'s) state stops moving —
    /// but stays readable.
    pub fn disarm(&mut self) {
        self.disarmed = true;
    }

    fn check_sampled(&self) -> Result<(), MeasureError> {
        if self.fed == 0 {
            return Err(MeasureError::NeverSampled);
        }
        Ok(())
    }

    /// The reduction's current value. Loud (never `NaN`/`0.0`/nominal)
    /// before the first feed, or if a gate never opened.
    pub fn value(&self) -> Result<f64, MeasureError> {
        self.check_sampled()?;
        Ok(self.kind.value())
    }

    /// The reduction's error bound, same loudness rule as `value()`.
    pub fn error_bound(&self) -> Result<f64, MeasureError> {
        self.check_sampled()?;
        Ok(self.kind.error_bound())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measure::kinds::{Count, Peak};

    #[test]
    fn arm_starts_fresh_and_two_arms_are_independent() {
        let mut a = Measure::arm(Count::new());
        let b = Measure::arm(Count::new());
        a.feed(0.0, 1.0, 0.0);
        a.feed(1.0, 1.0, 0.0);
        assert_eq!(a.value(), Ok(2.0));
        assert_eq!(b.value(), Err(MeasureError::NeverSampled), "b was never fed, independently of a");
    }

    #[test]
    fn reading_before_any_feed_is_loud_not_nan_zero_or_nominal() {
        let m = Measure::arm(Peak::new());
        let err = m.value().unwrap_err();
        assert_eq!(err, MeasureError::NeverSampled);
        // Never a plausible wrong number: Result::Err, not Ok(0.0)/Ok(NaN).
        assert!(m.value().is_err());
    }

    #[test]
    fn a_gated_measure_whose_window_never_opened_fails_loud_as_never_sampled() {
        let mut m = Measure::arm_gated(Count::new(), Gate::Time { t0: 10.0, t1: 20.0 });
        m.feed(0.0, 1.0, 0.0); // outside the window: gate never opens
        m.feed(1.0, 1.0, 0.0);
        let err = m.value().unwrap_err();
        assert_eq!(err, MeasureError::NeverSampled);
        assert!(err.to_string().contains("never sampled"), "named 'never sampled', per ideal.md §4.3");
    }

    #[test]
    fn disarm_freezes_the_value_which_stays_readable_afterwards() {
        let mut m = Measure::arm(Peak::new());
        m.feed(0.0, 1.0, 0.0);
        m.feed(1.0, 5.0, 0.0);
        m.disarm();
        m.feed(2.0, 100.0, 0.0); // must be ignored: disarmed
        assert_eq!(m.value(), Ok(5.0), "frozen at the value before disarm, still readable");
        assert_eq!(m.value(), Ok(5.0), "reading again doesn't change it");
    }

    #[test]
    fn gated_feed_only_reaches_the_accumulator_while_the_window_is_open() {
        let mut m = Measure::arm_gated(Count::new(), Gate::Time { t0: 1.0, t1: 2.0 });
        m.feed(0.5, 1.0, 0.0); // before the window
        m.feed(1.5, 1.0, 0.0); // inside
        m.feed(2.5, 1.0, 0.0); // after
        assert_eq!(m.value(), Ok(1.0));
    }
}
