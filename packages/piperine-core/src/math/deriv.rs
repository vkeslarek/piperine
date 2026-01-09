use std::collections::VecDeque;

pub struct BdfCoefficients {
    pub alpha: f64,
    pub history_coeffs: Vec<f64>,
}

pub struct BdfCoefficientGenerator;

impl BdfCoefficientGenerator {
    /// Generates coefficients based on the current step size and past step sizes.
    /// 'deltas' should contain [h1, h2, ... hn] where:
    /// h1 = t[n] - t[n-1] (current step)
    /// h2 = t[n-1] - t[n-2] (previous step)
    pub fn generate(order: usize, timestamps: Vec<f64>) -> Result<BdfCoefficients, String> {
        if timestamps.is_empty() {
            return Err("At least one time delta is required".into());
        }

        match order {
            0 => {
                // Not enough values to calculate the derivative
                if timestamps.len() < 1 {
                    return Err("BDF2 requires at least two time deltas".into());
                }

                Ok(BdfCoefficients {
                    alpha: 0.0,
                    history_coeffs: Vec::new(),
                })
            }
            1 => {
                // BDF1 / Backward Euler
                // Formula: v' = (v[n] - v[n-1]) / h1

                if timestamps.len() < 2 {
                    return Err("BDF2 requires at least two time deltas".into());
                }

                let h1 = timestamps[0] - timestamps[1];
                let alpha = 1.0 / h1;
                Ok(BdfCoefficients {
                    alpha,
                    history_coeffs: vec![-alpha],
                })
            }
            2 => {
                // BDF2 (Variable Timestep)
                if timestamps.len() < 3 {
                    return Err("BDF2 requires at least two time deltas".into());
                }
                let h1 = timestamps[0] - timestamps[1];
                let h2 = timestamps[1] - timestamps[2];

                // Derived from Lagrange polynomial differentiation
                let alpha = (2.0 * h1 + h2) / (h1 * (h1 + h2));
                let c1 = -(h1 + h2) / (h1 * h2);
                let c2 = h1 / (h2 * (h1 + h2));

                Ok(BdfCoefficients {
                    alpha,
                    history_coeffs: vec![c1, c2],
                })
            }
            3 => {
                // BDF3 (Variable Timestep)
                if timestamps.len() < 4 {
                    return Err("BDF3 requires at least three time deltas".into());
                }
                let h1 = timestamps[0] - timestamps[1];
                let h2 = timestamps[1] - timestamps[2];
                let h3 = timestamps[2] - timestamps[3];

                // Coefficients for BDF3 variable step get algebraically dense.
                // Usually calculated via Newton form or divided differences.
                let alpha = (1.0 / h1) + (1.0 / (h1 + h2)) + (1.0 / (h1 + h2 + h3));

                // Note: Simplified for common use; full Lagrange
                // expansion is typically used in the internal loop.
                let c1 = -((h1 + h2) * (h1 + h2 + h3)) / (h1 * h2 * (h2 + h3));
                let c2 = (h1 * (h1 + h2 + h3)) / (h2 * h3 * (h1 + h2));
                let c3 = -(h1 * (h1 + h2)) / (h3 * (h2 + h3) * (h1 + h2 + h3));

                Ok(BdfCoefficients {
                    alpha,
                    history_coeffs: vec![c1, c2, c3],
                })
            }
            _ => Err(format!("BDF order {} not implemented", order)),
        }
    }
}
