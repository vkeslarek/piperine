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
    is_branch: Array1<bool>,
}

impl DcCircuitState {
    pub fn new(
        mapping: HashMap<CircuitReference, usize>,
        num_symbols: usize,
        history_depth: usize,
    ) -> Self {
        // 1. Pre-calculate the Branch Mask (O(N) once, instead of O(N^2) every check)
        // We create a boolean array where index 'i' is true if mapping[i] is a Branch.
        let mut is_branch = Array1::from_elem(num_symbols, false);

        for (reference, &index) in &mapping {
            if let CircuitReference::Branch(_) = reference {
                if index < num_symbols {
                    is_branch[index] = true;
                }
            }
        }

        Self {
            mapping,
            // Initialize history with zeros
            values: Array2::zeros((history_depth, num_symbols)),
            is_branch,
        }
    }

    /// Pushes a new solution vector into history.
    /// This acts like a sliding window: [t_0, t_-1] -> [new, t_0]
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

    /// Vectorized SPICE convergence check.
    /// Returns true if ALL nodes/branches are within tolerance.
    pub fn check_convergence(
        &self,
        new_values: ArrayView1<f64>,
        reltol: f64,
        vntol: f64,
        abstol: f64,
    ) -> bool {
        // Get the previous iteration's guess (Row 0)
        let old_values = self.values.row(1);

        // We use Zip to iterate through 3 arrays simultaneously:
        // 1. Old Values
        // 2. New Values
        // 3. Is_Branch mask
        // This usually compiles down to very efficient SIMD instructions.
        Zip::from(&old_values)
            .and(&new_values)
            .and(&self.is_branch)
            .all(|&old_v, &new_v, &is_branch| {
                let diff = (new_v - old_v).abs();

                // Select absolute tolerance based on component type
                let abs_limit = if is_branch { abstol } else { vntol };

                // SPICE Formula: |new - old| < RELTOL * max(|new|, |old|) + ABSTOL
                let limit = reltol * old_v.abs().max(new_v.abs()) + abs_limit;

                diff <= limit
            })
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
    pub values: Col<f64>,
    pub mapping: HashMap<CircuitReference, usize>,
}

impl AnalysisResult for DcAnalysisResult {
    type NumType = f64;

    fn new() -> Self {
        Self {
            values: faer::Col::zeros(0),
            mapping: HashMap::new(),
        }
    }

    fn push_converged(
        &mut self,
        mapping: &HashMap<CircuitReference, usize>,
        values: ArrayView1<Self::NumType>,
    ) {
        // For a simple DC analysis, we just store the final result.
        // We use our FaerToNdarray logic (the inverse) or manually build the Col.
        self.values = faer::Col::from_fn(values.len(), |i| values[i]);
        self.mapping = mapping.clone();
    }
}

pub trait DcSolver {
    fn build(circuit: Circuit, context: Context) -> crate::result::Result<impl DcSolver>;
    fn solve(&mut self) -> crate::result::Result<DcAnalysisResult>;
}
