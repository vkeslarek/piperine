use crate::circuit::Circuit;
use crate::circuit::netlist::CircuitReference;
use crate::devices::Component;
use crate::math::deriv::BdfCoefficientGenerator;
use crate::math::linear::Stamp;
use crate::math::unit::Time;
use crate::solver::Context;
use ndarray::{Array1, Array2, ArrayView1, ArrayView2, s};
use std::collections::HashMap;

#[derive(Clone)]
pub struct TransientAnalysisOptions {
    pub stop_time: f64,
    pub dt: f64,
}

#[derive(Clone)]
pub struct TransientAnalysisContext {
    pub time: Time,
    pub dt: Time,
}

pub struct TransientCircuitState {
    pub mapping: HashMap<CircuitReference, usize>,
    pub history: Array2<f64>,
    pub timestamps: Array1<f64>,
}

impl TransientCircuitState {
    pub fn new(
        mapping: HashMap<CircuitReference, usize>,
        size: usize,
        history_depth: usize,
    ) -> Self {
        Self {
            mapping,
            history: Array2::zeros((history_depth, size)),
            timestamps: Array1::zeros(history_depth),
        }
    }

    pub fn integration_parameters(&self, order: usize) -> (f64, Array1<f64>) {
        let ts_window = self.timestamps.slice(s![0..=order]);

        let coeffs_opt = BdfCoefficientGenerator::generate(order, ts_window.to_vec());

        if coeffs_opt.is_none() {
            return (0.0, Array1::zeros(self.history.ncols()));
        }
        let coeffs = coeffs_opt.unwrap();

        let mut history_sum = Array1::zeros(self.history.ncols());

        for (i, &c) in coeffs.history_coeffs.iter().enumerate() {
            let history_idx = i + 1;
            if history_idx < self.history.nrows() {
                history_sum.scaled_add(c, &self.history.row(history_idx));
            }
        }

        (coeffs.alpha, history_sum)
    }

    pub fn update_guess(&mut self, new_values: Array1<f64>) {
        self.history.row_mut(0).assign(&new_values);
    }

    pub fn push_timestep(&mut self, next_time: f64) {
        let rows = self.history.nrows();
        let cols = self.history.ncols();

        // 1. Prediction (Linear Extrapolation)
        // (Same logic as before...)
        let prediction = if rows >= 2 && self.timestamps[0] > self.timestamps[1] {
            let dt_new = next_time - self.timestamps[0];
            let dt_old = self.timestamps[0] - self.timestamps[1];

            if dt_old > 1e-12 {
                let slope = (&self.history.row(0) - &self.history.row(1)) / dt_old;
                &self.history.row(0) + &(slope * dt_new)
            } else {
                self.history.row(0).to_owned()
            }
        } else {
            self.history.row(0).to_owned()
        };

        // 2. Shift History (The Fix)
        if rows > 1 {
            // We want to move [Row 0..Row N-1] into [Row 1..Row N]
            // Source Range: Start (0) to End of second-to-last row ((rows-1)*cols)
            // Dest Start: Start of Row 1 (cols)

            if let Some(slice) = self.history.as_slice_mut() {
                // OPTIMIZED: copy_within handles overlapping memory safely
                slice.copy_within(0..((rows - 1) * cols), cols);
            } else {
                // FALLBACK: If array is somehow not contiguous, we loop manually
                for i in (1..rows).rev() {
                    let (mut to, from) = self.history.multi_slice_mut((s![i, ..], s![i - 1, ..]));
                    to.assign(&from);
                }
            }

            // Shift timestamps too
            if let Some(ts_slice) = self.timestamps.as_slice_mut() {
                ts_slice.copy_within(0..(rows - 1), 1);
            }
        }

        // 3. Set New State
        self.timestamps[0] = next_time;
        self.history.row_mut(0).assign(&prediction);
    }

    pub fn get_value(&self, reference: &CircuitReference, lookback: usize) -> Option<f64> {
        let index = self.mapping.get(reference)?;
        if lookback >= self.history.nrows() {
            return None;
        }
        Some(self.history[[lookback, *index]])
    }
}

pub trait TransientAnalysis: Component {
    fn update_transient(
        &mut self,
        circuit_states: &TransientCircuitState,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> crate::result::Result<()> {
        Ok(())
    }

    fn load_transient(
        &self,
        circuit_states: &TransientCircuitState,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>>;

    fn load_dynamic(
        &self,
        circuit_states: &TransientCircuitState,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        vec![]
    }
}

#[derive(Debug)]
pub struct TransientAnalysisResult {
    pub mapping: HashMap<CircuitReference, usize>,
    pub data: Vec<f64>,
    times: Vec<f64>,
}

impl TransientAnalysisResult {
    pub fn new(mapping: HashMap<CircuitReference, usize>) -> Self {
        let num_symbols = mapping.len();
        Self {
            mapping,
            data: Vec::with_capacity(num_symbols * 1024),
            times: Vec::with_capacity(1024),
        }
    }

    pub fn push(&mut self, timestamp: f64, datapoint: ArrayView1<f64>) {
        if datapoint.len() != self.mapping.len() {
            panic!("Data point size mismatch");
        }

        self.times.push(timestamp);
        self.data.extend(datapoint.iter());
    }

    pub fn values(&self) -> ArrayView2<f64> {
        let rows = self.times.len();
        let cols = self.mapping.len();

        ArrayView2::from_shape((rows, cols), &self.data).expect("Data buffer corruption")
    }

    pub fn timestamps(&self) -> ArrayView1<f64> {
        ArrayView1::from(&self.times)
    }

    pub fn get_trace(&self, reference: &CircuitReference) -> Option<Array1<f64>> {
        let col_idx = *self.mapping.get(reference)?;
        Some(self.values().column(col_idx).to_owned())
    }
}

pub trait TransientSolver {
    fn build(
        circuit: Circuit,
        transient_analysis_options: TransientAnalysisOptions,
        context: Context,
    ) -> crate::result::Result<impl TransientSolver>;
    fn solve(&mut self) -> crate::result::Result<TransientAnalysisResult>;
}
