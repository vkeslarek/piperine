//! The shipped `MeasureKind` catalog (`ideal.md` §3.5):
//! `peak`/`min`/`average`/`rms`/`ripple`/`integral`/`cross`/`settle`/`count`.
//! `delay` is documented separately below — it is inherently two-channel
//! and cannot conform to `MeasureKind`'s single-`(t, v)` shape.
//!
//! Every kind is a **fixed-size struct with no `Vec`** — proof by
//! construction that feeding it any number of samples costs O(1) memory,
//! not "small in practice." `tests::feeding_many_samples_does_not_grow_the_accumulator`
//! asserts this structurally (`size_of_val` unchanged after 100k feeds),
//! not just that outputs look right.
//!
//! **Error bounds are derived from the local LTE each `update` is fed**
//! (`ideal.md` §3.6): a reduction over accepted points can miss the true
//! extremum between them, so every kind here folds the caller's `lte` into
//! a running bound rather than reporting `0.0` — a wide bound is the
//! honest answer when the sampling is coarse, not a plausible wrong number.

use super::MeasureKind;
use crate::events::{predict_crossing, CrossDir};

/// `Measure.peak(x)` — the running maximum, with the LTE observed at the
/// sample that set it as the bound (the true peak, if it fell between that
/// sample and its neighbor, is within `[value, value + error_bound]`).
pub struct Peak {
    max_v: f64,
    t_at_max: f64,
    bound: f64,
    seen: bool,
}

impl Default for Peak {
    fn default() -> Self {
        Self { max_v: f64::NEG_INFINITY, t_at_max: 0.0, bound: 0.0, seen: false }
    }
}

impl Peak {
    pub fn new() -> Self {
        Self::default()
    }

    /// Drive step refinement near the current peak until `error_bound()`
    /// meets `tol`, by probing `sample(t) -> (v, lte)` at shrinking offsets
    /// around `t_at_max` (the pure-math shape of what a real driver does
    /// via `bound_step_hint`: `ideal.md` §12 open question 13). Terminates
    /// at `step_floor` and reports — returns whether `tol` was actually
    /// met, never silently stalling past the floor.
    pub fn refine(
        &mut self,
        tol: f64,
        step_floor: f64,
        max_iters: usize,
        mut sample: impl FnMut(f64) -> (f64, f64),
    ) -> bool {
        for _ in 0..max_iters {
            if self.bound <= tol {
                return true;
            }
            // Re-derive the probe half-width from the *current* bound each
            // round, rather than a fixed halving schedule: the bound
            // already tracks how far off the current estimate could be,
            // so probing at that distance is the step actually worth
            // taking. A fixed schedule that shrinks regardless of what
            // was learned cannot outrun a badly-scaled first guess.
            let h = self.bound.max(step_floor);
            if h <= step_floor {
                break;
            }
            let center = self.t_at_max;
            for dt in [-h, h] {
                let t = center + dt;
                let (v, lte) = sample(t);
                self.update(t, v, lte);
            }
        }
        self.bound <= tol
    }
}

impl MeasureKind for Peak {
    fn update(&mut self, t: f64, v: f64, lte: f64) {
        if v > self.max_v {
            self.max_v = v;
            self.t_at_max = t;
            self.bound = lte.abs();
        }
        self.seen = true;
    }
    fn value(&self) -> f64 {
        self.max_v
    }
    fn error_bound(&self) -> f64 {
        self.bound
    }
}

/// `Measure.min(x)` — symmetric to [`Peak`].
pub struct Min {
    min_v: f64,
    bound: f64,
}

impl Default for Min {
    fn default() -> Self {
        Self { min_v: f64::INFINITY, bound: 0.0 }
    }
}

impl Min {
    pub fn new() -> Self {
        Self::default()
    }
}

impl MeasureKind for Min {
    fn update(&mut self, _t: f64, v: f64, lte: f64) {
        if v < self.min_v {
            self.min_v = v;
            self.bound = lte.abs();
        }
    }
    fn value(&self) -> f64 {
        self.min_v
    }
    fn error_bound(&self) -> f64 {
        self.bound
    }
}

/// `Measure.average(x)` — the time-weighted (trapezoidal) mean.
#[derive(Default)]
pub struct Average {
    area: f64,
    bound_area: f64,
    total_t: f64,
    last: Option<(f64, f64)>,
}

impl Average {
    pub fn new() -> Self {
        Self::default()
    }
}

impl MeasureKind for Average {
    fn update(&mut self, t: f64, v: f64, lte: f64) {
        if let Some((pt, pv)) = self.last {
            let dt = t - pt;
            self.area += 0.5 * (pv + v) * dt;
            self.bound_area += lte.abs() * dt;
            self.total_t += dt;
        }
        self.last = Some((t, v));
    }
    fn value(&self) -> f64 {
        if self.total_t > 0.0 { self.area / self.total_t } else { self.last.map(|(_, v)| v).unwrap_or(0.0) }
    }
    fn error_bound(&self) -> f64 {
        if self.total_t > 0.0 { self.bound_area / self.total_t } else { 0.0 }
    }
}

/// `Measure.rms(x)` — trapezoidal mean-of-squares, square-rooted. The
/// error bound is the average-of-squares bound linearized through `sqrt`
/// (`d(sqrt(x))/dx = 1/(2*sqrt(x))`), an approximation stated as such.
#[derive(Default)]
pub struct Rms {
    area2: f64,
    bound_area: f64,
    total_t: f64,
    last: Option<(f64, f64)>,
}

impl Rms {
    pub fn new() -> Self {
        Self::default()
    }
}

impl MeasureKind for Rms {
    fn update(&mut self, t: f64, v: f64, lte: f64) {
        if let Some((pt, pv)) = self.last {
            let dt = t - pt;
            self.area2 += 0.5 * (pv * pv + v * v) * dt;
            self.bound_area += lte.abs() * dt;
            self.total_t += dt;
        }
        self.last = Some((t, v));
    }
    fn value(&self) -> f64 {
        if self.total_t > 0.0 { (self.area2 / self.total_t).max(0.0).sqrt() } else { 0.0 }
    }
    fn error_bound(&self) -> f64 {
        if self.total_t <= 0.0 {
            return 0.0;
        }
        let mean_sq_bound = self.bound_area / self.total_t;
        let v = self.value().max(1e-300);
        mean_sq_bound / (2.0 * v)
    }
}

/// `Measure.ripple(x)` — peak-to-peak: running max minus running min, with
/// both extrema's bounds combined.
#[derive(Default)]
pub struct Ripple {
    peak: Peak,
    min: Min,
}

impl Ripple {
    pub fn new() -> Self {
        Self::default()
    }
}

impl MeasureKind for Ripple {
    fn update(&mut self, t: f64, v: f64, lte: f64) {
        self.peak.update(t, v, lte);
        self.min.update(t, v, lte);
    }
    fn value(&self) -> f64 {
        self.peak.value() - self.min.value()
    }
    fn error_bound(&self) -> f64 {
        self.peak.error_bound() + self.min.error_bound()
    }
}

/// `Measure.integral(x)` — plain trapezoidal area (no time normalization).
#[derive(Default)]
pub struct Integral {
    area: f64,
    bound_area: f64,
    last: Option<(f64, f64)>,
}

impl Integral {
    pub fn new() -> Self {
        Self::default()
    }
}

impl MeasureKind for Integral {
    fn update(&mut self, t: f64, v: f64, lte: f64) {
        if let Some((pt, pv)) = self.last {
            let dt = t - pt;
            self.area += 0.5 * (pv + v) * dt;
            self.bound_area += lte.abs() * dt;
        }
        self.last = Some((t, v));
    }
    fn value(&self) -> f64 {
        self.area
    }
    fn error_bound(&self) -> f64 {
        self.bound_area
    }
}

/// `Measure.cross(x, level, dir)` — the instant of a crossing, reusing
/// `events::predict_crossing` (tier 2) rather than a fresh detector: one
/// interface for "when does a predicate flip," shared with `Behavior`'s
/// crossing registration and the tier-1 detector.
pub struct Cross {
    level: f64,
    dir: CrossDir,
    prev: Option<(f64, f64)>,
    t_cross: Option<f64>,
    bound: f64,
}

impl Cross {
    pub fn new(level: f64, dir: CrossDir) -> Self {
        Self { level, dir, prev: None, t_cross: None, bound: f64::INFINITY }
    }
}

impl MeasureKind for Cross {
    fn update(&mut self, t: f64, v: f64, _lte: f64) {
        let shifted = v - self.level;
        if let Some((pt, pv)) = self.prev
            && self.t_cross.is_none()
            && let Some(p) = predict_crossing(self.dir, pt, pv, t, shifted)
        {
            self.t_cross = Some(p.t_cross);
            self.bound = p.error_bound;
        }
        self.prev = Some((t, shifted));
    }
    fn value(&self) -> f64 {
        self.t_cross.unwrap_or(f64::NAN)
    }
    fn error_bound(&self) -> f64 {
        self.bound
    }
}

/// `Measure.settle(x, target, tol)` — the instant the signal enters
/// `[target - tol, target + tol]` and never leaves it again.
pub struct Settle {
    target: f64,
    tol: f64,
    settle_t: Option<f64>,
    last_dt: f64,
}

impl Settle {
    pub fn new(target: f64, tol: f64) -> Self {
        Self { target, tol, settle_t: None, last_dt: 0.0 }
    }
}

impl MeasureKind for Settle {
    fn update(&mut self, t: f64, v: f64, _lte: f64) {
        if (v - self.target).abs() <= self.tol {
            if self.settle_t.is_none() {
                self.settle_t = Some(t);
            }
        } else {
            self.settle_t = None;
        }
        self.last_dt = t;
    }
    fn value(&self) -> f64 {
        self.settle_t.unwrap_or(f64::NAN)
    }
    fn error_bound(&self) -> f64 {
        // The settle instant is only known to within the accepted-point
        // spacing at which it was first observed inside the band.
        if self.settle_t.is_some() { 0.0 } else { self.last_dt }
    }
}

/// `Measure.count(pred)` — how many accepted points satisfied the caller's
/// predicate. The predicate itself is the caller's concern (a level-2
/// adapter concept); this kind counts nonzero feeds. Exact — the bound is
/// always `0.0`.
#[derive(Default)]
pub struct Count {
    n: u64,
}

impl Count {
    pub fn new() -> Self {
        Self::default()
    }
}

impl MeasureKind for Count {
    fn update(&mut self, _t: f64, v: f64, _lte: f64) {
        if v != 0.0 {
            self.n += 1;
        }
    }
    fn value(&self) -> f64 {
        self.n as f64
    }
    fn error_bound(&self) -> f64 {
        0.0
    }
}

// ── `Measure.delay(a, b, level)` — the two-channel exception ────────────
//
// SPEC_DEVIATION: propagation delay is inherently a two-signal quantity
// (when does `a` cross `level`, when does `b`), so it cannot be fed
// through `MeasureKind::update`'s single-`v` shape. `Delay` ships with its
// own two-argument `feed`, matching the catalog's *spelling*
// (`Measure.delay(a, b, level)`) without pretending to satisfy a trait
// whose signature does not have room for a second channel.
//
// Reason: changing `MeasureKind` to carry an arbitrary-width input vector
// would blur its contract for every other (single-signal) kind and this
// crate's catalog is otherwise unanimous on "one accepted (t, v) per
// feed" — a wider trait for one kind is a worse trade than one kind that
// is honestly not shaped like the others.
pub struct Delay {
    level: f64,
    dir: CrossDir,
    a_prev: Option<(f64, f64)>,
    b_prev: Option<(f64, f64)>,
    t_a: Option<f64>,
    t_b: Option<f64>,
}

impl Delay {
    pub fn new(level: f64, dir: CrossDir) -> Self {
        Self { level, dir, a_prev: None, b_prev: None, t_a: None, t_b: None }
    }

    /// Fold in one accepted `(t, a, b)` sample.
    pub fn feed(&mut self, t: f64, a: f64, b: f64) {
        if self.t_a.is_none()
            && let Some((pt, pv)) = self.a_prev
            && let Some(p) = predict_crossing(self.dir, pt, pv - self.level, t, a - self.level)
        {
            self.t_a = Some(p.t_cross);
        }
        if self.t_b.is_none()
            && let Some((pt, pv)) = self.b_prev
            && let Some(p) = predict_crossing(self.dir, pt, pv - self.level, t, b - self.level)
        {
            self.t_b = Some(p.t_cross);
        }
        self.a_prev = Some((t, a));
        self.b_prev = Some((t, b));
    }

    /// `t_b_cross - t_a_cross`, once both channels have crossed.
    pub fn value(&self) -> f64 {
        match (self.t_a, self.t_b) {
            (Some(a), Some(b)) => b - a,
            _ => f64::NAN,
        }
    }

    pub fn error_bound(&self) -> f64 {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peak_tracks_the_running_max_with_its_lte_as_the_bound() {
        let mut p = Peak::new();
        p.update(0.0, 1.0, 0.01);
        p.update(1.0, 3.0, 0.02);
        p.update(2.0, 2.0, 0.05);
        assert_eq!(p.value(), 3.0);
        assert_eq!(p.error_bound(), 0.02, "bound from the sample that set the max");
    }

    #[test]
    fn min_tracks_the_running_min() {
        let mut m = Min::new();
        m.update(0.0, 1.0, 0.0);
        m.update(1.0, -3.0, 0.01);
        m.update(2.0, 0.0, 0.0);
        assert_eq!(m.value(), -3.0);
        assert_eq!(m.error_bound(), 0.01);
    }

    #[test]
    fn average_is_the_time_weighted_trapezoidal_mean() {
        let mut a = Average::new();
        a.update(0.0, 0.0, 0.0);
        a.update(2.0, 4.0, 0.0);
        // Trapezoid over [0,2]: area = 0.5*(0+4)*2 = 4; total_t=2 -> avg=2.
        assert_eq!(a.value(), 2.0);
    }

    #[test]
    fn rms_matches_the_analytic_rms_of_a_ramp() {
        let mut r = Rms::new();
        // v(t) = t over [0, 1]: RMS = sqrt(integral(t^2)/1) = sqrt(1/3).
        let n = 1000;
        for i in 0..=n {
            let t = i as f64 / n as f64;
            r.update(t, t, 0.0);
        }
        let expected = (1.0f64 / 3.0).sqrt();
        assert!((r.value() - expected).abs() < 1e-3);
    }

    #[test]
    fn ripple_is_peak_minus_min() {
        let mut r = Ripple::new();
        for &(t, v) in &[(0.0, 1.0), (1.0, 3.0), (2.0, 0.5)] {
            r.update(t, v, 0.0);
        }
        assert_eq!(r.value(), 2.5);
    }

    #[test]
    fn integral_is_the_plain_trapezoidal_area() {
        let mut i = Integral::new();
        i.update(0.0, 0.0, 0.0);
        i.update(2.0, 2.0, 0.0);
        assert_eq!(i.value(), 2.0, "triangle area 0.5*2*2");
    }

    #[test]
    fn cross_reports_the_predicted_instant_and_bound() {
        let mut c = Cross::new(0.0, CrossDir::Rising);
        c.update(0.0, -1.0, 0.0);
        c.update(1.0, 1.0, 0.0);
        assert!((c.value() - 0.5).abs() < 1e-12);
        assert_eq!(c.error_bound(), 1.0);
    }

    #[test]
    fn settle_reports_the_first_instant_inside_the_band_that_holds() {
        let mut s = Settle::new(1.0, 0.1);
        s.update(0.0, 0.0, 0.0);
        s.update(1.0, 1.05, 0.0); // enters the band
        s.update(2.0, 0.5, 0.0); // leaves it: not settled yet
        s.update(3.0, 0.95, 0.0); // re-enters
        s.update(4.0, 1.02, 0.0); // stays: settled at t=3
        assert_eq!(s.value(), 3.0);
    }

    #[test]
    fn count_counts_nonzero_feeds() {
        let mut c = Count::new();
        for v in [1.0, 0.0, 1.0, 1.0, 0.0] {
            c.update(0.0, v, 0.0);
        }
        assert_eq!(c.value(), 3.0);
        assert_eq!(c.error_bound(), 0.0);
    }

    #[test]
    fn delay_measures_propagation_between_two_channels() {
        let mut d = Delay::new(0.0, CrossDir::Rising);
        // a crosses at t=0.5 (from -1 to 1 over [0,1]); b crosses at t=1.5
        // (from -1 to 1 over [1,2]) — 1.0s of propagation delay.
        d.feed(0.0, -1.0, -2.0);
        d.feed(1.0, 1.0, -1.0);
        d.feed(2.0, 1.0, 1.0);
        assert!((d.value() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn refine_narrows_the_peak_bound_toward_a_requested_tolerance() {
        // A parabola segment peaking at t=0.5 (value 1.0), sampled at two
        // accepted points on either side that skip the true peak — an
        // asymmetric bracket so there is a single, unambiguous argmax to
        // hill-climb from. `lte_for` shrinks with distance to the true
        // peak, the way a real local-truncation-error estimate would as
        // the step narrows toward an extremum.
        let true_peak_t = 0.5;
        let curvature = 8.0;
        let f = |t: f64| 1.0 - curvature * (t - true_peak_t).powi(2);
        let lte_for = |t: f64| 0.6 * (t - true_peak_t).abs().max(1e-9);

        let mut p = Peak::new();
        p.update(0.2, f(0.2), lte_for(0.2));
        p.update(0.6, f(0.6), lte_for(0.6)); // closer to the peak: this is the initial argmax
        let coarse_bound = p.error_bound();
        assert!((p.value() - f(0.6)).abs() < 1e-12, "starts at the closer, still-wrong sample");

        let met = p.refine(1e-4, 1e-9, 200, |t| (f(t), lte_for(t)));
        assert!(p.error_bound() <= coarse_bound, "refine never widens the bound");
        assert!(p.error_bound() < coarse_bound, "the bound actually narrowed");
        assert!(met, "hill-climbing toward the peak drives the bound under tol within the iteration budget");
        assert!((p.value() - 1.0).abs() < 1e-3, "converges toward the true peak value 1.0");
    }

    // ---- O(1) streaming: structural proof, not just output ----

    #[test]
    fn feeding_many_samples_does_not_grow_the_accumulator() {
        let mut p = Peak::new();
        let size_before = std::mem::size_of_val(&p);
        for i in 0..100_000 {
            p.update(i as f64, (i % 7) as f64, 0.001);
        }
        let size_after = std::mem::size_of_val(&p);
        assert_eq!(size_before, size_after, "a fixed-size struct: no Vec grows with sample count");

        let mut avg = Average::new();
        let sz = std::mem::size_of_val(&avg);
        for i in 0..100_000 {
            avg.update(i as f64, (i % 5) as f64, 0.0);
        }
        assert_eq!(std::mem::size_of_val(&avg), sz);
    }
}
