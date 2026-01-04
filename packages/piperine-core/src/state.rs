use crate::netlist::CircuitReference;
use num_complex::Complex;
use num_traits::Zero;
use std::collections::{HashMap, VecDeque};
use std::ops::{AddAssign, Mul};

pub trait CircuitStateElement: Clone + Zero + AddAssign + Mul<f64, Output = Self> {}

impl CircuitStateElement for f64 {}
impl CircuitStateElement for Complex<f64> {}

#[derive(Debug, Clone)]
pub struct CircuitState<E: CircuitStateElement> {
    values: VecDeque<HashMap<CircuitReference, E>>,
    derivatives: VecDeque<HashMap<CircuitReference, E>>,
    timestamps: VecDeque<f64>,
    has_guess_in_queue: bool,
    order: usize,
}

impl<E: CircuitStateElement> CircuitState<E> {
    pub(crate) fn get_last_vector(&self) -> HashMap<CircuitReference, E> {
        if self.has_guess_in_queue {
            self.values.get(1).unwrap().clone()
        } else {
            self.values.get(0).unwrap().clone()
        }
    }
}

impl<E: CircuitStateElement> CircuitState<E> {
    pub fn new(
        values: HashMap<CircuitReference, E>,
        derivatives: HashMap<CircuitReference, E>,
        order: usize,
    ) -> Self {
        let mut circuit_state = Self {
            values: VecDeque::with_capacity(order + 2),
            timestamps: VecDeque::with_capacity(order + 2),
            derivatives: VecDeque::with_capacity(order + 2),
            has_guess_in_queue: false,
            order,
        };
        circuit_state.push_commited(0.0, values, derivatives);
        circuit_state
    }

    pub fn push_guess(&mut self, timestamp: f64, guess: HashMap<CircuitReference, E>) {
        self.has_guess_in_queue = true;
        let references = guess.keys().cloned().collect::<Vec<_>>();
        self.values.push_front(guess);
        self.derivatives.push_front(HashMap::new());
        self.timestamps.push_front(timestamp);
        references
            .iter()
            .for_each(|reference| self.update_current_derivative(reference));
    }

    pub fn copy_last_value_as_guess(&mut self, timestamp: f64) {
        self.push_guess(timestamp, self.values.get(0).unwrap().clone());
    }

    pub fn push_commited(
        &mut self,
        timestamp: f64,
        values: HashMap<CircuitReference, E>,
        derivatives: HashMap<CircuitReference, E>,
    ) {
        if self.has_guess_in_queue {
            self.rollback_guess();
        }

        self.values.push_front(values);
        self.derivatives.push_front(derivatives);
        self.timestamps.push_front(timestamp);

        self.trim()
    }

    pub fn commit_guess(&mut self) {
        if self.has_guess_in_queue {
            self.has_guess_in_queue = false;
        }

        self.trim()
    }

    pub fn rollback_guess(&mut self) {
        if self.has_guess_in_queue {
            self.has_guess_in_queue = false;
            self.values.pop_front();
            self.derivatives.pop_front();
            self.timestamps.pop_front();
        }
    }

    pub fn get_commited_value(
        &self,
        circuit_reference: &CircuitReference,
        steps_back: usize,
    ) -> Option<E> {
        let index = if self.has_guess_in_queue {
            steps_back + 1
        } else {
            steps_back
        };

        if index >= self.order {
            None
        } else {
            self.values
                .get(index)
                .and_then(|value| value.get(circuit_reference))
                .cloned()
        }
    }

    pub fn get_guess_value(&self, circuit_reference: &CircuitReference) -> Option<E> {
        if self.has_guess_in_queue {
            None
        } else {
            self.values
                .get(0)
                .and_then(|value| value.get(circuit_reference))
                .cloned()
        }
    }

    pub fn derivative_coefficients(&self, circuit_reference: &CircuitReference) -> (f64, E) {
        let history_count = self.values.len();

        // CASE 0: DC / Start
        if history_count < 2 || self.order == 0 {
            return (0.0, E::zero());
        }

        let h_n = self.timestamps[0] - self.timestamps[1];

        // CASE 1: Backward Euler (Order 1)
        // Force BE if we don't have at least TWO distinct previous points in time.
        if self.order == 1 || history_count < 3 {
            let alpha_0 = 1.0 / h_n;
            let alpha_1 = -1.0 / h_n;
            let hist = self.get_commited_value(circuit_reference, 0).unwrap_or(E::zero()) * alpha_1;
            return (alpha_0, hist);
        }

        // CASE 2: Adaptive BDF2
        let h_prev = self.timestamps[1] - self.timestamps[2];

        // Safety check: if time hasn't actually advanced, stay in BE
        if h_prev <= 0.0 {
            let alpha_0 = 1.0 / h_n;
            let alpha_1 = -1.0 / h_n;
            let hist = self.get_commited_value(circuit_reference, 0).unwrap_or(E::zero()) * alpha_1;
            return (alpha_0, hist);
        }

        let r = h_n / h_prev;

        // ... your BDF2 alpha calculations ...
        let alpha_0 = (1.0 + 2.0 * r) / (1.0 + r);
        let alpha_1 = -(1.0 + r);
        let alpha_2 = (r * r) / (1.0 + r);

        let mut hist = E::zero();
        hist += self.get_commited_value(circuit_reference, 0).unwrap_or(E::zero()) * (alpha_1 / h_n);
        hist += self.get_commited_value(circuit_reference, 1).unwrap_or(E::zero()) * (alpha_2 / h_n);

        (alpha_0 / h_n, hist)
    }

    fn update_current_derivative(&mut self, circuit_reference: &CircuitReference) {
        let (alpha, hist) = self.derivative_coefficients(circuit_reference);

        let current_value = self.values.get(0).unwrap().get(circuit_reference).unwrap();
        self.derivatives.get_mut(0).unwrap().insert(
            circuit_reference.clone(),
            current_value.clone() * alpha + hist,
        );
    }

    fn trim(&mut self) {
        while self.values.len() > self.order + 1 {
            self.values.pop_back();
            self.derivatives.pop_back();
            self.timestamps.pop_back();
        }
    }
}
