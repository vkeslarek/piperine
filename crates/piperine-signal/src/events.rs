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
}
