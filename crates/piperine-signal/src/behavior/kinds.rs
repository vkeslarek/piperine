//! The shipped `BehaviorKind` catalog (`ideal.md` §3.3):
//! `comparator`/`hysteresis`/`switch`/`limiter`/`vcvs`/`vccs`/`vco` plus
//! `sum`/`scaled`/`composed`. `table` lives in `table.rs` (extracted first,
//! T4) with its own `BehaviorKind` impl appended there.
//!
//! **Smooth is the default, hard edges are opt-in** (§3.3): [`Comparator`]
//! is `tanh`-shaped and differentiable everywhere. [`Hysteresis`] and
//! [`Switch`] are the genuinely discontinuous kinds — flat between their
//! own transition (a stepped output has zero slope away from the edge, and
//! the edge itself is not differentiated: it is reported through
//! [`BehaviorKind::crossings`] so the integrator lands on it instead).
//! [`Hysteresis`] needs one bit of memory (which side of its band it is on)
//! — held in an [`std::sync::atomic::AtomicBool`] so `value`/`jacobian` can
//! stay `&self` (matching the trait) while still satisfying `Send + Sync`
//! (dv-tester T16/T18 SPEC_DEVIATION: a `HostElement` embedding a `Behavior`
//! reaches the solver as `Box<dyn Element>`, which requires it unconditionally
//! — `Cell` is `!Sync`, `AtomicBool` is the same interior-mutability shape
//! with no logic change); that memory is exactly the "value from circuit
//! state" a comparator-with-memory is.

use std::sync::atomic::{AtomicBool, Ordering};

use super::{BehaviorKind, Crossing};
use crate::events::CrossDir;

/// `Behavior.comparator(a, b, lo, hi, gain)` — a `tanh`-shaped smooth
/// comparator: `lo + (hi-lo)/2 * (1 + tanh(gain*(a-b)))`. Inputs `[a, b]`.
pub struct Comparator {
    pub lo: f64,
    pub hi: f64,
    pub gain: f64,
}

impl BehaviorKind for Comparator {
    fn value(&self, inputs: &[f64]) -> f64 {
        let (a, b) = (inputs[0], inputs[1]);
        self.lo + (self.hi - self.lo) * 0.5 * (1.0 + (self.gain * (a - b)).tanh())
    }
    fn jacobian(&self, inputs: &[f64], out: &mut [f64]) {
        let (a, b) = (inputs[0], inputs[1]);
        let t = (self.gain * (a - b)).tanh();
        let sech2 = 1.0 - t * t;
        let d = (self.hi - self.lo) * 0.5 * self.gain * sech2;
        out[0] = d;
        out[1] = -d;
    }
}

/// `Behavior.hysteresis(a, b, vth_hi, vth_lo)` — a two-terminal Schmitt
/// trigger: output is `1.0` once `a - b` rises past `vth_hi`, `0.0` once it
/// falls past `vth_lo`, and holds between the two thresholds. Inputs
/// `[a, b]`. Genuinely discontinuous: `jacobian` is `0` everywhere (a
/// stepped output has no local slope away from its own edge); the edge is
/// reported through [`BehaviorKind::crossings`], keyed to whichever
/// threshold the *current* state is watching for next.
pub struct Hysteresis {
    pub vth_hi: f64,
    pub vth_lo: f64,
    high: AtomicBool,
}

impl Hysteresis {
    pub fn new(vth_hi: f64, vth_lo: f64) -> Self {
        Self { vth_hi, vth_lo, high: AtomicBool::new(false) }
    }
}

impl BehaviorKind for Hysteresis {
    fn value(&self, inputs: &[f64]) -> f64 {
        let diff = inputs[0] - inputs[1];
        if self.high.load(Ordering::Relaxed) {
            if diff < self.vth_lo {
                self.high.store(false, Ordering::Relaxed);
            }
        } else if diff > self.vth_hi {
            self.high.store(true, Ordering::Relaxed);
        }
        if self.high.load(Ordering::Relaxed) { 1.0 } else { 0.0 }
    }
    fn jacobian(&self, inputs: &[f64], out: &mut [f64]) {
        let _ = inputs;
        out[0] = 0.0;
        out[1] = 0.0;
    }
    fn crossings(&self, inputs: &[f64]) -> Option<Crossing> {
        let diff = inputs[0] - inputs[1];
        let threshold = if self.high.load(Ordering::Relaxed) { self.vth_lo } else { self.vth_hi };
        let dir = if self.high.load(Ordering::Relaxed) { CrossDir::Falling } else { CrossDir::Rising };
        Some(Crossing { value: diff, threshold, dir })
    }
}

/// `Behavior.switch(ctrl, ron, roff, vth)` — a controlled conductance:
/// `1/ron` while `ctrl >= vth`, `1/roff` otherwise. Input `[ctrl]`.
/// Genuinely discontinuous, same treatment as [`Hysteresis`].
pub struct Switch {
    pub ron: f64,
    pub roff: f64,
    pub vth: f64,
}

impl BehaviorKind for Switch {
    fn value(&self, inputs: &[f64]) -> f64 {
        if inputs[0] >= self.vth { 1.0 / self.ron } else { 1.0 / self.roff }
    }
    fn jacobian(&self, inputs: &[f64], out: &mut [f64]) {
        let _ = inputs;
        out[0] = 0.0;
    }
    fn crossings(&self, inputs: &[f64]) -> Option<Crossing> {
        Some(Crossing { value: inputs[0], threshold: self.vth, dir: CrossDir::Either })
    }
}

/// `Behavior.limiter(x, lo, hi)` — a plain clamp. Input `[x]`. Continuous
/// (no jump, only a kink at the two corners) — no crossing registration
/// needed, matching [`crate::behavior::table::Table`]'s clamp-region
/// derivative convention (`0` outside `[lo, hi]`).
pub struct Limiter {
    pub lo: f64,
    pub hi: f64,
}

impl BehaviorKind for Limiter {
    fn value(&self, inputs: &[f64]) -> f64 {
        inputs[0].clamp(self.lo, self.hi)
    }
    fn jacobian(&self, inputs: &[f64], out: &mut [f64]) {
        out[0] = if inputs[0] > self.lo && inputs[0] < self.hi { 1.0 } else { 0.0 };
    }
}

/// `Behavior.vcvs(a, b, gain)` — `gain * (a - b)`. Inputs `[a, b]`.
pub struct Vcvs {
    pub gain: f64,
}

impl BehaviorKind for Vcvs {
    fn value(&self, inputs: &[f64]) -> f64 {
        self.gain * (inputs[0] - inputs[1])
    }
    fn jacobian(&self, inputs: &[f64], out: &mut [f64]) {
        let _ = inputs;
        out[0] = self.gain;
        out[1] = -self.gain;
    }
}

/// `Behavior.vccs(a, b, gm)` — `gm * (a - b)`. Inputs `[a, b]`. Same closed
/// form as [`Vcvs`]; the two differ only in which MNA row the level-1→stamp
/// adapter feeds them into (a level-2 concern, not this crate's).
pub struct Vccs {
    pub gm: f64,
}

impl BehaviorKind for Vccs {
    fn value(&self, inputs: &[f64]) -> f64 {
        self.gm * (inputs[0] - inputs[1])
    }
    fn jacobian(&self, inputs: &[f64], out: &mut [f64]) {
        let _ = inputs;
        out[0] = self.gm;
        out[1] = -self.gm;
    }
}

/// `Behavior.vco(ctrl, f0, kvco)` — reports the instantaneous
/// voltage-controlled *frequency* `f0 + kvco*ctrl`, not a phase or a
/// waveform: turning a frequency into an oscillating output is phase
/// integration, which is exactly what `ops::Operator::integrate` (`idt`)
/// already does — an author composes the two rather than this kind
/// re-implementing accumulation. Input `[ctrl]`.
pub struct Vco {
    pub f0: f64,
    pub kvco: f64,
}

impl BehaviorKind for Vco {
    fn value(&self, inputs: &[f64]) -> f64 {
        self.f0 + self.kvco * inputs[0]
    }
    fn jacobian(&self, inputs: &[f64], out: &mut [f64]) {
        let _ = inputs;
        out[0] = self.kvco;
    }
}

/// `Behavior.sum(a, b)` — `a(inputs) + b(inputs)`; both operands read the
/// same input slice.
pub struct Sum {
    pub a: Box<dyn BehaviorKind>,
    pub b: Box<dyn BehaviorKind>,
}

impl BehaviorKind for Sum {
    fn value(&self, inputs: &[f64]) -> f64 {
        self.a.value(inputs) + self.b.value(inputs)
    }
    fn jacobian(&self, inputs: &[f64], out: &mut [f64]) {
        let mut ja = vec![0.0; out.len()];
        let mut jb = vec![0.0; out.len()];
        self.a.jacobian(inputs, &mut ja);
        self.b.jacobian(inputs, &mut jb);
        for i in 0..out.len() {
            out[i] = ja[i] + jb[i];
        }
    }
    fn crossings(&self, inputs: &[f64]) -> Option<Crossing> {
        self.a.crossings(inputs).or_else(|| self.b.crossings(inputs))
    }
}

/// `Behavior.scaled(b, k)` — `k * b(inputs)`.
pub struct Scaled {
    pub inner: Box<dyn BehaviorKind>,
    pub k: f64,
}

impl BehaviorKind for Scaled {
    fn value(&self, inputs: &[f64]) -> f64 {
        self.k * self.inner.value(inputs)
    }
    fn jacobian(&self, inputs: &[f64], out: &mut [f64]) {
        self.inner.jacobian(inputs, out);
        for v in out.iter_mut() {
            *v *= self.k;
        }
    }
    fn crossings(&self, inputs: &[f64]) -> Option<Crossing> {
        self.inner.crossings(inputs)
    }
}

/// `Behavior.composed(f, g)` — `f(g(inputs))`: `g` reduces the whole input
/// vector to one scalar, which becomes `f`'s single input. This is the
/// shape that answers the PWM question (`ideal.md` §3.3): a
/// [`Hysteresis`] (`g`, reading two terminals) driving a [`Switch`] (`f`,
/// reading the one control scalar `g` produces).
pub struct Composed {
    pub f: Box<dyn BehaviorKind>,
    pub g: Box<dyn BehaviorKind>,
}

impl BehaviorKind for Composed {
    fn value(&self, inputs: &[f64]) -> f64 {
        let g_val = self.g.value(inputs);
        self.f.value(&[g_val])
    }
    fn jacobian(&self, inputs: &[f64], out: &mut [f64]) {
        let g_val = self.g.value(inputs);
        let mut df = [0.0];
        self.f.jacobian(&[g_val], &mut df);
        self.g.jacobian(inputs, out);
        for v in out.iter_mut() {
            *v *= df[0];
        }
    }
    fn crossings(&self, inputs: &[f64]) -> Option<Crossing> {
        let g_val = self.g.value(inputs);
        self.f.crossings(&[g_val]).or_else(|| self.g.crossings(inputs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparator_is_smooth_and_matches_the_tanh_closed_form() {
        let c = Comparator { lo: 0.0, hi: 5.0, gain: 10.0 };
        let v = c.value(&[0.1, 0.0]);
        let expected = 0.0 + 5.0 * 0.5 * (1.0 + (10.0 * 0.1f64).tanh());
        assert!((v - expected).abs() < 1e-12);
        let mut j = [0.0, 0.0];
        c.jacobian(&[0.1, 0.0], &mut j);
        assert!(j[0] > 0.0, "rises with a");
        assert_eq!(j[0], -j[1], "antisymmetric in a and b");
        assert!(c.crossings(&[0.1, 0.0]).is_none(), "smooth: no crossing registered");
    }

    #[test]
    fn hysteresis_holds_between_thresholds_and_flips_at_them() {
        let h = Hysteresis::new(0.6, 0.4);
        assert_eq!(h.value(&[0.5, 0.0]), 0.0, "starts low, band doesn't trip it yet");
        assert_eq!(h.value(&[0.7, 0.0]), 1.0, "rises past vth_hi");
        assert_eq!(h.value(&[0.5, 0.0]), 1.0, "holds while inside the band");
        assert_eq!(h.value(&[0.3, 0.0]), 0.0, "falls past vth_lo");
    }

    #[test]
    fn hysteresis_reports_the_crossing_it_is_next_watching_for() {
        let h = Hysteresis::new(0.6, 0.4);
        let c = h.crossings(&[0.0, 0.0]).unwrap();
        assert_eq!(c.threshold, 0.6, "starts low: watching for vth_hi");
        h.value(&[0.7, 0.0]); // flips high
        let c2 = h.crossings(&[0.7, 0.0]).unwrap();
        assert_eq!(c2.threshold, 0.4, "now high: watching for vth_lo");
    }

    #[test]
    fn switch_conducts_ron_above_threshold_roff_below() {
        let s = Switch { ron: 1.0, roff: 1e6, vth: 0.5 };
        assert_eq!(s.value(&[0.6]), 1.0);
        assert_eq!(s.value(&[0.4]), 1e-6);
    }

    #[test]
    fn limiter_clamps_and_the_derivative_is_the_table_clamp_convention() {
        let l = Limiter { lo: -1.0, hi: 1.0 };
        assert_eq!(l.value(&[5.0]), 1.0);
        assert_eq!(l.value(&[-5.0]), -1.0);
        assert_eq!(l.value(&[0.3]), 0.3);
        let mut j = [0.0];
        l.jacobian(&[5.0], &mut j);
        assert_eq!(j[0], 0.0, "clamp region: zero slope");
        l.jacobian(&[0.3], &mut j);
        assert_eq!(j[0], 1.0, "linear region: unit slope");
    }

    #[test]
    fn vcvs_and_vccs_are_the_same_closed_form() {
        let vcvs = Vcvs { gain: 3.0 };
        let vccs = Vccs { gm: 3.0 };
        assert_eq!(vcvs.value(&[1.0, 0.4]), vccs.value(&[1.0, 0.4]));
    }

    #[test]
    fn vco_reports_instantaneous_frequency() {
        let vco = Vco { f0: 1e3, kvco: 500.0 };
        assert_eq!(vco.value(&[2.0]), 2e3);
        let mut j = [0.0];
        vco.jacobian(&[2.0], &mut j);
        assert_eq!(j[0], 500.0);
    }

    #[test]
    fn hysteresis_driving_switch_is_the_pwm_composition() {
        // The acceptance case (ideal.md §3.3): a hysteretic PWM controller
        // reading two terminals, composed purely — no callback anywhere.
        let controller = Composed {
            f: Box::new(Switch { ron: 1.0, roff: 1e6, vth: 0.5 }),
            g: Box::new(Hysteresis::new(0.6, 0.4)),
        };
        // Below vth_hi: hysteresis stays low (0.0) -> switch reads 0.0 < 0.5 -> roff.
        assert_eq!(controller.value(&[0.5, 0.0]), 1e-6);
        // Past vth_hi: hysteresis flips high (1.0) -> switch reads 1.0 >= 0.5 -> ron.
        assert_eq!(controller.value(&[0.7, 0.0]), 1.0);
        // Holds while inside the band on the way back down.
        assert_eq!(controller.value(&[0.5, 0.0]), 1.0);
    }
}
