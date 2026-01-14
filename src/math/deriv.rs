use ndarray::{ArrayD, ArrayView1, ArrayViewD, Axis, Zip, s};
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
            return Some(BdfCoefficients { alpha: T::zero(), history_coeffs: vec![] });
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

pub struct Differentiator<E, T> {
    order: usize,
    _phantom_data: std::marker::PhantomData<(E, T)>,
}

impl<E, T> Differentiator<E, T>
where
    T: DifferentiableIndependentScalar,
    E: DifferentiableDependentScalar<T>,
{
    pub fn new(order: usize) -> Self {
        Self {
            order,
            _phantom_data: std::marker::PhantomData,
        }
    }

    pub fn differentiate(
        &self,
        data: &ArrayViewD<E>,
        times: &ArrayView1<T>,
        time_axis_idx: usize,
    ) -> Option<ArrayD<E>> {
        let time_axis = Axis(time_axis_idx);
        let len = data.len_of(time_axis);

        // Validation: Return None instead of panic
        if len != times.len() || len <= self.order {
            return None;
        }

        let output_len = len - self.order;

        // Prepare Output Shape
        let mut out_shape = data.shape().to_vec();
        out_shape[time_axis_idx] = output_len;

        let mut output = ArrayD::<E>::zeros(out_shape);

        // Main Loop
        for i in 0..output_len {
            let data_idx = i + self.order;

            // 1. Get Time Window
            // Slice: [t_{n-order}, ..., t_{n}]
            // We pass this slice directly to the generator (Zero-Copy)
            let time_window = times.slice(s![data_idx - self.order..=data_idx]);

            // 2. Generate Coefficients
            // We can convert the ArrayView directly to a slice for the function
            let coeffs = BdfCoefficientGenerator::generate(
                self.order,
                time_window.to_vec(), // Safe because we just sliced it contiguously
            )?;

            // 3. Apply
            let mut out_slice = output.index_axis_mut(time_axis, i);

            // Apply History Terms (c1 * v[n-1] + c2 * v[n-2]...)
            // Note: history_coeffs[0] corresponds to n-1 (most recent history)
            for (k, &coeff) in coeffs.history_coeffs.iter().enumerate() {
                // history_idx = n - 1 - k
                let history_idx = (data_idx - 1) - k;

                let data_slice = data.index_axis(time_axis, history_idx);

                Zip::from(&mut out_slice)
                    .and(&data_slice)
                    .for_each(|out_val, &in_val| {
                        *out_val = *out_val + (in_val * coeff);
                    });
            }

            // Apply Current Term (alpha * v[n])
            let current_slice = data.index_axis(time_axis, data_idx);
            let alpha = coeffs.alpha;

            Zip::from(&mut out_slice)
                .and(&current_slice)
                .for_each(|out_val, &in_val| {
                    *out_val = *out_val + (in_val * alpha);
                });
        }

        Some(output)
    }
}
