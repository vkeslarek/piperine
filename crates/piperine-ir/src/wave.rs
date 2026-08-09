//! `Wave` — value from time, `v(t)`, with no coupling to circuit state (its
//! Jacobian contribution is trivial: it fills an RHS, it is an independent
//! source). Unlike [`crate::behavior`], a `Wave` never reads the solution.
//!
//! `ideal.md` §3.2/§12.10:
//! - Every kind but [`Wave::FromFn`] evaluates natively with **zero host
//!   calls** — there is no callback field to invoke; [`Wave::Sampled`]
//!   accepts an arbitrary precomputed array at the same zero cost.
//! - `Wave.from_fn` is the one kind that costs a host call per evaluation:
//!   available, priced, and never the default.
//! - A `Wave`'s discontinuities are known a priori (its parameters are
//!   literals), so it publishes its own breakpoints through
//!   [`Wave::pending_edges`] — the same "publish your own discontinuities"
//!   shape as [`crate::ops::Operator::pending_edges`] (`ideal.md` §3.9 A
//!   rule (e)): one interface, not a special case per kind.
//! - Each kind declares its own **DC value**, following the SPICE
//!   convention (§12.10) rather than evaluating `value(0.0)` generically —
//!   `sin`'s DC value is its offset, not `offset + amp*sin(phase)`, and
//!   `pwl`/`sampled`'s DC value is their *first point*, not necessarily the
//!   value at `t = 0`.

use std::sync::Arc;

/// A discontinuity-free time-domain descriptor. Composition kinds
/// ([`Wave::Sum`], [`Wave::Scaled`], [`Wave::Shifted`]) hold their operands
/// boxed so a `Wave` tree has one owning root.
pub enum Wave {
    /// `Wave.const(v)` — a constant.
    Const { v: f64 },
    /// `Wave.ramp(v0, v1, dur)` — linear from `v0` at `t = 0` to `v1` at
    /// `t = dur`, holding `v1` after and `v0` before.
    Ramp { v0: f64, v1: f64, dur: f64 },
    /// `Wave.pulse(lo, hi, delay, rise, width, fall, period)` — the
    /// ngspice-faithful `PULSE` shape (`ng_pulse` in
    /// `headers/spice/sources.phdl`): `lo` until `delay`, ramps to `hi`
    /// over `rise`, holds `hi` for `width`, ramps back over `fall`, then
    /// `lo` until `period` repeats (no repeat when `period <= 0`).
    Pulse { lo: f64, hi: f64, delay: f64, rise: f64, width: f64, fall: f64, period: f64 },
    /// `Wave.sin(offset, amp, freq, phase, damping)` — `offset +
    /// amp*sin(2*pi*freq*t + phase)*exp(-damping*t)`, `phase` in radians.
    Sin { offset: f64, amp: f64, freq: f64, phase: f64, damping: f64 },
    /// `Wave.exp(v0, v1, tau, delay)` — a single-stage exponential
    /// approach: `v0` until `delay`, then `v0 + (v1-v0)*(1 -
    /// exp(-(t-delay)/tau))`.
    Exp { v0: f64, v1: f64, tau: f64, delay: f64 },
    /// `Wave.sigmoid(lo, hi, t0, tau)` — a logistic S-curve: `lo + (hi-lo)
    /// / (1 + exp(-(t-t0)/tau))`.
    Sigmoid { lo: f64, hi: f64, t0: f64, tau: f64 },
    /// `Wave.pwl([(t, v), …])` — piecewise-linear, authored inline. Points
    /// must be sorted by `t`; clamped flat beyond both ends, same as
    /// [`crate::behavior::table::Table`] but on the time axis.
    Pwl { points: Vec<(f64, f64)> },
    /// `Wave.sampled(t_array, v_array)` — the same shape as `Pwl`, for an
    /// arbitrary precomputed (e.g. numpy) array. Zero host cost either way
    /// — the two variants exist to name the authoring path, not to differ
    /// in evaluation.
    Sampled { t: Vec<f64>, v: Vec<f64> },
    /// `Wave.from_fn(f)` — a host callback, evaluated whenever the solver
    /// asks. The one kind that is not zero-cost.
    ///
    // SPEC_DEVIATION (dv-tester T16/T18): `Arc<dyn Fn + Send + Sync>`
    // rather than the `Rc<dyn Fn>` this crate started with. Reason: once a
    // `Wave` is installed inside a `HostElement` (T14/T16) and reaches the
    // solver as `Box<dyn Element>`, it inherits `Element`'s own
    // unconditional `Send + Sync` bound (`AnalogDevice: Send + Sync`,
    // `piperine-solver/src/core/element.rs`) — a bound `piperine-ir`
    // had no reason to anticipate before level 1 met level 0. `Rc` fails
    // that bound structurally regardless of which `Wave` variant is
    // actually constructed (Rust's auto-trait analysis is type-wide, not
    // value-wide), which made every `HostElement` embedding *any* `Wave` —
    // even a plain `Wave::constant` — fail to compile. The fix is a pure
    // marker-trait tightening: same external API (`.clone()`, call-through
    // `f(t)`), no behavior change, no new logic.
    FromFn(Arc<dyn Fn(f64) -> f64 + Send + Sync>),
    /// `Wave.sum(a, b)` — `a(t) + b(t)`.
    Sum(Box<Wave>, Box<Wave>),
    /// `Wave.scaled(w, k)` — `k * w(t)`.
    Scaled(Box<Wave>, f64),
    /// `Wave.shifted(w, dt)` — `w(t - dt)`.
    Shifted(Box<Wave>, f64),
}

impl Wave {
    /// Construct helpers mirroring the catalog spelling (`ideal.md` §3.2).
    pub fn constant(v: f64) -> Self {
        Wave::Const { v }
    }
    pub fn ramp(v0: f64, v1: f64, dur: f64) -> Self {
        Wave::Ramp { v0, v1, dur }
    }
    pub fn pulse(lo: f64, hi: f64, delay: f64, rise: f64, width: f64, fall: f64, period: f64) -> Self {
        Wave::Pulse { lo, hi, delay, rise, width, fall, period }
    }
    pub fn sin(offset: f64, amp: f64, freq: f64, phase: f64, damping: f64) -> Self {
        Wave::Sin { offset, amp, freq, phase, damping }
    }
    pub fn exp(v0: f64, v1: f64, tau: f64, delay: f64) -> Self {
        Wave::Exp { v0, v1, tau, delay }
    }
    pub fn sigmoid(lo: f64, hi: f64, t0: f64, tau: f64) -> Self {
        Wave::Sigmoid { lo, hi, t0, tau }
    }
    pub fn pwl(points: Vec<(f64, f64)>) -> Self {
        Wave::Pwl { points }
    }
    pub fn sampled(t: Vec<f64>, v: Vec<f64>) -> Self {
        Wave::Sampled { t, v }
    }
    pub fn from_fn(f: Arc<dyn Fn(f64) -> f64 + Send + Sync>) -> Self {
        Wave::FromFn(f)
    }
    pub fn sum(a: Wave, b: Wave) -> Self {
        Wave::Sum(Box::new(a), Box::new(b))
    }
    pub fn scaled(w: Wave, k: f64) -> Self {
        Wave::Scaled(Box::new(w), k)
    }
    pub fn shifted(w: Wave, dt: f64) -> Self {
        Wave::Shifted(Box::new(w), dt)
    }

    /// The value at time `t`. Zero host calls for every kind but
    /// [`Wave::FromFn`], where evaluating *is* the host call.
    pub fn value(&self, t: f64) -> f64 {
        match self {
            Wave::Const { v } => *v,
            Wave::Ramp { v0, v1, dur } => {
                if t <= 0.0 {
                    *v0
                } else if t >= *dur || *dur <= 0.0 {
                    *v1
                } else {
                    v0 + (v1 - v0) * t / dur
                }
            }
            Wave::Pulse { lo, hi, delay, rise, width, fall, period } => {
                pulse_value(t, *lo, *hi, *delay, *rise, *width, *fall, *period)
            }
            Wave::Sin { offset, amp, freq, phase, damping } => {
                offset + amp * (2.0 * std::f64::consts::PI * freq * t + phase).sin() * (-damping * t).exp()
            }
            Wave::Exp { v0, v1, tau, delay } => {
                if t <= *delay {
                    *v0
                } else {
                    v0 + (v1 - v0) * (1.0 - (-(t - delay) / tau).exp())
                }
            }
            Wave::Sigmoid { lo, hi, t0, tau } => lo + (hi - lo) / (1.0 + (-(t - t0) / tau).exp()),
            Wave::Pwl { points } => pwl_value(points, t),
            Wave::Sampled { t: ts, v } => {
                let points: Vec<(f64, f64)> = ts.iter().copied().zip(v.iter().copied()).collect();
                pwl_value(&points, t)
            }
            Wave::FromFn(f) => f(t),
            Wave::Sum(a, b) => a.value(t) + b.value(t),
            Wave::Scaled(w, k) => k * w.value(t),
            Wave::Shifted(w, dt) => w.value(t - dt),
        }
    }

    /// The DC (operating-point) value this `Wave` contributes when a
    /// sequencer is present but the analysis is DC/OP — the coroutine does
    /// not run, so each installed `Wave` must state its own convention
    /// (`ideal.md` §12.10), rather than this being `value(0.0)` generically
    /// (which would be wrong for `sin`, `sigmoid`, `pulse` with nonzero
    /// `delay`, and any `pwl`/`sampled` not starting at `t = 0`).
    pub fn dc_value(&self) -> f64 {
        match self {
            Wave::Const { v } => *v,
            Wave::Ramp { v0, .. } => *v0,
            Wave::Pulse { lo, .. } => *lo,
            Wave::Sin { offset, .. } => *offset,
            Wave::Exp { v0, .. } => *v0,
            Wave::Sigmoid { lo, .. } => *lo,
            Wave::Pwl { points } => points.first().map(|&(_, v)| v).unwrap_or(0.0),
            Wave::Sampled { v, .. } => v.first().copied().unwrap_or(0.0),
            // Evaluated at t = 0, per §12.10 — the one DC value that costs a
            // host call, paid once at the operating point, not per Newton
            // iteration.
            Wave::FromFn(f) => f(0.0),
            // Not in §12.10's table (composition kinds); extended
            // consistently with it: the DC value of a sum/scale/shift is
            // the same combination of its operands' *declared* DC values,
            // not a re-evaluation of the composed `value` at t = 0.
            Wave::Sum(a, b) => a.dc_value() + b.dc_value(),
            Wave::Scaled(w, k) => k * w.dc_value(),
            Wave::Shifted(w, _) => w.dc_value(),
        }
    }

    /// Discontinuities strictly inside `(from, end]`, appended to `out` —
    /// the same shape as [`crate::ops::Operator::pending_edges`]
    /// (`ideal.md` §3.9 A rule (e)/§3.2): a stateful signal element
    /// publishes its own future discontinuities through one interface, not
    /// a per-kind special case. A `Wave`'s edges are known a priori (its
    /// parameters are literals), so — unlike `Operator::pending_edges` —
    /// this never depends on accepted state.
    pub fn pending_edges(&self, from: f64, end: f64, out: &mut Vec<f64>) {
        match self {
            Wave::Const { .. } => {}
            Wave::Ramp { v0, v1, dur } => {
                if v0 != v1 {
                    push_in(out, 0.0, from, end);
                    push_in(out, *dur, from, end);
                }
            }
            Wave::Pulse { delay, rise, width, fall, period, .. } => {
                pulse_edges(*delay, *rise, *width, *fall, *period, from, end, out);
            }
            // Continuous (C-infinity): no a-priori corner to land on.
            Wave::Sin { .. } | Wave::Sigmoid { .. } => {}
            Wave::Exp { delay, .. } => push_in(out, *delay, from, end),
            Wave::Pwl { points } => {
                for &(t, _) in points {
                    push_in(out, t, from, end);
                }
            }
            Wave::Sampled { t, .. } => {
                for &tp in t {
                    push_in(out, tp, from, end);
                }
            }
            // Arbitrary and host-defined: no discontinuity is knowable a
            // priori.
            Wave::FromFn(_) => {}
            Wave::Sum(a, b) => {
                a.pending_edges(from, end, out);
                b.pending_edges(from, end, out);
            }
            Wave::Scaled(w, _) => w.pending_edges(from, end, out),
            Wave::Shifted(w, dt) => {
                let mut shifted = Vec::new();
                w.pending_edges(from - dt, end - dt, &mut shifted);
                out.extend(shifted.into_iter().map(|t| t + dt));
            }
        }
    }
}

fn push_in(out: &mut Vec<f64>, t: f64, from: f64, end: f64) {
    if t > from && t <= end {
        out.push(t);
    }
}

/// ngspice-faithful `PULSE(V1 V2 TD TR TF PW PER)`, matching `ng_pulse` in
/// `headers/spice/sources.phdl` — periodic when `period > 0`.
#[allow(clippy::too_many_arguments)]
fn pulse_value(t: f64, lo: f64, hi: f64, delay: f64, rise: f64, width: f64, fall: f64, period: f64) -> f64 {
    let mut tp = t - delay;
    if period > 0.0 && tp > period {
        tp -= period * (tp / period).floor();
    }
    if tp <= 0.0 || tp >= rise + width + fall {
        return lo;
    }
    if tp < rise {
        return lo + (hi - lo) * tp / rise;
    }
    if tp <= rise + width {
        return hi;
    }
    hi + (lo - hi) * (tp - rise - width) / fall
}

/// One pulse cycle's corner instants, repeated every `period` (or once,
/// when `period <= 0`) within `(from, end]`.
#[allow(clippy::too_many_arguments)]
fn pulse_edges(delay: f64, rise: f64, width: f64, fall: f64, period: f64, from: f64, end: f64, out: &mut Vec<f64>) {
    let corners = [delay, delay + rise, delay + rise + width, delay + rise + width + fall];
    if period <= 0.0 {
        for &c in &corners {
            push_in(out, c, from, end);
        }
        return;
    }
    // Every corner, at every period offset whose cycle could touch the
    // window — a bounded range, not an open-ended loop.
    let k_min = ((from - delay) / period).floor() as i64 - 1;
    let k_max = ((end - delay) / period).ceil() as i64 + 1;
    for k in k_min..=k_max {
        let base = k as f64 * period;
        for &c in &corners {
            push_in(out, c + base, from, end);
        }
    }
}

/// Piecewise-linear evaluation at `t`, clamped flat beyond both ends. Not
/// `behavior::table::Table` because that requires a *strictly increasing*
/// axis for a value/derivative pair; a `Wave`'s time axis is assumed
/// sorted but only value is needed here (a `Wave` has no Jacobian).
fn pwl_value(points: &[(f64, f64)], t: f64) -> f64 {
    let Some(&(t0, v0)) = points.first() else { return 0.0 };
    let Some(&(tn, vn)) = points.last() else { return 0.0 };
    if t <= t0 {
        return v0;
    }
    if t >= tn {
        return vn;
    }
    for w in points.windows(2) {
        let (ta, va) = w[0];
        let (tb, vb) = w[1];
        if t >= ta && t <= tb {
            if tb == ta {
                return vb;
            }
            return va + (vb - va) * (t - ta) / (tb - ta);
        }
    }
    vn
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    // ---- native kinds: zero host calls, per §12.10's DC convention ----

    #[test]
    fn const_value_and_dc() {
        let w = Wave::constant(3.5);
        assert_eq!(w.value(0.0), 3.5);
        assert_eq!(w.value(100.0), 3.5);
        assert_eq!(w.dc_value(), 3.5, "const -> its value");
    }

    #[test]
    fn ramp_value_and_dc_is_start() {
        let w = Wave::ramp(1.0, 5.0, 2.0);
        assert_eq!(w.value(0.0), 1.0);
        assert_eq!(w.value(1.0), 3.0);
        assert_eq!(w.value(2.0), 5.0);
        assert_eq!(w.value(10.0), 5.0, "holds v1 after dur");
        assert_eq!(w.dc_value(), 1.0, "ramp -> v0 (start)");
    }

    #[test]
    fn pulse_matches_ngspice_pulse_shape() {
        let w = Wave::pulse(0.0, 5.0, 1.0, 0.5, 1.0, 0.5, 0.0);
        assert_eq!(w.value(0.5), 0.0, "before delay");
        assert_eq!(w.value(1.25), 2.5, "mid-rise");
        assert_eq!(w.value(2.0), 5.0, "in the flat top");
        assert_eq!(w.value(2.75), 2.5, "mid-fall");
        assert_eq!(w.value(3.5), 0.0, "after the pulse, no repeat");
        assert_eq!(w.dc_value(), 0.0, "pulse -> lo (initial level)");
    }

    #[test]
    fn pulse_repeats_with_period() {
        let w = Wave::pulse(0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 2.0);
        assert_eq!(w.value(0.5), 1.0, "first cycle high");
        assert_eq!(w.value(2.5), 1.0, "second cycle high");
        assert_eq!(w.value(1.5), 0.0, "first cycle low tail");
    }

    #[test]
    fn sin_dc_is_offset_not_the_t0_sample() {
        // A nonzero phase makes value(0.0) != offset — the DC convention
        // is offset regardless (§12.10), not a generic value(0.0).
        let w = Wave::sin(1.0, 2.0, 10.0, std::f64::consts::FRAC_PI_2, 0.0);
        assert!((w.value(0.0) - 3.0).abs() < 1e-12, "sanity: sin(pi/2)=1 so value(0)=1+2*1=3");
        assert_eq!(w.dc_value(), 1.0, "sin -> offset, per §12.10, not value(0.0)");
    }

    #[test]
    fn exp_ramps_to_v1_and_dc_is_v0() {
        let w = Wave::exp(0.0, 10.0, 1.0, 0.0);
        assert_eq!(w.value(0.0), 0.0);
        assert!(w.value(5.0) > 9.0, "approaches v1");
        assert_eq!(w.dc_value(), 0.0, "exp -> v0 (initial level)");
    }

    #[test]
    fn sigmoid_dc_is_lo_not_the_t0_sample() {
        // t0 > 0 makes value(0.0) land above lo — the DC convention is lo
        // regardless.
        let w = Wave::sigmoid(0.0, 1.0, 5.0, 1.0);
        assert!(w.value(0.0) > 0.0, "sanity: not yet at lo by t=0");
        assert_eq!(w.dc_value(), 0.0, "sigmoid -> lo, per §12.10");
    }

    #[test]
    fn pwl_and_sampled_dc_is_first_point_not_value_at_t0() {
        // First point at t=1.0, not t=0 — dc_value must be the first
        // point's y, not value(0.0) (which clamps to the same thing here,
        // so also test a case where the axis starts after t=0 with a
        // different value than a naive value(0.0) probe would suggest).
        let pwl = Wave::pwl(vec![(1.0, 7.0), (2.0, 9.0)]);
        assert_eq!(pwl.dc_value(), 7.0, "pwl -> first point");
        assert_eq!(pwl.value(0.0), 7.0, "clamped flat before the first point too");

        let sampled = Wave::sampled(vec![1.0, 2.0], vec![7.0, 9.0]);
        assert_eq!(sampled.dc_value(), 7.0, "sampled -> first point, same as pwl");
        assert_eq!(sampled.value(1.5), 8.0);
    }

    #[test]
    fn composition_sum_scaled_shifted() {
        let a = Wave::constant(2.0);
        let b = Wave::ramp(0.0, 1.0, 1.0);
        let sum = Wave::sum(a, b);
        assert_eq!(sum.value(0.5), 2.5);

        let scaled = Wave::scaled(Wave::constant(3.0), 2.0);
        assert_eq!(scaled.value(0.0), 6.0);

        let shifted = Wave::shifted(Wave::ramp(0.0, 1.0, 1.0), 1.0);
        assert_eq!(shifted.value(1.5), 0.5, "shifted: same shape, 1s later");
        assert_eq!(shifted.value(0.5), 0.0, "before the shifted ramp starts");
    }

    // ---- pending_edges: the shared "publish your own discontinuities" shape ----

    #[test]
    fn const_and_smooth_kinds_publish_no_edges() {
        let mut out = Vec::new();
        Wave::constant(1.0).pending_edges(0.0, 10.0, &mut out);
        Wave::sin(0.0, 1.0, 1.0, 0.0, 0.0).pending_edges(0.0, 10.0, &mut out);
        Wave::sigmoid(0.0, 1.0, 1.0, 1.0).pending_edges(0.0, 10.0, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn ramp_publishes_start_and_end_edges() {
        let w = Wave::ramp(0.0, 1.0, 2.0);
        let mut out = Vec::new();
        w.pending_edges(-1.0, 10.0, &mut out);
        assert_eq!(out, vec![0.0, 2.0]);
    }

    #[test]
    fn pulse_publishes_all_four_corners_and_repeats() {
        let w = Wave::pulse(0.0, 1.0, 1.0, 0.5, 1.0, 0.5, 3.0);
        let mut out = Vec::new();
        w.pending_edges(0.0, 3.5, &mut out);
        out.sort_by(|a, b| a.partial_cmp(b).unwrap());
        // First cycle: delay=1, +rise=1.5, +width=2.5, +fall=3.0; second
        // cycle's first corner at 1+3=4.0 falls outside (0, 3.5].
        assert_eq!(out, vec![1.0, 1.5, 2.5, 3.0]);
    }

    #[test]
    fn pwl_and_sampled_publish_their_own_points() {
        let mut out = Vec::new();
        Wave::pwl(vec![(1.0, 0.0), (2.0, 1.0), (3.0, 0.0)]).pending_edges(0.0, 10.0, &mut out);
        assert_eq!(out, vec![1.0, 2.0, 3.0]);

        let mut out2 = Vec::new();
        Wave::sampled(vec![1.0, 2.0], vec![0.0, 1.0]).pending_edges(0.0, 10.0, &mut out2);
        assert_eq!(out2, vec![1.0, 2.0]);
    }

    #[test]
    fn an_edge_exactly_at_the_window_right_boundary_is_included() {
        // `push_in`'s window test is inclusive on the right (`t <= end`):
        // an edge landing exactly on `end` must still be reported, not
        // dropped. Isolate the right-boundary case from the left-boundary
        // exclusion (`t > from`) by starting the window just after the
        // ramp's other edge (t=0.0) so only the t=2.0 edge is in range.
        let w = Wave::ramp(0.0, 1.0, 2.0);
        let mut out = Vec::new();
        w.pending_edges(1.0, 2.0, &mut out);
        assert_eq!(out, vec![2.0], "an edge exactly at the window's right boundary (end) must be included");
    }

    #[test]
    fn shifted_edges_carry_the_shift() {
        let w = Wave::shifted(Wave::ramp(0.0, 1.0, 2.0), 5.0);
        let mut out = Vec::new();
        w.pending_edges(0.0, 10.0, &mut out);
        assert_eq!(out, vec![5.0, 7.0], "ramp's [0,2] edges shifted by +5");
    }

    // ---- from_fn: the one kind that costs a host call, contrast by count ----

    #[test]
    fn from_fn_costs_exactly_one_host_call_per_evaluation() {
        use std::sync::atomic::{AtomicU32, Ordering};
        let calls = Arc::new(AtomicU32::new(0));
        let counted = calls.clone();
        let w = Wave::from_fn(Arc::new(move |t: f64| {
            counted.fetch_add(1, Ordering::Relaxed);
            t * 2.0
        }));
        assert_eq!(calls.load(Ordering::Relaxed), 0, "constructing must not evaluate");
        assert_eq!(w.value(3.0), 6.0);
        assert_eq!(calls.load(Ordering::Relaxed), 1, "one call per value() evaluation");
        w.value(1.0);
        w.value(2.0);
        assert_eq!(calls.load(Ordering::Relaxed), 3, "each evaluation costs exactly one call");
        assert_eq!(w.dc_value(), 0.0, "from_fn dc -> evaluated at t=0");
        assert_eq!(calls.load(Ordering::Relaxed), 4, "dc_value pays its own one call, per §12.10");
    }

    #[test]
    fn non_from_fn_kinds_never_touch_the_shared_counter() {
        // Contrast with the test above: a counter that only `from_fn`
        // could increment stays at zero after exercising every other kind
        // many times — because no other variant holds (or could invoke) a
        // callback at all; this is provable at the type level (no `Fn`
        // field exists on these variants), and this test demonstrates it
        // operationally too.
        let calls = Cell::new(0u32);
        let kinds: Vec<Wave> = vec![
            Wave::constant(1.0),
            Wave::ramp(0.0, 1.0, 1.0),
            Wave::pulse(0.0, 1.0, 0.0, 0.1, 0.1, 0.1, 1.0),
            Wave::sin(0.0, 1.0, 1.0, 0.0, 0.0),
            Wave::exp(0.0, 1.0, 1.0, 0.0),
            Wave::sigmoid(0.0, 1.0, 0.5, 0.1),
            Wave::pwl(vec![(0.0, 0.0), (1.0, 1.0)]),
            Wave::sampled(vec![0.0, 1.0], vec![0.0, 1.0]),
        ];
        for kind in &kinds {
            for i in 0..100 {
                kind.value(i as f64 * 0.01);
            }
            kind.dc_value();
        }
        assert_eq!(calls.get(), 0, "no non-from_fn kind can reach the counter — none holds a callback");
    }
}
