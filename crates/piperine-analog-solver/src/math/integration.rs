//! Numerical integration for transient analysis: TR-BDF2 is the sole
//! transient scheme (there is no method-selection surface).
//!
//! The companion coefficients `(c0, c1, c2)` a kernel stamps into the
//! reactive form `i_C = c0·Q_n + c1·Q_{n-1} + c2·Q_{n-2}` come from
//! [`TrBdf2::phase_coeffs`] / [`TrBdf2::stage_coeffs`], so the codegen
//! kernel does not need to know the integration formula family. (Breakpoint
//! landing belongs to the owner protocol, not to a separate trait.)

use crate::tolerances::Tolerances;

/// BDF2 (non-uniform) coefficients for `dQ/dt ≈ c0·Q_n + c1·Q_{n-1} + c2·Q_{n-2}`.
///
/// `dt0 = t_n − t_{n-1}` is the current step; `dt1 = t_{n-1} − t_{n-2}` is the
/// previous step. With `dt1 = 0` (first step), the formula collapses to
/// backward-Euler to avoid division by zero.
fn bdf2_coeffs(dt0: f64, dt1: f64) -> (f64, f64, f64) {
    if dt1 <= 0.0 || !dt1.is_finite() {
        // First step (no history) — fall back to backward-Euler.
        return (1.0 / dt0, -1.0 / dt0, 0.0);
    }
    let sum = dt0 + dt1;
    let c0 = (2.0 * dt0 + dt1) / (dt0 * sum);
    let c1 = -sum / (dt0 * dt1);
    let c2 = dt0 / (dt1 * sum);
    (c0, c1, c2)
}

// ── TR-BDF2 (sole transient integration scheme) ────────────────────────────
//
// TR-BDF2 (Hosea & Shampine, 1996) advances `[t_n, t_{n+1}]` in two stages
// with the damping parameter γ = 2 − √2: a Trapezoidal stage over `γh` to the
// intermediate point `x_{n+γ}`, then a BDF2 stage over the remaining `(1−γ)h`
// to `x_{n+1}` using `x_{n+γ}` and `x_n` as history. The BDF2 stage is a
// native low-pass filter, giving L-stability (no trapezoidal ringing on
// stiff/LC circuits). This is the only transient scheme — there is no
// method-selection surface.

/// Which sub-step of a TR-BDF2 step the kernel is stamping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrBdf2Phase {
    /// Trapezoidal stage over `γh` → solves for the intermediate `x_{n+γ}`.
    Trapezoidal,
    /// BDF2 stage over `(1−γ)h` → solves for `x_{n+1}` from `x_{n+γ}` and `x_n`.
    Bdf2,
}

/// One `ddt` state's history across a TR-BDF2 step: the argument of one `ddt` instance
/// (§15.2 rule 1), the quantity the integrator advances and the truncation error is measured on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DdtState {
    /// The state at `t_n`, `t_{n+γ}` and `t_{n+1}`.
    pub q: [f64; 3],
    /// The companion current (the state's derivative) at `t_n` and `t_{n+1}`.
    pub i: [f64; 2],
    /// The instance's own `abstol` (§14, §17.2 Resolution), when it gave one: it replaces the
    /// run's flow `abstol` for this state.
    pub abstol: Option<f64>,
}

/// TR-BDF2 — the sole transient integration scheme. Owns the per-phase
/// companion coefficients and the Milne-device LTE estimate.
pub struct TrBdf2;

impl TrBdf2 {
    /// The damping parameter γ = 2 − √2 (Hosea & Shampine). With this value
    /// both stages share equal weight and the method is L-stable. Pre-computed
    /// because `f64::sqrt` is not `const` on stable Rust.
    pub const GAMMA: f64 = 0.5857864376269049; // = 2.0 − 1.4142135623730951

    /// Companion coefficients `(c0, c1, c2)` for
    /// `i_C = c0·Q + c1·Q_hist1 + c2·Q_hist2` at the given phase and step `h`.
    ///
    /// - [`Trapezoidal`](TrBdf2Phase::Trapezoidal) over the sub-step `γh`:
    ///   `(2/(γh), −2/(γh), 0)` — the unknown is `Q_{n+γ}`, history is `Q_n`.
    /// - [`Bdf2`](TrBdf2Phase::Bdf2) over the sub-step `(1−γ)h`: the non-uniform
    ///   BDF2 coefficients with previous sub-step `γh` — the unknown is
    ///   `Q_{n+1}`, history is `Q_{n+γ}` then `Q_n`.
    pub fn phase_coeffs(phase: TrBdf2Phase, h: f64) -> (f64, f64, f64) {
        match phase {
            TrBdf2Phase::Trapezoidal => {
                let sub = Self::GAMMA * h;
                (2.0 / sub, -2.0 / sub, 0.0)
            }
            // The BDF2 stage reuses the existing non-uniform formula with the
            // current sub-step `(1−γ)h` and the previous sub-step `γh`.
            TrBdf2Phase::Bdf2 => bdf2_coeffs((1.0 - Self::GAMMA) * h, Self::GAMMA * h),
        }
    }

    /// [`phase_coeffs`](Self::phase_coeffs) with the restart convention
    /// applied — the form device companions stamp.
    ///
    /// The trapezoidal companion `i_{n+γ} = (2/(γh))(Q_{n+γ} − Q_n) − i_n`
    /// needs the previous derivative term `i_n` (capacitor current / inductor
    /// branch voltage). Across a discontinuity — breakpoint edge or a
    /// restarted run — the history is unusable and `prev_h` is 0: keeping the
    /// full `2/(γh)` weight while taking `i_n = 0` doubles the derivative
    /// estimate for the first step (an O(h)·i_n error that scales with the
    /// post-edge current). The standard restart convention degrades that
    /// first TR stage to backward Euler over the `γh` sub-step:
    /// `(1/(γh), −1/(γh), 0)`, no previous-derivative term. The BDF2 stage
    /// only spans the current step and is unaffected.
    pub fn stage_coeffs(phase: TrBdf2Phase, h: f64, prev_h: f64) -> (f64, f64, f64) {
        if matches!(phase, TrBdf2Phase::Trapezoidal) && prev_h <= 0.0 {
            let sub = Self::GAMMA * h;
            return (1.0 / sub, -1.0 / sub, 0.0);
        }
        Self::phase_coeffs(phase, h)
    }

    /// The coefficients of a stage of the step that reads a jump (§15.2.6): the step is first
    /// order throughout. The trapezoidal stage is the restart's backward Euler over `γh`,
    /// `(1/(γh), −1/(γh), 0)`; the second stage is backward Euler over the whole step from the
    /// step's start, `(1/h, 0, −1/h)` on `(Q_{n+1}, Q_{n+γ}, Q_n)`, so the derivative the step
    /// ends with is its secant `(Q_{n+1} − Q_n)/h` and has the sign of the change.
    pub fn jump_coeffs(phase: TrBdf2Phase, h: f64) -> (f64, f64, f64) {
        match phase {
            TrBdf2Phase::Trapezoidal => Self::stage_coeffs(phase, h, 0.0),
            TrBdf2Phase::Bdf2 => (1.0 / h, 0.0, -1.0 / h),
        }
    }

    /// The truncation error of a step over its `ddt` states, normalized as ngspice's `CKTterr`:
    /// per state, Milne's difference `|q_{n+1} − (q_{n+γ} + ((1−γ)/γ)(q_{n+γ} − q_n))|` over
    /// [`state_bound`](Self::state_bound); the worst is returned. No state is skipped and a
    /// run with no state returns `0.0` (nothing limits the step).
    pub fn state_lte(states: &[DdtState], tol: &Tolerances, h: f64) -> f64 {
        let slope_scale = (1.0 - Self::GAMMA) / Self::GAMMA;
        Self::worst_ratio(states, tol, h, |state| {
            let [q_n, q_gamma, q_n1] = state.q;
            (q_n1 - (q_gamma + slope_scale * (q_gamma - q_n))).abs()
        })
    }

    /// The truncation error of the step that reads a jump (§15.2 rule 6), normalized like
    /// [`state_lte`](Self::state_lte). Both stages of that step are backward Euler from `q_n`, so
    /// the two secants `s_γ = (q_{n+γ} − q_n)/(γh)` and `s_1 = (q_{n+1} − q_n)/h` differ by
    /// `q''·(1−γ)h/2`, and backward Euler's local error over `h` is `(h²/2)·q''`, which gives
    /// `h·|s_1 − s_γ|/(1−γ)` per state. It uses only the step's own two stages, nothing from
    /// before the jump, so the jump itself is not measured. A run with no state returns `0.0`.
    pub fn jump_lte(states: &[DdtState], tol: &Tolerances, h: f64) -> f64 {
        Self::worst_ratio(states, tol, h, |state| {
            let [q_n, q_gamma, q_n1] = state.q;
            let secant_gamma = (q_gamma - q_n) / (Self::GAMMA * h);
            let secant_whole = (q_n1 - q_n) / h;
            h * (secant_whole - secant_gamma).abs() / (1.0 - Self::GAMMA)
        })
    }

    /// The tolerance one state's error is measured against, as ngspice's `CKTterr` takes it: the
    /// larger of `h·(abstol + reltol·max(|i_n|, |i_{n+1}|))` and
    /// `reltol·max(|q_n|, |q_{n+1}|, chgtol)`. The state's own `abstol` replaces the run's.
    fn state_bound(state: &DdtState, tol: &Tolerances, h: f64) -> f64 {
        let abstol = state.abstol.unwrap_or(tol.abstol);
        let by_current = h * (abstol + tol.reltol * state.i[0].abs().max(state.i[1].abs()));
        let by_charge = tol.reltol * state.q[0].abs().max(state.q[2].abs()).max(tol.chgtol);
        by_current.max(by_charge)
    }

    /// The largest `error(state) / state_bound(state)` over the states, `0.0` for none.
    fn worst_ratio(states: &[DdtState], tol: &Tolerances, h: f64, error: impl Fn(&DdtState) -> f64) -> f64 {
        let mut worst = 0.0_f64;
        for state in states {
            let bound = Self::state_bound(state, tol, h);
            if bound > 0.0 {
                worst = worst.max(error(state) / bound);
            }
        }
        worst
    }
}

/// Numerical quadrature over sampled data (noise PSD integration, …).
pub struct Integrator;

impl Integrator {
    /// Trapezoidal integral `Σ ½(x_{i+1} − x_i)·(y_i + y_{i+1})` over
    /// (possibly non-uniform) abscissae. When the slices differ in length the
    /// shorter one bounds the sum; empty/single-point input integrates to 0.
    pub fn trapezoid(xs: &[f64], ys: &[f64]) -> f64 {
        xs.windows(2)
            .zip(ys.windows(2))
            .map(|(x, y)| 0.5 * (x[1] - x[0]) * (y[0] + y[1]))
            .sum()
    }

    /// The integral of a density that is a power law between successive points, ngspice's
    /// quadrature for a noise band (`Nintegrate`): between `(f₁, N₁)` and `(f₂, N₂)` the density
    /// is `N₁·(f/f₁)^a` with `a = ln(N₂/N₁) / ln(f₂/f₁)`, so the interval adds
    /// `N₁·f₁/(a+1)·((f₂/f₁)^(a+1) − 1)`, and `N₁·f₁·ln(f₂/f₁)` when `a = −1`.
    ///
    /// Where a density is not positive its logarithm does not exist, so the interval takes the
    /// plain `N₁·(f₂ − f₁)` (design D8). An interval that does not start at a positive frequency
    /// has no logarithm either and takes the trapezoid. When the slices differ in length the
    /// shorter one bounds the sum, as for [`Integrator::trapezoid`].
    pub fn power_law(frequencies: &[f64], densities: &[f64]) -> f64 {
        frequencies
            .windows(2)
            .zip(densities.windows(2))
            .map(|(f, n)| Self::power_law_interval((f[0], n[0]), (f[1], n[1])))
            .sum()
    }

    /// One interval of [`Integrator::power_law`] between the points `(f₁, N₁)` and `(f₂, N₂)`.
    fn power_law_interval((f1, n1): (f64, f64), (f2, n2): (f64, f64)) -> f64 {
        /// The distance of `a + 1` from 0 under which the exponent counts as `−1`, as ngspice's.
        const LOG_CASE: f64 = 1.0e-10;
        if f2 == f1 {
            return 0.0;
        }
        if f1 <= 0.0 || f2 <= 0.0 {
            return 0.5 * (f2 - f1) * (n1 + n2);
        }
        if n1 <= 0.0 || n2 <= 0.0 {
            return n1 * (f2 - f1);
        }
        let ratio = f2 / f1;
        let exponent_plus_one = (n2 / n1).ln() / ratio.ln() + 1.0;
        if exponent_plus_one.abs() < LOG_CASE {
            n1 * f1 * ratio.ln()
        } else {
            n1 * f1 / exponent_plus_one * (ratio.powf(exponent_plus_one) - 1.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trapezoid_uniform_grid() {
        // y = x² on [0, 2] with unit spacing: ½(0+1) + ½(1+4) = 3.0.
        let xs = [0.0, 1.0, 2.0];
        let ys = [0.0, 1.0, 4.0];
        assert_eq!(Integrator::trapezoid(&xs, &ys), 3.0);
    }

    #[test]
    fn trapezoid_non_uniform_grid() {
        // y = x on [0, 0.5, 2]: ½·0.5·(0+0.5) + ½·1.5·(0.5+2) = 0.125 + 1.875.
        let xs = [0.0, 0.5, 2.0];
        let ys = [0.0, 0.5, 2.0];
        assert_eq!(Integrator::trapezoid(&xs, &ys), 2.0);
    }

    #[test]
    fn trapezoid_degenerate_inputs_integrate_to_zero() {
        assert_eq!(Integrator::trapezoid(&[], &[]), 0.0);
        assert_eq!(Integrator::trapezoid(&[1.0], &[2.0]), 0.0);
        // Length mismatch: the shorter slice bounds the sum.
        assert_eq!(Integrator::trapezoid(&[0.0, 1.0, 2.0], &[1.0, 1.0]), 1.0);
    }

    // ── bdf2_coeffs (the helper behind `TrBdf2::phase_coeffs` BDF2) ───────

    #[test]
    fn bdf2_uniform_collapses_to_canonical_form() {
        let dt = 1e-3_f64;
        let (c0, c1, c2) = bdf2_coeffs(dt, dt);
        // Uniform-step BDF2: c0 = 3/(2·dt), c1 = −2/dt, c2 = 1/(2·dt).
        assert!((c0 - 1500.0).abs() < 1e-9, "c0 = {c0}");
        assert!((c1 + 2000.0).abs() < 1e-9, "c1 = {c1}");
        assert!((c2 - 500.0).abs() < 1e-9, "c2 = {c2}");
    }

    #[test]
    fn bdf2_first_step_falls_back_to_backward_euler() {
        // dt1 = 0 → no history → backward-Euler.
        let (c0, c1, c2) = bdf2_coeffs(1e-3, 0.0);
        assert!((c0 - 1000.0).abs() < 1e-9);
        assert!((c1 + 1000.0).abs() < 1e-9);
        assert_eq!(c2, 0.0);
    }

    #[test]
    fn bdf2_non_uniform_uses_three_point_formula() {
        let dt0 = 1e-3;
        let dt1 = 2e-3;
        let (c0, c1, c2) = bdf2_coeffs(dt0, dt1);
        // Hand-computed from the non-uniform formula with dt0=1e-3, dt1=2e-3.
        let sum = dt0 + dt1;
        assert!((c0 - (2.0 * dt0 + dt1) / (dt0 * sum)).abs() < 1e-9);
        assert!((c1 + sum / (dt0 * dt1)).abs() < 1e-9);
        assert!((c2 - dt0 / (dt1 * sum)).abs() < 1e-9);
    }

    // ── TR-BDF2 ───────────────────────────────────────────────────────────

    /// Tolerances with `chgtol` as given and every other value as the run's default.
    fn with_chgtol(chgtol: f64) -> Tolerances {
        Tolerances { chgtol, ..Tolerances::default() }
    }

    #[test]
    fn the_state_estimate_reads_chgtol() {
        // q = [0, 1e-16, 3e-16] C, no current, abstol 0: Milne's difference is
        // 3e-16 − (1e-16 + 0.7071…·1e-16) = 1.2929e-16 C. With chgtol 1e-14 the bound is
        // reltol·chgtol = 1e-17 C → 12.929; with chgtol 1e-16 it is reltol·3e-16 = 3e-19 C → 430.97.
        let state = DdtState { q: [0.0, 1e-16, 3e-16], i: [0.0, 0.0], abstol: Some(0.0) };
        let slope_scale = (1.0 - TrBdf2::GAMMA) / TrBdf2::GAMMA;
        let error = 3e-16 - (1e-16 + slope_scale * 1e-16);
        let under = TrBdf2::state_lte(&[state], &with_chgtol(1e-14), 1e-9);
        let above = TrBdf2::state_lte(&[state], &with_chgtol(1e-16), 1e-9);
        assert!((under - error / 1e-17).abs() < 1e-6 * under, "chgtol above the charge: {under}");
        assert!((above - error / 3e-19).abs() < 1e-6 * above, "chgtol below the charge: {above}");
        assert!(above > 10.0 * under);
    }

    #[test]
    fn the_state_estimate_is_zero_for_a_linear_charge() {
        // q(t) = 2e-9·t is exact for the linear predictor at t = 0, γh, h.
        let h = 1e-6;
        let q = |fraction: f64| 2e-9 * fraction * h;
        let state = DdtState { q: [q(0.0), q(TrBdf2::GAMMA), q(1.0)], i: [2e-9, 2e-9], abstol: None };
        assert!(TrBdf2::state_lte(&[state], &Tolerances::default(), h) < 1e-9);
        assert_eq!(TrBdf2::state_lte(&[], &Tolerances::default(), h), 0.0, "no state, nothing limits the step");
    }

    #[test]
    fn the_jump_estimate_is_zero_for_a_state_linear_in_time() {
        let h = 1e-6;
        let q = |fraction: f64| 2e-9 * fraction * h;
        let state = DdtState { q: [q(0.0), q(TrBdf2::GAMMA), q(1.0)], i: [2e-9, 2e-9], abstol: None };
        assert!(TrBdf2::jump_lte(&[state], &Tolerances::default(), h) < 1e-9);
    }

    #[test]
    fn the_jump_estimate_of_a_quadratic_state_is_backward_eulers_error() {
        // q = t²: q'' = 2, so backward Euler's error over h is (h²/2)·q'' = h². The bound is
        // reltol·max(|q_n|, |q_{n+1}|, chgtol) = reltol·h² (abstol 0, no current), so the ratio is
        // 1/reltol.
        let h = 1e-3;
        let tol = Tolerances::default();
        let q = |fraction: f64| (fraction * h).powi(2);
        let state = DdtState { q: [q(0.0), q(TrBdf2::GAMMA), q(1.0)], i: [0.0, 0.0], abstol: Some(0.0) };
        let ratio = TrBdf2::jump_lte(&[state], &tol, h);
        let expected = 1.0 / tol.reltol;
        assert!((ratio - expected).abs() < 0.01 * expected, "ratio {ratio}, expected {expected}");
    }

    #[test]
    fn the_jump_estimate_of_no_state_is_zero() {
        assert_eq!(TrBdf2::jump_lte(&[], &Tolerances::default(), 1e-6), 0.0);
    }

    #[test]
    fn the_jump_estimate_is_normalized_like_the_state_estimate() {
        // One state, one error: the two estimates differ only by the error they measure, so the
        // ratio of the results is the ratio of the errors.
        let h = 1e-6;
        let tol = Tolerances::default();
        let state = DdtState { q: [0.0, 1e-10, 4e-10], i: [0.0, 1e-4], abstol: None };
        let milne = (4e-10 - (1e-10 + (1.0 - TrBdf2::GAMMA) / TrBdf2::GAMMA * 1e-10_f64)).abs();
        let secants = h * (4e-10 / h - 1e-10 / (TrBdf2::GAMMA * h)).abs() / (1.0 - TrBdf2::GAMMA);
        let ratio = TrBdf2::jump_lte(&[state], &tol, h) / TrBdf2::state_lte(&[state], &tol, h);
        assert!((ratio - secants / milne).abs() < 1e-9 * ratio, "ratio {ratio}");
    }

    #[test]
    fn a_rise_from_flat_is_measured_and_rejected() {
        // Flat over γh, a rise of 1e-9 C over the rest: the old node estimate skipped it (the
        // asymmetric differences), the state estimate measures 1e-9 / (reltol·1e-9) = 1000.
        let tol = Tolerances::default();
        let state = DdtState { q: [0.0, 0.0, 1e-9], i: [0.0, 1e-3], abstol: None };
        let error = TrBdf2::state_lte(&[state], &tol, 1e-6);
        assert!(error > tol.trtol, "error {error} against trtol {}", tol.trtol);
    }

    #[test]
    fn trbdf2_gamma_is_two_minus_sqrt2() {
        // γ = 2 − √2 ≈ 0.5857864376269049.
        assert!((TrBdf2::GAMMA - (2.0 - 2.0_f64.sqrt())).abs() < 1e-12);
        assert!((TrBdf2::GAMMA - 0.5857864376269049).abs() < 1e-9);
        // Equal-weight stages: γ and (1−γ) both positive.
        const { assert!(TrBdf2::GAMMA > 0.0 && TrBdf2::GAMMA < 1.0); }
        // Sanity: γ + (1−γ) = 1, and 1−γ = √2 − 1.
        assert!(((1.0 - TrBdf2::GAMMA) - (2.0_f64.sqrt() - 1.0)).abs() < 1e-12);
    }

    #[test]
    fn trbdf2_trapezoidal_phase_coeffs_match_formula() {
        // TR phase over γh: i_C = (2/(γh))·Q_{n+γ} − (2/(γh))·Q_n, c2 = 0.
        let h = 1e-3;
        let sub = TrBdf2::GAMMA * h;
        let (c0, c1, c2) = TrBdf2::phase_coeffs(TrBdf2Phase::Trapezoidal, h);
        assert!((c0 - 2.0 / sub).abs() < 1e-6 * (2.0 / sub).abs(), "c0 = {c0}");
        assert!((c1 + 2.0 / sub).abs() < 1e-6 * (2.0 / sub).abs(), "c1 = {c1}");
        assert_eq!(c2, 0.0);
    }

    #[test]
    fn trbdf2_bdf2_phase_coeffs_match_nonuniform_formula() {
        // BDF2 phase over (1−γ)h with previous sub-step γh must equal the
        // existing non-uniform bdf2_coeffs(dt0=(1−γ)h, dt1=γh) — the kernel
        // delegates to that single source of truth.
        let h = 1e-3;
        let dt0 = (1.0 - TrBdf2::GAMMA) * h;
        let dt1 = TrBdf2::GAMMA * h;
        let expected = bdf2_coeffs(dt0, dt1);
        let got = TrBdf2::phase_coeffs(TrBdf2Phase::Bdf2, h);
        assert!((got.0 - expected.0).abs() < 1e-6 * expected.0.abs(), "c0 {} vs {}", got.0, expected.0);
        assert!((got.1 - expected.1).abs() < 1e-6 * expected.1.abs(), "c1 {} vs {}", got.1, expected.1);
        assert!((got.2 - expected.2).abs() < 1e-6 * expected.2.abs(), "c2 {} vs {}", got.2, expected.2);
    }

    #[test]
    fn trbdf2_bdf2_phase_coeffs_hand_computed() {
        // Hand-computed for h = 1e-3, γ = 2−√2 ≈ 0.5857864:
        //   dt0 = (1−γ)h ≈ 4.14214e-4, dt1 = γh ≈ 5.85786e-4, sum = h = 1e-3
        //   c0 = (2·dt0+dt1)/(dt0·sum) ≈ 3414.21
        //   c1 = −sum/(dt0·dt1)        ≈ −4121.32
        //   c2 = dt0/(dt1·sum)         ≈ 707.107
        let h = 1e-3;
        let (c0, c1, c2) = TrBdf2::phase_coeffs(TrBdf2Phase::Bdf2, h);
        let dt0 = (1.0 - TrBdf2::GAMMA) * h;
        let dt1 = TrBdf2::GAMMA * h;
        let sum = dt0 + dt1;
        assert!((c0 - (2.0 * dt0 + dt1) / (dt0 * sum)).abs() < 1e-3);
        assert!((c1 - (-sum / (dt0 * dt1))).abs() < 1e-3);
        assert!((c2 - dt0 / (dt1 * sum)).abs() < 1e-3);
    }

    #[test]
    fn trbdf2_stage_coeffs_degrade_to_backward_euler_on_restart() {
        // Restart convention (prev_h = 0): the TR stage cannot trust a
        // previous derivative, so it must stamp backward Euler over the γh
        // sub-step — (1/(γh), −1/(γh), 0) — NOT the trapezoid's 2/(γh),
        // which doubles the first-step derivative after a discontinuity.
        let h = 1e-3;
        let sub = TrBdf2::GAMMA * h;
        let (c0, c1, c2) = TrBdf2::stage_coeffs(TrBdf2Phase::Trapezoidal, h, 0.0);
        assert!((c0 - 1.0 / sub).abs() < 1e-6 * (1.0 / sub).abs(), "c0 = {c0}");
        assert!((c1 + 1.0 / sub).abs() < 1e-6 * (1.0 / sub).abs(), "c1 = {c1}");
        assert_eq!(c2, 0.0);
        // The BDF2 stage only spans the current step — unaffected by restart.
        assert_eq!(
            TrBdf2::stage_coeffs(TrBdf2Phase::Bdf2, h, 0.0),
            TrBdf2::phase_coeffs(TrBdf2Phase::Bdf2, h)
        );
    }

    #[test]
    fn trbdf2_stage_coeffs_pass_through_with_history() {
        // With real history (prev_h > 0) the TR stage keeps the full
        // trapezoid weights from phase_coeffs.
        let h = 1e-3;
        assert_eq!(
            TrBdf2::stage_coeffs(TrBdf2Phase::Trapezoidal, h, 2e-4),
            TrBdf2::phase_coeffs(TrBdf2Phase::Trapezoidal, h)
        );
    }
}