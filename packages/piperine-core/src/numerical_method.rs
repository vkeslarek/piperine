use crate::circuit::CircuitReference;

pub trait History {
    fn get_value(&self, circuit_reference: &CircuitReference, lookback: usize) -> Option<f64>;
    fn get_derivative(&self, circuit_reference: &CircuitReference, lookback: usize) -> Option<f64>;
    fn get_dt(&self, lookback: usize) -> f64;
    fn get_size(&self) -> usize;
}

pub trait NumericalMethod {
    /// Returns (alpha_0, history_sum) such that:
    /// dx/dt = (alpha_0 * x_n + history_sum) / dt
    fn get_differentiation_coeffs(
        &self,
        history: &dyn History,
        circuit_reference: &CircuitReference,
    ) -> (f64, f64);

    /// Returns coefficients for integration: x_n = x_{n-1} + dt * f(x)
    fn get_integration_coeffs(&self, history: &dyn History) -> f64;
}

pub struct GearMethod(pub usize);

const GEAR_COEFFS: &[&[f64]] = &[
    &[1.0, -1.0],                                                   // Order 1 (BE)
    &[1.5, -2.0, 0.5],                                              // Order 2
    &[11.0 / 6.0, -3.0, 1.5, -1.0 / 3.0],                           // Order 3
    &[25.0 / 12.0, -4.0, 3.0, -4.0 / 3.0, 0.25],                    // Order 4
    &[137.0 / 60.0, -5.0, 5.0, -10.0 / 3.0, 1.25, -0.2],            // Order 5
    &[147.0 / 60.0, -6.0, 7.5, -20.0 / 3.0, 3.75, -1.2, 1.0 / 6.0], // Order 6
];

const GEAR_INV: &[f64] = &[
    1.0,
    2.0 / 3.0,
    6.0 / 11.0,
    12.0 / 25.0,
    60.0 / 137.0,
    60.0 / 147.0,
];

impl NumericalMethod for GearMethod {
    fn get_differentiation_coeffs(
        &self,
        history: &dyn History,
        circuit_reference: &CircuitReference,
    ) -> (f64, f64) {
        let order = self.0.min(history.get_size()).max(1);

        let coeffs = GEAR_COEFFS[order - 1];
        let mut history_sum = 0.0;

        // Sum history: alpha_1*x_{n-1} + alpha_2*x_{n-2} ...
        for i in 1..coeffs.len() {
            history_sum += coeffs[i] * history.get_value(circuit_reference, i).unwrap_or(0.0);
        }

        (coeffs[0], history_sum)
    }

    fn get_integration_coeffs(&self, _history: &dyn History) -> f64 {
        GEAR_INV[self.0.min(6) - 1]
    }
}

pub struct TrapezoidalMethod;

impl NumericalMethod for TrapezoidalMethod {
    fn get_differentiation_coeffs(
        &self,
        history: &dyn History,
        circuit_reference: &CircuitReference,
    ) -> (f64, f64) {
        let x_prev = history.get_value(circuit_reference, 1).unwrap_or(0.0);
        let x_dot_prev = history.get_derivative(circuit_reference, 1).unwrap_or(0.0);

        // derivative = (2 * x_now - 2 * x_prev - dt * x_dot_prev) / dt
        // alpha_0 = 2.0
        // history_sum = -2.0 * x_prev - dt * x_dot_prev
        let dt = history.get_dt(0);
        (2.0, -2.0 * x_prev - dt * x_dot_prev)
    }

    fn get_integration_coeffs(&self, _history: &dyn History) -> f64 {
        0.5 // Standard Trap weighting
    }
}
