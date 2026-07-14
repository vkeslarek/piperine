//! Numerical integration for transient analysis.
//!
//! Two concerns live here:
//!
//! 1. The [`IntegrationMethod`] enum — Trapezoidal, Gear/BDF (orders 1..=6),
//!    plus their LTE coefficients.
//! 2. The companion coefficients `(c0, c1, c2)` a kernel stamps into the
//!    reactive form `i_C = c0·Q_n + c1·Q_{n-1} + c2·Q_{n-2}`. One method,
//!    `coeffs`, returns them for every supported method, so the codegen
//!    kernel does not need to know which family of integration formula is in
//!    use.
//!
//! The [`TruncationError`] trait lives here too — it describes a
//!    numerical-integration concern (per-element LTE-driven timestep
//!    suggestion) rather than analysis structure. A stepper strategy
//!    consumes it. (Breakpoint forcing is now an [`Element`][crate::core::element::Element]
//!    ABI method, `next_breakpoints`, not a separate trait.)

use crate::analysis::transient::TransientAnalysisState;
use crate::math::unit::Second;
use crate::solver::Context;

/// Numerical integration method for the reactive companion in transient
/// analysis. Each variant exposes its companion coefficients `(c0, c1, c2)`
/// via [`coeffs`] and its LTE coefficient via [`truncation_coefficient`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IntegrationMethod {
    /// Trapezoidal rule (order 2, implicit, A-stable).
    ///
    /// `i_C = (2/dt)·Q_n − (2/dt)·Q_{n-1}`.
    ///
    /// Good general-purpose choice; shows numerical ringing on stiff/LC
    /// circuits. LTE coefficient: `1/12`.
    Trapezoidal,

    /// Gear's method (Backward Differentiation Formula).
    ///
    /// L-stable, variable order (1..=6). Better for stiff systems; damps the
    /// ringing trapezoidal shows. LTE coefficients vary by order:
    /// - Order 1: `1/2`
    /// - Order 2: `2/9 ≈ 0.2222…`
    /// - Order 3: `3/22 ≈ 0.1364…`
    /// - Order 4: `12/125 = 0.096`
    /// - Order 5: `10/137 ≈ 0.073`
    /// - Order 6: `20/343 ≈ 0.058`
    Gear { order: usize },
}

impl IntegrationMethod {
    /// Returns the LTE coefficient for this integration method.
    ///
    /// Relates the divided difference of state history to the local
    /// truncation error:
    /// `LTE ≈ coeff × (n+1)th divided difference × h^(n+1)`.
    pub fn truncation_coefficient(&self) -> f64 {
        match self {
            IntegrationMethod::Trapezoidal => 1.0 / 12.0,
            IntegrationMethod::Gear { order } => match order {
                1 => 0.5,
                2 => 2.0 / 9.0,
                3 => 3.0 / 22.0,
                4 => 12.0 / 125.0,
                5 => 10.0 / 137.0,
                6 => 20.0 / 343.0,
                // Out-of-range orders clamp to the highest supported order (6)
                // rather than panicking, per the "no panic on user input"
                // convention in AGENTS.md. The deviation from a true higher-order
                // Gear coefficient is bounded; callers should treat the result
                // as a conservative truncation estimate for an unsupported order.
                _ => 20.0 / 343.0,
            },
        }
    }

    /// Returns the order of the integration method.
    pub fn order(&self) -> usize {
        match self {
            IntegrationMethod::Trapezoidal => 2,
            IntegrationMethod::Gear { order } => *order,
        }
    }

    /// Companion coefficients `(c0, c1, c2)` for
    /// `i_C = c0·Q_n + c1·Q_{n-1} + c2·Q_{n-2}` at the current timepoint.
    ///
    /// - `dt` is the current step size (`t_n − t_{n-1}`).
    /// - `dt_prev` is the previous step size (`t_{n-1} − t_{n-2}`); `0.0`
    ///   on the first step (the formula falls back to the uniform-step form).
    /// - `effective_order` overrides the method's nominal order on the first
    ///   step (where BDF-2 lacks `t_{n-2}` and must drop to backward-Euler).
    ///
    /// Trapezoidal ignores order and history depth — its formula is two-point.
    /// Gear uses BDF2-style coefficients for order ≥ 2; order 1 is
    /// backward-Euler.
    pub fn coeffs(
        self,
        dt: f64,
        dt_prev: f64,
        effective_order: usize,
    ) -> (f64, f64, f64) {
        match self {
            IntegrationMethod::Trapezoidal => {
                // i_C = (2/dt)·Q_n − (2/dt)·Q_{n-1}; c2 = 0.
                (2.0 / dt, -2.0 / dt, 0.0)
            }
            IntegrationMethod::Gear { order } => {
                let order = effective_order.min(order).max(1);
                match order {
                    1 => (1.0 / dt, -1.0 / dt, 0.0),
                    _ => bdf2_coeffs(dt, dt_prev),
                }
            }
        }
    }
}

/// BDF2 (non-uniform) coefficients for `dQ/dt ≈ c0·Q_n + c1·Q_{n-1} + c2·Q_{n-2}`.
///
/// `dt0 = t_n − t_{n-1}` is the current step; `dt1 = t_{n-1} − t_{n-2}` is
/// the previous step. With `dt1 = 0` (first step), the formula collapses to
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

    /// Global local-truncation-error estimate via Milne's device. A linear
    /// extrapolation of `Q_n` and `Q_{n+γ}` to `t_{n+1}` is differenced from
    /// the BDF2 solution `Q_{n+1}`, normalized per component by
    /// `reltol·|Q_{n+1}| + chgtol`; the worst component is returned.
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
        chgtol: f64,
    ) -> f64 {
        // Linear extrapolation slope from [t_n, t_{n+γ}] extended to t_{n+1}:
        // Q_pred = Q_{n+γ} + ((1−γ)/γ)·(Q_{n+γ} − Q_n).
        let slope_scale = (1.0 - Self::GAMMA) / Self::GAMMA;
        let mut worst = 0.0_f64;
        let n = q_n1.len().min(q_n.len()).min(q_n_gamma.len());
        for i in 0..n {
            let q_pred = q_n_gamma[i] + slope_scale * (q_n_gamma[i] - q_n[i]);
            let err = (q_n1[i] - q_pred).abs();
            let scale = reltol * q_n1[i].abs() + chgtol;
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

/// Trait for devices that contribute to truncation error estimation.
///
/// Reactive devices (capacitors, inductors) implement this trait to report
/// their local truncation error and suggest a maximum timestep for the next
/// transient step. The transient stepper consumes it in Phase 4.
pub trait TruncationError {
    /// Estimate the local truncation error and suggest a maximum timestep.
    ///
    /// - `state_history`: historical circuit states (voltages/currents).
    /// - `time_history`: historical timesteps corresponding to states.
    /// - `method`: the integration method in use.
    /// - `context`: solver context (tolerances: `trtol`, `chgtol`, `abstol`,
    ///   `reltol`).
    ///
    /// Returns `Some(dt)` when an estimate is available, `None` otherwise
    /// (first few steps, no state change).
    fn suggest_timestep(
        &self,
        state_history: &TransientAnalysisState<'_>,
        time_history: &[f64],
        method: IntegrationMethod,
        context: &Context,
    ) -> Option<Second>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_integration_method_coefficients() {
        let trap = IntegrationMethod::Trapezoidal;
        assert_eq!(trap.order(), 2);
        assert!((trap.truncation_coefficient() - 1.0 / 12.0).abs() < 1e-10);

        let gear2 = IntegrationMethod::Gear { order: 2 };
        assert_eq!(gear2.order(), 2);
        assert!((gear2.truncation_coefficient() - 2.0 / 9.0).abs() < 1e-10);

        let gear1 = IntegrationMethod::Gear { order: 1 };
        assert_eq!(gear1.order(), 1);
        assert!((gear1.truncation_coefficient() - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_invalid_gear_order_clamps() {
        let gear_invalid = IntegrationMethod::Gear { order: 7 };
        let coeff = gear_invalid.truncation_coefficient();
        assert!(coeff.is_finite() && coeff > 0.0);
    }

    // A.12 — AGENTS.md says "never panic on user input". Out-of-range Gear
    // orders must not panic. We clamp to the highest supported order (6)
    // which keeps the API total and the truncation coefficient within the
    // Gear family's well-conditioned range.
    #[test]
    fn out_of_range_gear_order_does_not_panic_a12() {
        let gear_too_high = IntegrationMethod::Gear { order: 7 };
        let coeff = gear_too_high.truncation_coefficient();
        assert!(coeff.is_finite(), "coefficient must be finite, got {coeff}");
        assert!(coeff > 0.0, "coefficient must be positive, got {coeff}");

        let gear_zero = IntegrationMethod::Gear { order: 0 };
        let coeff0 = gear_zero.truncation_coefficient();
        assert!(coeff0.is_finite());
        assert!(coeff0 > 0.0);

        let gear_huge = IntegrationMethod::Gear { order: usize::MAX };
        let coeff_max = gear_huge.truncation_coefficient();
        assert!(coeff_max.is_finite());
        assert!(coeff_max > 0.0);
    }

    #[test]
    fn trapezoidal_coeffs_match_formula() {
        let (c0, c1, c2) = IntegrationMethod::Trapezoidal.coeffs(1e-3, 0.0, 1);
        assert!((c0 - 2000.0).abs() < 1e-9, "c0 = {c0}");
        assert!((c1 + 2000.0).abs() < 1e-9, "c1 = {c1}");
        assert_eq!(c2, 0.0);
        // Order and history depth are ignored.
        let (a, b, c) = IntegrationMethod::Trapezoidal.coeffs(1e-6, 5e-6, 3);
        assert_eq!((a, b, c), IntegrationMethod::Trapezoidal.coeffs(1e-6, 0.0, 1));
    }

    #[test]
    fn gear1_is_backward_euler() {
        let (c0, c1, c2) = IntegrationMethod::Gear { order: 1 }.coeffs(1e-3, 0.0, 1);
        assert!((c0 - 1000.0).abs() < 1e-9);
        assert!((c1 + 1000.0).abs() < 1e-9);
        assert_eq!(c2, 0.0);
    }

    #[test]
    fn gear2_uniform_collapses_to_canonical_form() {
        let dt = 1e-3_f64;
        let (c0, c1, c2) = IntegrationMethod::Gear { order: 2 }.coeffs(dt, dt, 2);
        // Uniform-step BDF2: c0 = 3/(2·dt), c1 = −2/dt, c2 = 1/(2·dt).
        assert!((c0 - 1500.0).abs() < 1e-9, "c0 = {c0}");
        assert!((c1 + 2000.0).abs() < 1e-9, "c1 = {c1}");
        assert!((c2 - 500.0).abs() < 1e-9, "c2 = {c2}");
    }

    #[test]
    fn gear2_first_step_falls_back_to_backward_euler() {
        // dt_prev = 0 → no history → backward-Euler, regardless of nominal order.
        let (c0, c1, c2) = IntegrationMethod::Gear { order: 2 }.coeffs(1e-3, 0.0, 1);
        assert!((c0 - 1000.0).abs() < 1e-9);
        assert!((c1 + 1000.0).abs() < 1e-9);
        assert_eq!(c2, 0.0);
    }

    #[test]
    fn gear2_non_uniform_uses_three_point_formula() {
        let dt0 = 1e-3;
        let dt1 = 2e-3;
        let (c0, c1, c2) = IntegrationMethod::Gear { order: 2 }.coeffs(dt0, dt1, 2);
        // Hand-computed from bdf2_coeffs with dt0=1e-3, dt1=2e-3.
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
        let method = IntegrationMethod::Gear { order: 2 };

        let el = PlainElement;
        assert!(el.suggest_transient_step(&state, &[1e-6], method, &ctx).is_none());
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
                _method: IntegrationMethod,
                _context: &Context,
            ) -> Option<f64> {
                Some(self.0)
            }
        }

        let buf = CircularArrayBuffer2::new(1, 2);
        let state = TransientAnalysisState::new(&buf, &[]);
        let ctx = Context::default();
        let method = IntegrationMethod::Trapezoidal;

        let el = FixedLte(10e-6);
        let sug = el.suggest_transient_step(&state, &[1e-6], method, &ctx);
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