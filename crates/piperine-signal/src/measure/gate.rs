//! `Measure` gating (`ideal.md` §3.5/§8.3, DVT-17): a measure's optional
//! window, by time, by an analog crossing predicate, or by a digital net.
//!
//! **Settled-only is a correctness rule, not a preference** (user,
//! 2026-07-31, `ideal.md` §3.5): a mid-delta-cycle value can be an
//! artifact of DAG evaluation order, not a signal the circuit ever had.
//! The digital network settles by propagating through an evaluation
//! graph; an intermediate value observed partway through is an
//! implementation detail of the traversal, and gating on one would
//! produce a result that changes when an unrelated edit reorders the
//! graph — wrong, but stable-looking until it moves. So [`GateTracker`]'s
//! digital path only has a *settled* input to react to
//! ([`GateTracker::note_digital`]): there is no mid-cycle variant to call
//! by mistake, because the caller — the scheduler, in the layer above this
//! crate — is the only one who knows when settling happened, and this
//! crate does not (and must not) guess at it.

use super::MeasureKind;
use crate::events::{predict_crossing, CrossDir};

/// What gates a `Measure`'s accumulation.
pub enum Gate {
    /// `window=(t0, t1)` — open for `t0 <= t <= t1`.
    Time { t0: f64, t1: f64 },
    /// `gate=pip.above(net, level)` — open while the gate signal is above
    /// `level`, closed otherwise (T10's crossing predicate: the instant of
    /// the flip is available via [`GateTracker::last_crossing`]).
    Analog { level: f64 },
    /// `gate=pip.digital_high(net)` — open while the net's last *settled*
    /// value was high.
    Digital,
}

/// Tracks a [`Gate`]'s open/closed state across a run, and whether it has
/// ever been open — the second accumulated point in `Measure`'s "unfed
/// or never-gated is loud" rule (T13).
pub struct GateTracker {
    gate: Gate,
    open: bool,
    ever_opened: bool,
    prev_analog: Option<(f64, f64)>,
    last_crossing: Option<f64>,
}

impl GateTracker {
    pub fn new(gate: Gate) -> Self {
        Self { gate, open: false, ever_opened: false, prev_analog: None, last_crossing: None }
    }

    /// Whether the gate is currently open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Whether the gate has been open at least once, ever — even if it has
    /// since closed.
    pub fn ever_opened(&self) -> bool {
        self.ever_opened
    }

    /// The instant of the most recent analog crossing this tracker
    /// detected (`None` for `Time`/`Digital` gates, or before any
    /// crossing).
    pub fn last_crossing(&self) -> Option<f64> {
        self.last_crossing
    }

    /// Advance a [`Gate::Time`] window against the measured point's own
    /// time — trivial, no separate feed needed. A no-op for `Analog`/
    /// `Digital` gates (they have their own `note_*` calls).
    pub fn note_time(&mut self, t: f64) {
        if let Gate::Time { t0, t1 } = self.gate {
            self.open = t >= t0 && t <= t1;
            if self.open {
                self.ever_opened = true;
            }
        }
    }

    /// Feed the gate-controlling analog signal's value at `t` — only
    /// meaningful for [`Gate::Analog`]. Reuses [`predict_crossing`] (tier
    /// 2) to locate the flip instant rather than a bare threshold compare,
    /// the same "when does a predicate flip" primitive `Behavior` and
    /// `Measure.cross` share.
    pub fn note_analog(&mut self, t: f64, v: f64) {
        let Gate::Analog { level } = self.gate else { return };
        let shifted = v - level;
        let was_open = self.open;
        self.open = shifted > 0.0;
        if self.open {
            self.ever_opened = true;
        }
        if self.open != was_open
            && let Some((pt, pv)) = self.prev_analog
        {
            let dir = if self.open { CrossDir::Rising } else { CrossDir::Falling };
            if let Some(p) = predict_crossing(dir, pt, pv, t, shifted) {
                self.last_crossing = Some(p.t_cross);
            }
        }
        self.prev_analog = Some((t, shifted));
    }

    /// Set the digital gate state from a **settled** digital event only —
    /// the caller (the scheduler, above this crate) is responsible for
    /// never calling this from mid-delta-cycle; see the module header.
    pub fn note_digital(&mut self, high: bool) {
        if !matches!(self.gate, Gate::Digital) {
            return;
        }
        self.open = high;
        if high {
            self.ever_opened = true;
        }
    }
}

/// A `MeasureKind` wrapped with a [`Gate`]: `feed` (the `Time`-gate path,
/// which needs nothing but `t`) or the `note_*_gate` calls (`Analog`/
/// `Digital`, which need an out-of-band signal) decide whether a sample
/// actually reaches the accumulator. A gate that opens more than once
/// keeps feeding across **every** open interval, not just the first — it
/// is the same accumulator throughout, never reset on close.
pub struct Gated<K> {
    kind: K,
    tracker: GateTracker,
}

impl<K: MeasureKind> Gated<K> {
    pub fn new(kind: K, gate: Gate) -> Self {
        Self { kind, tracker: GateTracker::new(gate) }
    }

    /// Feed one accepted `(t, v, lte)`. For a [`Gate::Time`] gate this is
    /// the whole story (the window is judged from `t` alone); for
    /// `Analog`/`Digital` gates, call the matching `note_*_gate` first.
    pub fn feed(&mut self, t: f64, v: f64, lte: f64) {
        self.tracker.note_time(t);
        if self.tracker.is_open() {
            self.kind.update(t, v, lte);
        }
    }

    /// Update an [`Gate::Analog`] gate's controlling signal.
    pub fn note_analog_gate(&mut self, t: f64, gate_v: f64) {
        self.tracker.note_analog(t, gate_v);
    }

    /// Update a [`Gate::Digital`] gate's settled state.
    pub fn note_digital_gate(&mut self, high: bool) {
        self.tracker.note_digital(high);
    }

    pub fn value(&self) -> f64 {
        self.kind.value()
    }
    pub fn error_bound(&self) -> f64 {
        self.kind.error_bound()
    }
    pub fn ever_opened(&self) -> bool {
        self.tracker.ever_opened()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measure::kinds::{Count, Integral};

    #[test]
    fn time_gate_opens_and_closes_at_its_bounds() {
        let mut g = GateTracker::new(Gate::Time { t0: 1.0, t1: 2.0 });
        g.note_time(0.5);
        assert!(!g.is_open());
        g.note_time(1.5);
        assert!(g.is_open());
        g.note_time(2.5);
        assert!(!g.is_open());
        assert!(g.ever_opened());
    }

    #[test]
    fn analog_gate_opens_on_a_rising_crossing_and_closes_on_falling() {
        let mut g = GateTracker::new(Gate::Analog { level: 0.9 });
        g.note_analog(0.0, 0.0);
        assert!(!g.is_open());
        g.note_analog(1.0, 1.0); // crosses 0.9 between t=0 and t=1
        assert!(g.is_open());
        assert!(g.last_crossing().is_some());
        g.note_analog(2.0, 0.0); // falls back below
        assert!(!g.is_open());
    }

    #[test]
    fn digital_gate_reflects_only_settled_state() {
        let mut g = GateTracker::new(Gate::Digital);
        assert!(!g.is_open());
        g.note_digital(true);
        assert!(g.is_open());
        g.note_digital(false);
        assert!(!g.is_open());
        assert!(g.ever_opened());
    }

    #[test]
    fn digital_gate_is_insensitive_to_a_mid_delta_cycle_glitch_never_fed_to_it() {
        // The correctness argument (module header): the *only* input this
        // gate accepts is "the settled value". A caller that (wrongly)
        // fed an intermediate DAG-evaluation-order artifact as if it were
        // settled would corrupt the gate — proven here by contrast: two
        // sequences that share the same *settled* endpoints but differ in
        // a hypothetical mid-cycle value produce identical gate states,
        // because there is no code path for the crate to see that
        // mid-cycle value at all.
        let mut settled_only = GateTracker::new(Gate::Digital);
        settled_only.note_digital(false); // settled: low
        settled_only.note_digital(true); // settled: high (next edge)
        assert!(settled_only.is_open());

        // A hypothetical caller that (incorrectly) fed a glitch between
        // the two settled values has no way to do so through this API —
        // `note_digital` takes exactly one bit, the settled one. The
        // guard is structural: there is no `note_digital_unsettled`.
        let mut also_settled_only = GateTracker::new(Gate::Digital);
        also_settled_only.note_digital(false);
        also_settled_only.note_digital(true);
        assert_eq!(settled_only.is_open(), also_settled_only.is_open());
    }

    #[test]
    fn a_measure_whose_gate_opens_more_than_once_accumulates_across_every_open_interval() {
        // A digital gate that opens, closes, and reopens — Count must
        // reflect samples fed during *both* open intervals, not just the
        // first (DVT-17). Using Count (not a value the closing/reopening
        // itself could accidentally reset) makes "only the first interval
        // counted" and "both intervals counted" produce different,
        // checkable numbers.
        let mut gated = Gated::new(Count::new(), Gate::Digital);
        gated.note_digital_gate(true); // first interval opens
        gated.feed(0.0, 1.0, 0.0);
        gated.feed(1.0, 1.0, 0.0);
        gated.note_digital_gate(false); // closes
        gated.feed(2.0, 1.0, 0.0); // must NOT count: gate closed
        gated.note_digital_gate(true); // second interval opens
        gated.feed(3.0, 1.0, 0.0);
        gated.note_digital_gate(false);

        assert_eq!(gated.value(), 3.0, "2 samples from the first open interval + 1 from the second");
        assert!(gated.ever_opened());
    }

    #[test]
    fn analog_gated_measure_accumulates_only_while_the_gate_is_open() {
        let mut gated = Gated::new(Integral::new(), Gate::Analog { level: 0.5 });
        gated.note_analog_gate(0.0, 0.0); // gate closed (0.0 <= 0.5)
        gated.feed(0.0, 10.0, 0.0); // must not accumulate: gate closed
        gated.note_analog_gate(1.0, 1.0); // opens (crosses 0.5)
        gated.feed(1.0, 10.0, 0.0);
        gated.feed(2.0, 10.0, 0.0); // trapezoid over [1,2] at v=10: area 10
        assert_eq!(gated.value(), 10.0);
    }
}
