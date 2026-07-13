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
//! The [`TruncationError`] and [`BreakpointProvider`] traits live here too —
//!    they describe numerical-integration concerns (LTE-driven timestep and
//!    integration-error forcing breakpoints) rather than analysis structure.
//!    A future stepper strategy will consume them.

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

/// Trait for devices/sources that provide time breakpoints.
///
/// Sources with time-varying waveforms (Pulse, Step, PWL, etc.) need the
/// solver to land exactly on critical transition points so it does not step
/// over fast edges.
///
/// # Example
///
/// A pulse source with rise time `1ns` at `t=10ns` and fall time `1ns` at
/// `t=20ns` should provide breakpoints at `[10ns, 11ns, 20ns, 21ns]` so
/// the integrator captures the transitions with at least three points
/// (before, during, after).
pub trait BreakpointProvider {
    /// Absolute times (not relative to current time) where the solver must
    /// land exactly or take smaller steps to avoid overshooting.
    fn get_breakpoints(&self, start_time: Second, stop_time: Second) -> Vec<Second>;
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
}