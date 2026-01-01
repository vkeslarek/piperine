use crate::circuit::CircuitReference;
use crate::numerical_method::{History, NumericalMethod};
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone)]
pub struct TimeHistory {
    /// A deque of solutions.
    /// Index 0 is the current iteration/guess.
    /// Index 1 is the previous converged time step (t_n).
    /// Index 2 is the time step before that (t_n-1).
    pub values: VecDeque<Vec<f64>>,
    pub timestamps: VecDeque<f64>,
    pub derivatives: VecDeque<Vec<f64>>,
    pub capacity: usize,
}

#[derive(Debug)]
pub struct CircuitStates {
    /// The map stays here once; components use the ID to index into the Buffer.
    pub mapping: HashMap<CircuitReference, usize>,
    pub history: TimeHistory,
}

impl CircuitStates {
    pub fn new(mapping: HashMap<CircuitReference, usize>, history_depth: usize) -> Self {
        let size = mapping.len();
        let mut values = VecDeque::with_capacity(history_depth + 1);
        let mut timestamps = VecDeque::with_capacity(history_depth + 1);
        let mut derivatives = VecDeque::with_capacity(history_depth + 1);

        // Initialize t=0 state with zeros
        values.push_back(vec![0.0; size]);
        derivatives.push_back(vec![0.0; size]);
        timestamps.push_back(0.0);

        Self {
            mapping,
            history: TimeHistory {
                values,
                timestamps,
                derivatives,
                capacity: history_depth,
            },
        }
    }

    pub fn get_value_with_timestamp(
        &self,
        reference: &CircuitReference,
        lookback: usize,
    ) -> Option<(f64, f64)> {
        let col_idx = *self.mapping.get(reference)?;
        let value = self.history.values.get(lookback).map(|vec| vec[col_idx])?;
        let timestamp = self.history.timestamps.get(lookback)?;

        Some((*timestamp, value))
    }

    pub fn push_commited(&mut self, values: Vec<f64>, timestamp: f64) {
        self.history.values.push_back(values);
        self.history.timestamps.push_back(timestamp);
    }

    /// Returns a slice of the last X samples for a specific variable.
    /// Useful for high-order integration (Gear/BDF2)
    pub fn get_history(&self, reference: &CircuitReference, depth: usize) -> Vec<f64> {
        let col_idx = match self.mapping.get(reference) {
            Some(&i) => i,
            None => return vec![],
        };

        self.history
            .values
            .iter()
            .take(depth)
            .map(|v| v[col_idx])
            .collect()
    }

    pub fn prepare_new_iteration(&mut self) {
        if let Some(last_converged) = self.history.values.get(0).cloned() {
            self.history.values.push_front(last_converged);
        }
    }

    fn differentiate_direct(
        &self,
        circuit_reference: &CircuitReference,
        method: &dyn NumericalMethod,
    ) -> f64 {
        let dt = self.get_dt(0);
        if dt <= 0.0 {
            return 0.0;
        }
        let (alpha0, history_sum) = method.get_differentiation_coeffs(self, circuit_reference);
        (alpha0 * self.get_value(circuit_reference, 0).unwrap_or(0.0) + history_sum) / dt
    }

    pub fn commit_step(&mut self, time: f64, method: &dyn NumericalMethod) {
        self.history.timestamps.push_front(time);

        // Calculate the converged derivatives for this step before pruning
        let size = self.history.values[0].len();
        let mut current_derivatives = vec![0.0; size];

        // We use the 'History' implementation to calculate the new dot{x}
        for (reference, index) in &self.mapping {
            current_derivatives[*index] = self.differentiate_direct(reference, method);
        }
        self.history.derivatives.push_front(current_derivatives);

        // Prune history to capacity
        if self.history.values.len() > self.history.capacity {
            self.history.values.pop_back();
            self.history.timestamps.pop_back();
            self.history.derivatives.pop_back();
        }
    }
}

impl History for CircuitStates {
    fn get_value(&self, reference: &CircuitReference, lookback: usize) -> Option<f64> {
        let idx = self.mapping.get(reference)?;
        self.history.values.get(lookback)?.get(*idx).copied()
    }

    fn get_derivative(&self, reference: &CircuitReference, lookback: usize) -> Option<f64> {
        // Safe access to mapping to prevent panics during iteration
        let idx = self.mapping.get(reference)?;
        self.history.derivatives.get(lookback)?.get(*idx).copied()
    }

    fn get_dt(&self, lookback: usize) -> f64 {
        let t_now = self
            .history
            .timestamps
            .get(lookback)
            .copied()
            .unwrap_or(0.0);
        let t_prev = self
            .history
            .timestamps
            .get(lookback + 1)
            .copied()
            .unwrap_or(0.0);

        // Ensure we don't return 0 to avoid DivisionByZero in components
        let dt = t_now - t_prev;
        if dt <= 0.0 { 1e-15 } else { dt }
    }

    fn get_size(&self) -> usize {
        self.history.values.len().saturating_sub(1)
    }
}
