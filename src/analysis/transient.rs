use crate::circuit::Circuit;
use crate::devices::Component;
use crate::math::deriv::BdfCoefficientGenerator;
use crate::math::linear::Stamp;
use crate::math::unit::Time;
use crate::circuit::netlist::CircuitReference;
use crate::solver::Context;
use faer::Col;
use std::collections::{HashMap, VecDeque};

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
    pub mapping: HashMap<CircuitReference, usize>, // Made public for solver access
    pub values: VecDeque<Col<f64>>,
    pub timestamps: VecDeque<f64>,
    pub current_guess: Col<f64>,
    pub current_dt: f64,
    pub size: usize,
    pub num_symbols: usize,
}

impl TransientCircuitState {
    pub fn new(
        mapping: HashMap<CircuitReference, usize>,
        size: usize,
        history_depth: usize,
    ) -> Self {
        let num_symbols = mapping.len(); // Or use size if mapping includes ground

        // Initialize with zeros. For transient, we usually need at least
        // 2 steps of history + current guess = 3
        Self {
            mapping,
            values: VecDeque::from(vec![Col::zeros(size); history_depth]),
            timestamps: VecDeque::from(vec![0.0; history_depth]),
            current_guess: Col::zeros(size),
            current_dt: 0.0,
            size,
            num_symbols,
        }
    }

    // --- Data Access Methods (Crucial for Component Linearization) ---

    /// Get a value (Voltage or Current) for a specific circuit node/branch.
    /// `lookback`: 0 = current iteration guess, 1 = previous time step, etc.
    pub fn get_value(&self, reference: &CircuitReference, lookback: usize) -> Option<f64> {
        let index = self.mapping.get(reference)?;
        let col = self.values.get(lookback)?;
        Some(col[*index])
    }

    /// Helper to get (V_plus - V_minus) easily
    pub fn get_voltage_diff(
        &self,
        positive: &CircuitReference,
        negative: &CircuitReference,
        lookback: usize,
    ) -> f64 {
        let v_p = self.get_value(positive, lookback).unwrap_or(0.0);
        let v_n = self.get_value(negative, lookback).unwrap_or(0.0);
        v_p - v_n
    }

    // --- Convergence Logic ---

    pub fn check_convergence(
        &self,
        new_values: &Col<f64>,
        reltol: f64,
        vntol: f64,
        abstol: f64,
    ) -> bool {
        // We compare the NEW result against the CURRENT guess (values[0])
        // values[0] holds the result of the previous Newton-Raphson iteration.
        let old_values = &self.values[0];

        for i in 0..self.size {
            let old_v = old_values[i];
            let new_v = new_values[i];
            let diff = (new_v - old_v).abs();

            // Distinguish between Voltage (Nodes) and Current (Branches)
            // for appropriate Absolute Tolerance.
            let abs_limit = if self.is_index_branch(i) {
                abstol
            } else {
                vntol
            };

            // SPICE Standard "Hybrid" Check
            let limit = reltol * old_v.abs().max(new_v.abs()) + abs_limit;

            if diff > limit {
                return false;
            }
        }
        true
    }

    /// Helper to determine if an index corresponds to a Branch (Current) or Node (Voltage)
    fn is_index_branch(&self, index: usize) -> bool {
        // This is O(N) but negligible compared to matrix solve.
        // Can be optimized by storing a BitVec if needed.
        self.mapping
            .iter()
            .any(|(k, &v)| v == index && matches!(k, CircuitReference::Branch(_)))
    }

    // --- BDF / Integration Logic (Preserved) ---

    pub fn last_derivative(&self) -> Col<f64> {
        // We use order 1 or 2 based on points available *before* the new step
        let available_points = self.values.len();
        let order = if available_points > 2 { 2 } else { 1 };

        // We pass the timestamps starting from index 0 (which is t_n-1 relative to the push)
        // Note: Logic here assumes timestamps are synchronized with values
        let ts = self
            .timestamps
            .iter()
            .take(order + 1)
            .cloned()
            .collect::<Vec<_>>();

        // Safety check if we have enough points for BDF
        if ts.len() < 2 {
            return Col::zeros(self.size);
        }

        let coeffs = BdfCoefficientGenerator::generate(order, ts).unwrap();

        // v_dot = alpha * v[0] + sum(c_i * v[i+1])
        let mut v_dot = &self.values[0] * coeffs.alpha;
        for i in 0..coeffs.history_coeffs.len() {
            // Check bounds just in case
            if i + 1 < self.values.len() {
                v_dot += &self.values[i + 1] * coeffs.history_coeffs[i];
            }
        }
        v_dot
    }

    pub fn update_guess(&mut self, corrected_v: Col<f64>) {
        // Overwrite the current iteration's value
        if !self.values.is_empty() {
            self.values[0] = corrected_v;
        }
    }

    pub fn deriv_guess(&self) -> (f64, Col<f64>) {
        let order = (self.values.len() - 1).min(2);

        if order < 1 {
            return (0.0, Col::zeros(self.size));
        }

        // timestamps[0] is current time t_n
        let ts = self
            .timestamps
            .iter()
            .take(order + 1)
            .cloned()
            .collect::<Vec<_>>();
        let coeffs = BdfCoefficientGenerator::generate(order, ts).unwrap();

        let mut vec_sum = Col::zeros(self.size);
        // History starts at values[1] (which is t_n-1)
        for i in 0..coeffs.history_coeffs.len() {
            if i + 1 < self.values.len() {
                vec_sum += coeffs.history_coeffs[i] * &self.values[i + 1];
            }
        }

        (coeffs.alpha, vec_sum)
    }

    pub fn push_new_step(&mut self, next_timestamp: f64) {
        // 1. Calculate the prediction BEFORE pushing to the queue
        // We need at least 2 points (t_n-1, t_n-2) to do a meaningful derivative extrapolation
        // or just 1 point (t_n-1) for simple Forward Euler prediction.

        let prediction = if self.values.len() >= 2 {
            // Predict based on previous derivative
            // v_n_guess = v_n-1 + dt * v_dot_n-1
            let prev_val = &self.values[0];
            let dt = next_timestamp - self.timestamps[0];

            // Note: last_derivative calculation needs to be careful about which '0' index it uses.
            // If we haven't pushed yet, values[0] is the solution at t_prev.
            prev_val + (self.last_derivative() * dt)
        } else if !self.values.is_empty() {
            // Fallback: Use previous value as guess (Zero-Order Hold)
            self.values[0].clone()
        } else {
            Col::zeros(self.size)
        };

        // 2. Commit the new time and the predicted guess
        self.timestamps.push_front(next_timestamp);
        self.values.push_front(prediction);

        // Limit history depth (usually order + 2 is enough)
        if self.values.len() > 7 {
            self.values.pop_back();
            self.timestamps.pop_back();
        }
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

    fn check_convergence(
        &self,
        circuit_states: &TransientCircuitState,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> bool {
        true
    }
}

#[derive(Debug)]
pub struct TransientAnalysisResult {
    pub(crate) mapping: HashMap<CircuitReference, usize>,
    pub(crate) values: VecDeque<Col<f64>>,
    pub(crate) timestamps: VecDeque<f64>,
}

impl TransientAnalysisResult {
    pub fn push(&mut self, timestamp: f64, datapoint: Col<f64>) {
        self.values.push_front(datapoint);
        self.timestamps.push_front(timestamp);
    }
}

impl TransientAnalysisResult {
    pub fn new(mapping: HashMap<CircuitReference, usize>) -> Self {
        Self {
            mapping,
            values: VecDeque::new(),
            timestamps: VecDeque::new(),
        }
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
