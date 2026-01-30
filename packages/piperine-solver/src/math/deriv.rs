use crate::math::circular_array::CircularArrayBuffer2;
use ndarray::{Array1, Zip};
use num_complex::Complex;
use num_traits::{One, Zero};
use std::ops::{Add, Div, Mul, Neg, Sub};

pub trait DifferentiableIndependentScalar:
    'static
    + Copy
    + Clone
    + PartialOrd
    + Zero
    + One
    + Neg<Output = Self>
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
{
}

impl DifferentiableIndependentScalar for f64 {}

pub trait DifferentiableDependentScalar<T>:
    'static + Copy + Clone + Zero + Add<Output = Self> + Sub<Output = Self> + Mul<T, Output = Self>
{
}

impl DifferentiableDependentScalar<f64> for f64 {}
impl DifferentiableDependentScalar<f64> for Complex<f64> {}

pub struct BdfCoefficients<T> {
    pub alpha: T,
    pub history_coeffs: Vec<T>,
}

pub struct BdfCoefficientGenerator;

impl BdfCoefficientGenerator {
    pub fn generate<T: DifferentiableIndependentScalar>(
        order: usize,
        timestamps: Vec<T>,
    ) -> Option<BdfCoefficients<T>> {
        if order == 0 {
            return Some(BdfCoefficients {
                alpha: T::zero(),
                history_coeffs: vec![],
            });
        }

        let len = timestamps.len();

        // 1. Basic Validation (Optimized: Fail fast)
        if len < order + 1 {
            return None;
        }

        // We access the end of the slice for the most recent points
        let t_n = timestamps[len - 1];
        let t_n1 = timestamps[len - 2];

        match order {
            0 => Some(BdfCoefficients {
                alpha: T::zero(),
                history_coeffs: Vec::new(),
            }),
            1 => {
                // BDF1 (Backward Euler)
                // v' = (v[n] - v[n-1]) / h1
                let h1 = t_n - t_n1;

                if h1 == T::zero() {
                    return None;
                } // Prevent division by zero

                let alpha = T::one() / h1;
                Some(BdfCoefficients {
                    alpha,
                    history_coeffs: vec![alpha.neg()],
                })
            }
            2 => {
                // BDF2
                if len < 3 {
                    return None;
                }
                let t_n2 = timestamps[len - 3];

                let h1 = t_n - t_n1;
                let h2 = t_n1 - t_n2;

                if h1 == T::zero() || h2 == T::zero() {
                    return None;
                }

                // Precompute common terms to save divisions
                let sum_h = h1 + h2;
                let prod_h1_sum = h1 * sum_h;
                let prod_h1_h2 = h1 * h2;
                let prod_h2_sum = h2 * sum_h;

                let two = T::one() + T::one();

                // alpha = (2*h1 + h2) / (h1 * (h1 + h2))
                let alpha = (h1 * two + h2) / prod_h1_sum;

                // c1 = -(h1 + h2) / (h1 * h2)
                let c1 = sum_h.neg() / prod_h1_h2;

                // c2 = h1 / (h2 * (h1 + h2))
                let c2 = h1 / prod_h2_sum;

                Some(BdfCoefficients {
                    alpha,
                    history_coeffs: vec![c1, c2],
                })
            }
            3 => {
                // BDF3
                if len < 4 {
                    return None;
                }
                let t_n2 = timestamps[len - 3];
                let t_n3 = timestamps[len - 4];

                let h1 = t_n - t_n1;
                let h2 = t_n1 - t_n2;
                let h3 = t_n2 - t_n3;

                // Simple checks
                if h1 == T::zero() || h2 == T::zero() || h3 == T::zero() {
                    return None;
                }

                // Coefficients for Variable Step BDF3
                // These formulas are standard Newton-form derived
                let h1_h2 = h1 + h2;
                let h2_h3 = h2 + h3;
                let h1_h2_h3 = h1 + h2 + h3;

                let one = T::one();

                // alpha = 1/h1 + 1/(h1+h2) + 1/(h1+h2+h3)
                let alpha = (one / h1) + (one / h1_h2) + (one / h1_h2_h3);

                // c1
                let c1 = -((h1_h2 * h1_h2_h3) / (h1 * h2 * h2_h3));

                // c2
                let c2 = (h1 * h1_h2_h3) / (h2 * h3 * h1_h2);

                // c3
                let c3 = -((h1 * h1_h2) / (h3 * h2_h3 * h1_h2_h3));

                Some(BdfCoefficients {
                    alpha,
                    history_coeffs: vec![c1, c2, c3],
                })
            }
            _ => None, // Not implemented
        }
    }
}

pub trait Integrable<E: DifferentiableIndependentScalar> {
    fn integration_parameters(&self, independent_variables: Vec<E>) -> Option<(E, Array1<E>)>;
}
impl<E> Integrable<E> for CircularArrayBuffer2<E>
where
    E: DifferentiableIndependentScalar,
{
    fn integration_parameters(&self, independent_variables: Vec<E>) -> Option<(E, Array1<E>)> {
        let available_buffer_history = self.len().saturating_sub(1);
        let available_time_history = independent_variables.len().saturating_sub(1);

        let order = available_buffer_history.min(available_time_history).min(3);

        if order == 0 {
            return None;
        }

        let mut bdf_timestamps = independent_variables
            .iter()
            .take(order + 1)
            .cloned()
            .collect::<Vec<E>>();

        bdf_timestamps.reverse();

        let coeffs = BdfCoefficientGenerator::generate(order, bdf_timestamps)?;

        let size = self.size();
        let mut history_sum = Array1::<E>::zeros(size);

        for (i, &beta) in coeffs.history_coeffs.iter().enumerate() {
            let lookback = i + 1;

            if let Some(view) = self.view(lookback) {
                Zip::from(&mut history_sum).and(view).for_each(|sum, &val| {
                    *sum = *sum + val * beta;
                });
            } else {
                return None;
            }
        }

        Some((coeffs.alpha, history_sum))
    }
}
