//! Event detectors: `cross`/`above`/`timer` — the tier-1 crossing detection
//! and timer bookkeeping that the runtime operators (`ops.rs`) sit beside.
//!
//! Extracted from `piperine-codegen`'s
//! `device/analog/events.rs::EventDetector`, semantics unchanged (the
//! `@initial` event-action dispatch stays in codegen — it is not a
//! detector). Codegen keeps its own copy until `codegen-simplification`
//! delegates to this one.
//!
//! **The seeding asymmetry is deliberate, not an inconsistency:** an
//! unseeded `cross` never fires (the first observation only arms it); an
//! unseeded `above` *does* fire on its first sample.
//!
//! **The tier-1/tier-2 boundary:** a `timer`'s instants are known a priori
//! (parameter-constant), so it declares static breakpoints
//! ([`EventDetector::next_breakpoints`]) that the integrator lands on
//! exactly. A crossing is solution-dependent, so it is detected reactively —
//! it contributes no static breakpoints here. Level 1's crossing tier 2
//! (`ideal.md` §3.4) is what moves crossings from the second class toward
//! the first.

/// Which sign changes a `cross` detector fires on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossDir {
    Rising,
    Falling,
    Either,
}

/// What an [`EventDetector`] watches.
#[derive(Debug, Clone, Copy)]
pub enum Trigger {
    /// `above(x)` — fires while rising past zero; fires on the first sample
    /// too (unseeded `rose` defaults to `true`).
    Above,
    /// `cross(x, dir)` — fires on a sign change between accepted samples,
    /// per direction. Never fires unseeded.
    Cross(CrossDir),
    /// `timer(period, phase)` — fires when `time` reaches `next_fire`,
    /// advancing `next_fire` by whole periods.
    Timer,
}

/// Per-event transition detector: remembers the previous accepted trigger
/// value (crossings) or the next fire time (timers).
pub struct EventDetector {
    /// A trigger value has been observed (crossing detection is armed).
    seeded: bool,
    prev: f64,
    next_fire: f64,
}

impl EventDetector {
    /// A fresh detector. `first_fire` is the timer's first `next_fire`
    /// instant (`phase` when `phase > 0`, else `period` — irrelevant for
    /// non-timer triggers).
    pub fn new(first_fire: f64) -> Self {
        Self { seeded: false, prev: 0.0, next_fire: first_fire }
    }

    /// Whether the event fires given the trigger value at the accepted
    /// solution, updating the detector state. `period` is the timer's
    /// period (ignored for non-timer triggers).
    pub fn fired(&mut self, trigger: Trigger, value: f64, time: f64, period: f64) -> bool {
        let fired = match trigger {
            Trigger::Above => {
                let rose = if self.seeded { self.prev <= 0.0 } else { true };
                rose && value > 0.0
            }
            Trigger::Cross(dir) => {
                let rising = self.seeded && self.prev <= 0.0 && value > 0.0;
                let falling = self.seeded && self.prev >= 0.0 && value < 0.0;
                match dir {
                    CrossDir::Rising => rising,
                    CrossDir::Falling => falling,
                    CrossDir::Either => rising || falling,
                }
            }
            Trigger::Timer => {
                let fires = period > 0.0 && time >= self.next_fire;
                if fires {
                    while self.next_fire <= time {
                        self.next_fire += period;
                    }
                }
                fires
            }
        };
        self.prev = value;
        self.seeded = true;
        fired
    }

    /// Absolute landing points this timer fires at within `(from, end]`.
    /// Only meaningful for a `Timer` detector with `period > 0`; a
    /// non-timer detector (or a non-finite/non-positive period) contributes
    /// nothing, matching the tier-1/tier-2 boundary above.
    pub fn next_breakpoints(&self, period: f64, from: f64, end: f64, out: &mut Vec<f64>) {
        if period <= 0.0 || !period.is_finite() {
            return;
        }
        // First fire strictly after `from` (next_fire may lag if the timer
        // hasn't been advanced past the current step yet).
        let mut t = self.next_fire;
        while t <= from {
            t += period;
        }
        while t <= end {
            out.push(t);
            t += period;
        }
    }
}

// ── Crossing tier 2: predicted + bounded (T10, DVT-10) ─────────────────────
//
// The detector above is **tier 1**: it fires reactively at the first
// accepted point after a sign change, up to one full timestep late, with
// no interpolation and unreported error — today's PHDL `@cross` (audited
// 2026-07-31, `ideal.md` §3.4). What follows is **tier 2**: extrapolate
// `t_cross` from the bracketing samples, publish an error bound, and (via
// [`refine_crossing`]) iterate the bracket narrower using the same
// `bound_step_hint`/`$bound_step` seam PHDL devices already have — no
// solver change, because both seams already exist end to end. This module
// needs no `piperine-solver` dependency: the crate never calls the solver,
// it only computes the instant and the bound a consumer (the host-device
// adapter, later) hands to the step-hint path.
//
// **Tier 3** (exact) is out of scope here: it needs `EventKind::Crossing`
// promoted from `Advisory` to `Exact`, plus reject-and-retry-at-`t*` — a
// real solver feature with its own spec. Tier 2 converges to the caller's
// step floor and reports that bound honestly; it never claims machine
// precision it did not pay for.

/// A predicted crossing instant with its error bound — `t_cross` is inside
/// `[t0, t1]`, the bracket that produced it, and `error_bound` is the width
/// of that bracket (never smaller than what the caller actually resolved).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Prediction {
    /// The interpolated (secant) crossing instant.
    pub t_cross: f64,
    /// How far `t_cross` could still be off — the bracket width. Shrinks
    /// as the bracket narrows; never claims better than it has.
    pub error_bound: f64,
}

impl Prediction {
    /// A next-step cap that walks the integrator toward `t_cross` — the
    /// value a consumer hands to `bound_step_hint`/`$bound_step` (`ideal.md`
    /// §3.9 F): the distance from `from` to the predicted instant, or the
    /// full step if the prediction is already behind `from`.
    pub fn next_step_bound(&self, from: f64) -> f64 {
        (self.t_cross - from).max(0.0)
    }
}

/// One secant prediction from a bracketing pair of accepted samples whose
/// sign change matches `dir` (`Rising`: `v0 <= 0` and `v1 > 0`; `Falling`:
/// `v0 >= 0` and `v1 < 0`; `Either`: any sign change). `None` when
/// `(t0, v0)`-`(t1, v1)` does not actually bracket a crossing in that
/// direction.
pub fn predict_crossing(dir: CrossDir, t0: f64, v0: f64, t1: f64, v1: f64) -> Option<Prediction> {
    let brackets = match dir {
        CrossDir::Rising => v0 <= 0.0 && v1 > 0.0,
        CrossDir::Falling => v0 >= 0.0 && v1 < 0.0,
        CrossDir::Either => (v0 <= 0.0 && v1 > 0.0) || (v0 >= 0.0 && v1 < 0.0),
    };
    if !brackets || v1 == v0 {
        return None;
    }
    let t_cross = t0 + (t1 - t0) * (-v0) / (v1 - v0);
    Some(Prediction { t_cross, error_bound: (t1 - t0).abs() })
}

/// Iteratively narrows a bracketing pair toward the crossing instant by
/// bisection, probing `sample` at each midpoint and re-bracketing — the
/// pure-math shape of what a real driver does by capping the next step via
/// `bound_step_hint` and re-detecting. Stops once the bracket width reaches
/// `step_floor` (or `max_iters` is spent) and **reports** rather than
/// stalling: the returned [`Prediction`]'s `error_bound` is the final
/// bracket width, floored at `step_floor` — it never claims a precision the
/// loop did not actually reach.
pub fn refine_crossing(
    mut t0: f64,
    mut v0: f64,
    mut t1: f64,
    mut v1: f64,
    step_floor: f64,
    max_iters: usize,
    mut sample: impl FnMut(f64) -> f64,
) -> Prediction {
    for _ in 0..max_iters {
        if (t1 - t0) <= step_floor {
            break;
        }
        let t_mid = 0.5 * (t0 + t1);
        let v_mid = sample(t_mid);
        // Re-bracket: keep whichever half still straddles zero (same
        // convention as CrossDir::Either — bisection needs only "opposite
        // signs", the caller already established which direction matters
        // by handing in a bracket of that direction to begin with).
        if (v0 <= 0.0) == (v_mid <= 0.0) {
            t0 = t_mid;
            v0 = v_mid;
        } else {
            t1 = t_mid;
            v1 = v_mid;
        }
    }
    let width = (t1 - t0).max(step_floor);
    let t_cross = if v1 != v0 { t0 + (t1 - t0) * (-v0) / (v1 - v0) } else { 0.5 * (t0 + t1) };
    Prediction { t_cross, error_bound: width }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cross_fires_on_rising_sign_change_between_accepted_samples() {
        let mut d = EventDetector::new(0.0);
        d.fired(Trigger::Cross(CrossDir::Rising), -1.0, 0.0, 0.0); // seed
        assert!(d.fired(Trigger::Cross(CrossDir::Rising), 1.0, 1.0, 0.0));
    }

    #[test]
    fn cross_fires_on_falling_sign_change_between_accepted_samples() {
        let mut d = EventDetector::new(0.0);
        d.fired(Trigger::Cross(CrossDir::Falling), 1.0, 0.0, 0.0); // seed
        assert!(d.fired(Trigger::Cross(CrossDir::Falling), -1.0, 1.0, 0.0));
    }

    #[test]
    fn cross_either_fires_on_both_directions() {
        let mut d = EventDetector::new(0.0);
        d.fired(Trigger::Cross(CrossDir::Either), -1.0, 0.0, 0.0);
        assert!(d.fired(Trigger::Cross(CrossDir::Either), 1.0, 1.0, 0.0), "rising");
        assert!(d.fired(Trigger::Cross(CrossDir::Either), -1.0, 2.0, 0.0), "falling");
    }

    #[test]
    fn unseeded_cross_never_fires() {
        let mut d = EventDetector::new(0.0);
        // First observation only arms the detector — must not fire even
        // though the raw value is positive (there is no "previous" to
        // compare against).
        assert!(!d.fired(Trigger::Cross(CrossDir::Rising), 1.0, 0.0, 0.0));
        assert!(!d.fired(Trigger::Cross(CrossDir::Either), 1.0, 0.0, 0.0));
    }

    #[test]
    fn unseeded_above_does_fire_on_first_sample() {
        // Deliberately asymmetric to `cross` (ideal.md §3.9 B): `above`
        // fires on its very first sample because unseeded `rose` defaults
        // to `true`.
        let mut d = EventDetector::new(0.0);
        assert!(d.fired(Trigger::Above, 1.0, 0.0, 0.0));
    }

    #[test]
    fn above_does_not_fire_on_first_sample_if_not_positive() {
        let mut d = EventDetector::new(0.0);
        assert!(!d.fired(Trigger::Above, -1.0, 0.0, 0.0));
    }

    #[test]
    fn timer_first_fire_at_phase_when_positive() {
        let mut d = EventDetector::new(0.5); // phase = 0.5 (> 0)
        assert!(!d.fired(Trigger::Timer, 0.4, 0.4, 1.0));
        assert!(d.fired(Trigger::Timer, 0.5, 0.5, 1.0));
    }

    #[test]
    fn timer_first_fire_at_period_when_phase_not_positive() {
        // Caller passes `first_fire = period` when `phase <= 0` (the
        // resolution rule lives in the consumer, per the source).
        let mut d = EventDetector::new(1.0); // first_fire = period = 1.0
        assert!(!d.fired(Trigger::Timer, 0.5, 0.5, 1.0));
        assert!(d.fired(Trigger::Timer, 1.0, 1.0, 1.0));
    }

    #[test]
    fn timer_advances_next_fire_by_whole_periods() {
        let mut d = EventDetector::new(1.0);
        assert!(d.fired(Trigger::Timer, 1.0, 1.0, 1.0));
        // A big jump past several periods advances next_fire to the first
        // period boundary at or after `time` (2, 3, 4, 5, 6 → lands on 6.0).
        assert!(d.fired(Trigger::Timer, 5.5, 5.5, 1.0));
        assert!(!d.fired(Trigger::Timer, 5.9, 5.9, 1.0), "next_fire is 6.0, not yet reached");
        assert!(d.fired(Trigger::Timer, 6.0, 6.0, 1.0), "next_fire reached exactly");
    }

    #[test]
    fn timer_declares_static_breakpoints() {
        let d = EventDetector::new(1.0);
        let mut out = Vec::new();
        d.next_breakpoints(1.0, 0.0, 3.5, &mut out);
        assert_eq!(out, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn crossings_declare_no_static_breakpoints() {
        // The tier-1/tier-2 boundary: only timers (known a priori) get
        // static breakpoints. `next_breakpoints` on a non-timer detector
        // must contribute nothing — asserted by passing period=0 (a
        // crossing detector has no period).
        let d = EventDetector::new(0.0);
        let mut out = Vec::new();
        d.next_breakpoints(0.0, 0.0, 10.0, &mut out);
        assert!(out.is_empty());
    }

    // ---- tier 2: predicted + bounded ----

    #[test]
    fn predict_crossing_interpolates_a_linear_bracket_exactly() {
        // v(t) = t - 0.5, bracketed by [0, 1]: the secant is exact for a
        // linear function.
        let p = predict_crossing(CrossDir::Rising, 0.0, -0.5, 1.0, 0.5).unwrap();
        assert!((p.t_cross - 0.5).abs() < 1e-15);
        assert_eq!(p.error_bound, 1.0, "bracket width, unrefined");
    }

    #[test]
    fn predict_crossing_rejects_a_non_bracketing_pair() {
        assert!(predict_crossing(CrossDir::Rising, 0.0, 0.5, 1.0, 1.0).is_none(), "no sign change");
        assert!(predict_crossing(CrossDir::Falling, 0.0, -0.5, 1.0, 0.5).is_none(), "wrong direction");
    }

    #[test]
    fn next_step_bound_walks_toward_the_prediction() {
        let p = predict_crossing(CrossDir::Rising, 0.0, -1.0, 1.0, 1.0).unwrap();
        assert_eq!(p.t_cross, 0.5);
        assert_eq!(p.next_step_bound(0.2), 0.3, "distance from `from` to t_cross");
        assert_eq!(p.next_step_bound(0.9), 0.0, "already past the prediction: no further cap");
    }

    #[test]
    fn refine_crossing_converges_on_an_analytically_known_instant_and_the_error_shrinks_toward_the_floor() {
        // v(t) = t - 0.37, crossing exactly at t = 0.37 (analytically
        // known). Coarser floor first, then finer — the published error
        // must shrink as the floor drops, per DVT-10.
        let sample = |t: f64| t - 0.37;
        let coarse = refine_crossing(0.0, sample(0.0), 1.0, sample(1.0), 1e-2, 100, sample);
        let fine = refine_crossing(0.0, sample(0.0), 1.0, sample(1.0), 1e-8, 100, sample);

        assert!((coarse.t_cross - 0.37).abs() <= coarse.error_bound + 1e-12);
        assert!((fine.t_cross - 0.37).abs() <= fine.error_bound + 1e-12);
        assert!(fine.error_bound < coarse.error_bound, "a smaller step floor yields a tighter published bound");
        assert!((fine.t_cross - 0.37).abs() < 1e-6, "converges close to the true instant");
    }

    #[test]
    fn refine_crossing_handles_a_nonlinear_analytically_known_crossing() {
        // v(t) = sin(t - 0.5), crossing exactly at t = 0.5. Bisection is
        // curvature-agnostic, unlike a single secant guess.
        let sample = |t: f64| (t - 0.5).sin();
        let p = refine_crossing(0.0, sample(0.0), 1.0, sample(1.0), 1e-9, 200, sample);
        assert!((p.t_cross - 0.5).abs() < 1e-6);
        assert!((p.t_cross - 0.5).abs() <= p.error_bound + 1e-6);
    }

    #[test]
    fn refine_crossing_terminates_at_the_step_floor_and_reports_rather_than_stalling() {
        let sample = |t: f64| t - 0.37;
        let step_floor = 1e-3;
        let p = refine_crossing(0.0, sample(0.0), 1.0, sample(1.0), step_floor, 1000, sample);
        // Must stop (not loop until max_iters chews arbitrary precision)
        // once the bracket reaches the floor, and the reported bound
        // reflects exactly that — never smaller than what was actually
        // resolved.
        assert!(p.error_bound >= step_floor - 1e-15);
        assert!(p.error_bound < 1.0, "actually narrowed from the initial bracket");
    }
}
