use crate::analysis::InitialValue;
use crate::analysis::transient::TransientAnalysis;
use crate::circuit::netlist::{CircuitReference, IndependentVariable};
use crate::math::deriv::BdfCoefficientGenerator;
use crate::math::num::{Field, ScalableByReal};
use ndarray::{Array1, Array2, ArrayView1, s};
use num_traits::Saturating;
use std::collections::HashMap;

pub struct CircuitState<E: Field> {
    dependent_mapping: HashMap<CircuitReference, usize>,
    dependent_data: Array2<E>,
    independent_mapping: HashMap<IndependentVariable, usize>,
    independent_data: Array2<f64>,
    current_index: usize,
    available_datapoints: usize,
    history_depth: usize,
}

impl<E: Field> CircuitState<E> {
    pub fn new(
        dependent_mapping: HashMap<CircuitReference, usize>,
        independent_mapping: HashMap<IndependentVariable, usize>,
        history_depth: usize,
    ) -> Self {
        let num_dependent_symbols = dependent_mapping.len();
        let num_independent_symbols = independent_mapping.len();

        Self {
            dependent_mapping,
            dependent_data: Array2::zeros((history_depth, num_dependent_symbols)),
            independent_mapping,
            independent_data: Array2::zeros((history_depth, num_independent_symbols)),
            current_index: 0,
            available_datapoints: 0,
            history_depth,
        }
    }

    pub fn apply_initial_conditions(&mut self, initial_values: Vec<InitialValue<E>>) {
        for init in initial_values {
            if let Some(idx) = self.dependent_mapping.get(&init.reference) {
                let phys_idx = self.get_physical_index(0);
                self.dependent_data[[phys_idx, *idx]] = init.value;
            }
        }
    }

    pub fn commit(&mut self) {
        self.current_index = (self.current_index + 1) % self.history_depth;
        self.available_datapoints = (self.available_datapoints + 1).min(self.history_depth);
    }

    pub fn rollback(&mut self) {
        self.current_index = (self.current_index + self.history_depth - 1) % self.history_depth;
        self.available_datapoints = self.available_datapoints.saturating_sub(1);
    }

    pub fn get_dependent_value(&self, reference: &CircuitReference, lookback: usize) -> Option<E> {
        let idx = *self.dependent_mapping.get(reference)?;
        let phys_idx = self.get_physical_index(lookback);
        Some(self.dependent_data[[phys_idx, idx]])
    }

    pub fn get_independent_value(
        &self,
        variable: &IndependentVariable,
        lookback: usize,
    ) -> Option<f64> {
        let idx = *self.independent_mapping.get(variable)?;
        let phys_idx = self.get_physical_index(lookback);
        Some(self.independent_data[[phys_idx, idx]])
    }

    pub fn get_dependent_column(&self, lookback: usize) -> ArrayView1<E> {
        let phys_idx = self.get_physical_index(lookback);
        self.dependent_data.row(phys_idx)
    }

    pub fn get_independent_column(&self, lookback: usize) -> ndarray::ArrayView1<f64> {
        let phys_idx = self.get_physical_index(lookback);
        self.independent_data.row(phys_idx)
    }

    pub fn get_current_dependent_column(&mut self) -> ndarray::ArrayViewMut1<E> {
        let phys_idx = self.get_physical_index(0);
        self.dependent_data.row_mut(phys_idx)
    }

    pub fn get_current_independent_column(&mut self) -> ndarray::ArrayViewMut1<f64> {
        let phys_idx = self.get_physical_index(0);
        self.independent_data.row_mut(phys_idx)
    }

    pub fn get_available_datapoints(&self) -> usize {
        self.available_datapoints
    }

    fn get_physical_index(&self, lookback: usize) -> usize {
        (self.current_index + self.history_depth - lookback) % self.history_depth
    }
}

impl<E: Field + ScalableByReal> CircuitState<E> {
    pub fn prepare_next(&mut self, independent_variables: &ArrayView1<f64>) {
        let phys_idx = self.get_physical_index(0);
        self.independent_data
            .row_mut(phys_idx)
            .assign(&independent_variables);

        let prev_phys_idx = self.get_physical_index(1);
        let prev_values = self.dependent_data.row(prev_phys_idx).to_owned();
        self.dependent_data.row_mut(phys_idx).assign(&prev_values);
    }

    pub fn integration_parameters(&self, dx: IndependentVariable) -> Option<(f64, Array1<E>)> {
        let order = self.available_datapoints.saturating_sub(1).min(3);

        let dx_idx = *self.independent_mapping.get(&dx)?;

        let mut ts_vec = Vec::with_capacity(order + 1);
        for i in (0..=order).rev() {
            let phys_idx = self.get_physical_index(i);
            let val = self.independent_data[[phys_idx, dx_idx]];
            ts_vec.push(val);
        }

        let coeffs = BdfCoefficientGenerator::generate(order, ts_vec)?;

        let num_cols = self.dependent_data.ncols();
        let mut history_sum = Array1::<E>::zeros(num_cols);

        for col in 0..num_cols {
            let mut sum = E::zero();
            let column_view = self.dependent_data.column(col);

            for (i, &c) in coeffs.history_coeffs.iter().enumerate() {
                let lookback = i + 1;
                let phys_idx = self.get_physical_index(lookback);

                sum += column_view[phys_idx] * c;
            }
            history_sum[col] = sum;
        }

        Some((coeffs.alpha, history_sum))
    }
}
