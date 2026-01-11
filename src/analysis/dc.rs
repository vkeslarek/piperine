use crate::analysis::InitialValue;
use crate::circuit::Circuit;
use crate::circuit::netlist::CircuitReference;
use crate::devices::Component;
use crate::math::linear::Stamp;
use crate::solver::{AnalysisResult, CircuitState, Context};
use faer::Col;
use ndarray::{Array1, Array2, ArrayView1, ArrayViewMut1, Zip, s};
use std::collections::HashMap;

pub struct DcCircuitState {
    pub mapping: HashMap<CircuitReference, usize>,
    pub values: Array2<f64>,
}

impl DcCircuitState {
    pub fn new(
        mapping: HashMap<CircuitReference, usize>,
        num_symbols: usize,
        history_depth: usize,
    ) -> Self {
        Self {
            mapping,
            values: Array2::zeros((history_depth, num_symbols)),
        }
    }

    pub fn push(&mut self, new_values: ArrayView1<f64>) {
        let rows = self.values.nrows();
        if rows > 1 {
            let (mut older, newer) = self.values.multi_slice_mut((
                s![1..rows, ..],     // Destination
                s![0..rows - 1, ..], // Source
            ));
            older.assign(&newer);
        }

        // Write new values to Row 0 (Current)
        self.values.row_mut(0).assign(&new_values);
    }

    pub fn current_guess_mut(&mut self) -> ArrayViewMut1<f64> {
        if self.values.nrows() == 0 {
            self.values
                .push_row(Array1::zeros(self.mapping.len()).view())
                .unwrap()
        }

        self.values.row_mut(0)
    }

    /// Overwrites the current guess (Row 0) without shifting history.
    /// Used during Newton-Raphson iterations.
    pub fn update_current_guess(&mut self, new_values: ArrayView1<f64>) {
        self.values.row_mut(0).assign(&new_values);
    }

    /// Returns: (Last Guess - New Guess)
    pub fn get_diff(&self, new_guess: ArrayView1<f64>) -> Array1<f64> {
        &self.values.row(0) - &new_guess
    }

    pub fn get_value(&self, reference: &CircuitReference, lookback: usize) -> Option<f64> {
        let index = self.mapping.get(reference)?;
        // Check bounds manually to return Option instead of panic
        if lookback >= self.values.nrows() {
            return None;
        }
        Some(self.values[[lookback, *index]])
    }
}

impl CircuitState for DcCircuitState {
    type NumType = f64;

    fn current_guess_mut(&mut self) -> ArrayViewMut1<Self::NumType> {
        self.values.row_mut(0)
    }

    fn hist_deriv(&self) -> (Self::NumType, ArrayView1<Self::NumType>) {
        // In DC, there is no time derivative (d/dt = 0).
        // alpha = 0, history = vector of zeros.
        (0.0, self.values.row(0)) // Using row(0) as a dummy view of correct size
    }

    fn push(&mut self, new_values: ArrayView1<Self::NumType>) {
        self.push(new_values);
    }
}

pub trait DcAnalysis: Component {
    fn update_dc(
        &mut self,
        dc_circuit_state: &DcCircuitState,
        context: &Context,
    ) -> crate::result::Result<()> {
        Ok(())
    }

    fn load_dc(
        &self,
        dc_circuit_state: &DcCircuitState,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>>;

    fn initial_dc_values(&self, context: &Context) -> Vec<InitialValue> {
        Vec::new()
    }
}

#[derive(Debug)]
pub struct DcAnalysisResult {
    pub values: Array1<f64>,
    pub mapping: HashMap<CircuitReference, usize>,
}

impl AnalysisResult for DcAnalysisResult {
    type NumType = f64;

    fn new() -> Self {
        Self {
            values: Array1::zeros(0),
            mapping: HashMap::new(),
        }
    }

    fn push_converged(
        &mut self,
        mapping: &HashMap<CircuitReference, usize>,
        values: Array1<Self::NumType>,
    ) {
        self.values = values;
        self.mapping = mapping.clone();
    }
}
