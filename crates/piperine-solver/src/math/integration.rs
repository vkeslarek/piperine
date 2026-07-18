//! Numerical integration for transient analysis: TR-BDF2 is the sole
//! transient scheme (there is no method-selection surface).
//!
//! The companion coefficients `(c0, c1, c2)` a kernel stamps into the
//! reactive form `i_C = c0·Q_n + c1·Q_{n-1} + c2·Q_{n-2}` come from
//! [`TrBdf2::phase_coeffs`] / [`TrBdf2::stage_coeffs`], so the codegen
//! kernel does not need to know the integration formula family. (Breakpoint
//! forcing is an [`Element`][crate::core::element::Element] ABI method,
//! `next_breakpoints`, not a separate trait.)

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

    /// Global local-truncation-error estimate via Milne's device. A linear
    /// extrapolation of `Q_n` and `Q_{n+γ}` to `t_{n+1}` is differenced from
    /// the BDF2 solution `Q_{n+1}`, normalized per component by
    /// `reltol·|Q_{n+1}| + tol`; the worst component is returned.
    ///
    /// Returns `0.0` for collinear/constant charge (linear extrapolation is
    /// exact) and positive for curvature. The linear predictor is O(h²), a
    /// conservative over-estimate of TR-BDF2's true O(h³) LTE — safe for
    /// timestep control; the residual scale is absorbed by the PI gains
    /// (`kp`/`ki`).
    pub fn milne_lte(
        q_n: &[f64],
        q_n_gamma: &[f64],
        q_n1: &[f64],
        reltol: f64,
        tol: f64,
    ) -> f64 {
        let n = q_n1.len().min(q_n.len()).min(q_n_gamma.len());
        let indices: Vec<usize> = (0..n).collect();
        Self::milne_lte_indexed(q_n, q_n_gamma, q_n1, &indices, reltol, tol)
    }

    /// Milne LTE restricted to `indices` — used by the driver to evaluate the
    /// error only over **node-voltage** components. Branch currents are derived
    /// from the node voltages (KCL), so their accuracy follows; including them
    /// in the predictor misbehaves (the `/γ` extrapolation amplifies the
    /// startup jump of a source's branch current, giving a false huge LTE).
    pub fn milne_lte_indexed(
        q_n: &[f64],
        q_n_gamma: &[f64],
        q_n1: &[f64],
        indices: &[usize],
        reltol: f64,
        tol: f64,
    ) -> f64 {
        let slope_scale = (1.0 - Self::GAMMA) / Self::GAMMA;
        let mut worst = 0.0_f64;
        for &i in indices {
            if i >= q_n1.len() || i >= q_n.len() || i >= q_n_gamma.len() {
                continue;
            }
            // Skip nodes whose history spans a discontinuity — e.g. a
            // voltage-source-forced node that jumped at a breakpoint edge.
            // Such a node's predictor residual is the intentional jump, not
            // truncation error; counting it would reject the step the
            // integrator deliberately landed on. A discontinuity shows up as
            // ASYMMETRIC consecutive differences: one side is flat (pre- or
            // post-jump) while the other is large. Smooth curvature has
            // comparable differences on both sides, so it is kept.
            let d1 = (q_n_gamma[i] - q_n[i]).abs();
            let d2 = (q_n1[i] - q_n_gamma[i]).abs();
            if d1.max(d2) > 0.0 && d1.min(d2) < 0.1 * d1.max(d2) {
                continue;
            }
            let q_pred = q_n_gamma[i] + slope_scale * (q_n_gamma[i] - q_n[i]);
            let err = (q_n1[i] - q_pred).abs();
            let scale = reltol * q_n1[i].abs() + tol;
            if scale > 0.0 {
                let normalized = err / scale;
                if normalized > worst {
                    worst = normalized;
                }
            }
        }
        worst
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// Mock element that returns a fixed timestep, used to verify the
    /// stepper loop consults elements for LTE via `Element::suggest_transient_step`.
    #[test]
    fn lte_default_element_returns_none() {
        use crate::analysis::transient::TransientAnalysisState;
        use crate::core::element::{Element, ElementCapabilities};
        use crate::math::circular_array::CircularArrayBuffer2;
        use crate::solver::Context;

        struct PlainElement;
        impl Element for PlainElement {
            fn name(&self) -> &str { "plain" }
            fn capabilities(&self) -> ElementCapabilities { ElementCapabilities::ANALOG }
        }

        let buf = CircularArrayBuffer2::new(1, 2);
        let state = TransientAnalysisState::new(&buf, &[]);
        let ctx = Context::default();

        let el = PlainElement;
        assert!(el.suggest_transient_step(&state, &[1e-6], &ctx).is_none());
    }

    /// Mini element that always returns a fixed dt of 10 μs, confirming the
    /// stepper can consume the suggestion through the trait.
    #[test]
    fn lte_element_override_returns_custom_dt() {
        use crate::analysis::transient::TransientAnalysisState;
        use crate::core::element::{Element, ElementCapabilities};
        use crate::math::circular_array::CircularArrayBuffer2;
        use crate::solver::Context;

        struct FixedLte(f64);
        impl Element for FixedLte {
            fn name(&self) -> &str { "fixed_lte" }
            fn capabilities(&self) -> ElementCapabilities { ElementCapabilities::ANALOG }
            fn suggest_transient_step(
                &self,
                _state: &TransientAnalysisState<'_>,
                _time_history: &[f64],
                _context: &Context,
            ) -> Option<f64> {
                Some(self.0)
            }
        }

        let buf = CircularArrayBuffer2::new(1, 2);
        let state = TransientAnalysisState::new(&buf, &[]);
        let ctx = Context::default();

        let el = FixedLte(10e-6);
        let sug = el.suggest_transient_step(&state, &[1e-6], &ctx);
        assert_eq!(sug, Some(10e-6));
    }

    // ── TR-BDF2 ───────────────────────────────────────────────────────────

    #[test]
    fn trbdf2_gamma_is_two_minus_sqrt2() {
        // γ = 2 − √2 ≈ 0.5857864376269049.
        assert!((TrBdf2::GAMMA - (2.0 - 2.0_f64.sqrt())).abs() < 1e-12);
        assert!((TrBdf2::GAMMA - 0.5857864376269049).abs() < 1e-9);
        // Equal-weight stages: γ and (1−γ) both positive.
        assert!(TrBdf2::GAMMA > 0.0 && TrBdf2::GAMMA < 1.0);
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

    #[test]
    fn milne_lte_is_zero_for_constant_charge() {
        // Constant charge (no dynamics) → linear extrapolation exact → 0.
        let q = [1e-9, 1e-9, 1e-9];
        let e = TrBdf2::milne_lte(&q, &q, &q, 1e-3, 1e-14);
        assert!(e < 1e-15, "constant charge LTE = {e}");
    }

    #[test]
    fn milne_lte_is_zero_for_linear_charge() {
        // Linearly varying charge: Q_n=0, Q_{n+γ}=γ, Q_{n+1}=1 → predictor
        // extrapolates linearly and hits Q_{n+1} exactly → 0.
        let g = TrBdf2::GAMMA;
        let q_n = [0.0_f64, 0.0];
        let q_ng = [g, 2.0 * g];
        let q_n1 = [1.0_f64, 2.0];
        let e = TrBdf2::milne_lte(&q_n, &q_ng, &q_n1, 1e-3, 1e-14);
        assert!(e < 1e-9, "linear charge LTE = {e}");
    }

    #[test]
    fn milne_lte_is_positive_for_curvature() {
        // Quadratic charge: Q_n=0, Q_{n+γ}=γ², Q_{n+1}=1. The linear
        // predictor misses the curvature → positive normalized error.
        let g = TrBdf2::GAMMA;
        let q_n = [0.0_f64];
        let q_ng = [g * g];
        let q_n1 = [1.0_f64];
        let e = TrBdf2::milne_lte(&q_n, &q_ng, &q_n1, 1e-3, 1e-14);
        assert!(e > 0.0, "quadratic charge LTE should be positive, got {e}");
        // Predictor = γ² + ((1−γ)/γ)·(γ²−0) = γ² + γ(1−γ) = γ. Actual = 1.
        // err = |1 − γ|, scale = reltol·1 + chgtol.
        let pred = g * g + ((1.0 - g) / g) * (g * g - 0.0);
        let expected_err = (1.0 - pred).abs() / (1e-3 * 1.0 + 1e-14);
        assert!((e - expected_err).abs() < 1e-9 * expected_err.abs(), "e={e} expected={expected_err}");
    }
}