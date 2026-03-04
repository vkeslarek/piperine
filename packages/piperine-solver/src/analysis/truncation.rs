//! Truncation error control for adaptive timestep selection in transient analysis.
//!
//! This module implements Local Truncation Error (LTE) estimation based on ngSpice's
//! methodology. Devices (capacitors, inductors) report their own truncation errors
//! using divided differences of their state history. The transient solver uses these
//! errors to adaptively adjust the timestep for optimal accuracy and performance.
//!
//! # Algorithm Overview
//!
//! 1. **Device-level error estimation**: Each reactive device (C/L) calculates its
//!    local truncation error using divided differences of state history (charge for
//!    capacitors, flux for inductors).
//!
//! 2. **Timestep suggestion**: Each device suggests a maximum timestep based on:
//!    - Truncation tolerance (trtol)
//!    - Charge tolerance (chgtol)
//!    - Integration method and order
//!
//! 3. **Global timestep selection**: The solver takes the minimum of all device
//!    suggestions, subject to constraints (dt_min, dt_max, breakpoints).
//!
//! # References
//!
//! - ngSpice: `src/spicelib/analysis/ckttrunc.c` - Main truncation algorithm
//! - ngSpice: `src/spicelib/analysis/cktterr.c` - Divided differences calculation
//! - ngSpice: `src/spicelib/devices/cap/captrunc.c` - Capacitor implementation
//! - ngSpice: `src/spicelib/devices/ind/indtrunc.c` - Inductor implementation

use crate::math::unit::Second;

/// Integration method used for transient analysis.
///
/// Each method has different truncation error coefficients that determine
/// how local truncation error is estimated. These coefficients are derived
/// from numerical analysis theory and are specific to each method and order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IntegrationMethod {
    /// Trapezoidal rule (order 2, implicit)
    ///
    /// A-stable, second-order accurate method. Good general-purpose choice.
    /// Truncation coefficient: 1/12 for second-order error term
    Trapezoidal,

    /// Gear's method (Backward Differentiation Formula)
    ///
    /// L-stable, variable order (1-6). Better for stiff systems.
    /// Truncation coefficients vary by order:
    /// - Order 1: 1/2
    /// - Order 2: 2/9 ≈ 0.2222...
    /// - Order 3: 3/22 ≈ 0.1364...
    /// - Order 4: 12/125 = 0.096
    /// - Order 5: 10/137 ≈ 0.073
    /// - Order 6: 20/343 ≈ 0.058
    Gear { order: usize },
}

impl IntegrationMethod {
    /// Returns the truncation error coefficient for this integration method.
    ///
    /// The coefficient relates the divided difference of state history to the
    /// local truncation error. For a method of order `n`, the truncation error
    /// is approximately:
    ///
    /// ```text
    /// LTE ≈ coeff × (n+1)th divided difference × h^(n+1)
    /// ```
    ///
    /// where `h` is the timestep.
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
                _ => panic!("Gear method order must be between 1 and 6"),
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
}

/// Trait for devices that contribute to truncation error estimation.
///
/// Reactive devices (capacitors, inductors) implement this trait to report
/// their local truncation error and suggest a maximum timestep for the next
/// transient step.
///
/// # Implementation Notes
///
/// Devices should:
/// 1. Calculate their state quantity (charge for C, flux for L)
/// 2. Use divided differences of state history to estimate LTE
/// 3. Suggest timestep based on tolerance requirements
/// 4. Return `None` if unable to estimate (e.g., insufficient history)
pub trait TruncationError {
    /// Estimates the local truncation error and suggests a maximum timestep.
    ///
    /// # Parameters
    ///
    /// - `method`: The integration method being used
    /// - `trtol`: Truncation tolerance (typical: 7.0)
    /// - `chgtol`: Charge tolerance in Coulombs (typical: 1e-14)
    ///
    /// # Returns
    ///
    /// - `Some(dt)`: Suggested maximum timestep
    /// - `None`: Unable to estimate (e.g., first few steps, no state change)
    fn suggest_timestep(
        &self,
        method: IntegrationMethod,
        trtol: f64,
        chgtol: f64,
    ) -> Option<Second>;
}

/// Trait for devices/sources that provide time breakpoints.
///
/// Sources with time-varying waveforms (Pulse, Step, PWL, etc.) need to ensure
/// the solver takes timesteps that land exactly on critical transition points.
/// This prevents the solver from "stepping over" fast edges.
///
/// # Example
///
/// A pulse source with:
/// - Rise time: 1ns at t=10ns
/// - Fall time: 1ns at t=20ns
///
/// Should provide breakpoints at: [10ns, 11ns, 20ns, 21ns] to ensure the
/// solver captures the transitions with at least 3 points (before, during, after).
pub trait BreakpointProvider {
    /// Returns a list of time breakpoints where the solver must stop.
    ///
    /// Breakpoints are absolute times (not relative to current time).
    /// The solver will ensure that no timestep exceeds a breakpoint.
    ///
    /// # Returns
    ///
    /// Vector of absolute times where solver must land exactly or take
    /// smaller steps to not overshoot.
    fn get_breakpoints(&self, start_time: Second, stop_time: Second) -> Vec<Second>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_integration_method_coefficients() {
        // Test Trapezoidal
        let trap = IntegrationMethod::Trapezoidal;
        assert_eq!(trap.order(), 2);
        assert!((trap.truncation_coefficient() - 1.0 / 12.0).abs() < 1e-10);

        // Test Gear order 2 (most common)
        let gear2 = IntegrationMethod::Gear { order: 2 };
        assert_eq!(gear2.order(), 2);
        assert!((gear2.truncation_coefficient() - 2.0 / 9.0).abs() < 1e-10);

        // Test Gear order 1
        let gear1 = IntegrationMethod::Gear { order: 1 };
        assert_eq!(gear1.order(), 1);
        assert!((gear1.truncation_coefficient() - 0.5).abs() < 1e-10);
    }

    #[test]
    #[should_panic(expected = "Gear method order must be between 1 and 6")]
    fn test_invalid_gear_order() {
        let gear_invalid = IntegrationMethod::Gear { order: 7 };
        gear_invalid.truncation_coefficient(); // Should panic
    }
}
