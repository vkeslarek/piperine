use crate::circuit::Circuit;
use crate::devices::Component;
use crate::math::deriv::BdfCoefficientGenerator;
use crate::math::linear::Stamp;
use crate::math::unit::Time;
use crate::netlist::CircuitReference;
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
    mapping: HashMap<CircuitReference, usize>,
    values: VecDeque<Col<f64>>,
    timestamps: VecDeque<f64>,
    current_guess: Col<f64>,
    current_dt: f64,
    size: usize,
    num_symbols: usize,
}

impl TransientCircuitState {
    pub fn last_derivative(&self) -> Col<f64> {
        // We use order 1 or 2 based on points available *before* the new step
        let order = (self.values.len() - 1).min(2);

        // We pass the timestamps starting from index 0 (which is t_n-1)
        let ts = self.timestamps.iter().cloned().collect::<Vec<_>>();
        let coeffs = BdfCoefficientGenerator::generate(order, ts).unwrap();

        // v_dot = alpha * v[0] + sum(c_i * v[i+1])
        let mut v_dot = &self.values[0] * coeffs.alpha;
        for i in 0..coeffs.history_coeffs.len() {
            v_dot += &self.values[i + 1] * coeffs.history_coeffs[i];
        }
        v_dot
    }

    pub fn update_guess(&mut self, corrected_v: Col<f64>) {
        self.values[0] = corrected_v;
    }

    pub fn deriv_guess(&self) -> (f64, Col<f64>) {
        let order = (self.values.len() - 1).min(2);

        // timestamps[0] is already the current time t_n
        let ts = self.timestamps.iter().cloned().collect::<Vec<_>>();
        let coeffs = BdfCoefficientGenerator::generate(order, ts).unwrap();

        let mut vec_sum = Col::zeros(self.num_symbols);
        // History starts at values[1] (which is t_n-1)
        for i in 0..coeffs.history_coeffs.len() {
            vec_sum += coeffs.history_coeffs[i] * &self.values[i + 1];
        }

        (coeffs.alpha, vec_sum)
    }

    pub fn push_new_step(&mut self, next_timestamp: f64) {
        // 1. Calculate the prediction BEFORE pushing to the queue
        let mut prediction = self.values[0].clone();
        if self.values.len() >= 1 {
            let dt = next_timestamp - self.timestamps[0];
            // Linear Prediction: v_n ≈ v_n-1 + dt * v_dot_n-1
            prediction += dt * self.last_derivative();
        }

        // 2. Commit the new time and the predicted guess
        self.timestamps.push_front(next_timestamp);
        self.values.push_front(prediction);

        if self.values.len() > 4 {
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
    ) -> crate::error::Result<()> {
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

pub struct TransientAnalysisResult {}

pub trait TransientSolver {
    fn solve(
        circuit: Circuit,
        transient_analysis_options: TransientAnalysisOptions,
        context: Context,
    ) -> crate::error::Result<TransientAnalysisResult>;
}
