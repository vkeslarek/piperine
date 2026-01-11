use crate::circuit::netlist::CircuitReference;
use crate::circuit::state::CircuitState;
use crate::devices::Component;
use crate::math::linear::{InitialValue, Stamp};
use crate::math::unit::Time;
use crate::solver::Context;
use ndarray::{Array1, ArrayView1, ArrayView2};
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

pub trait TransientAnalysis: Component {
    fn update_transient(
        &mut self,
        circuit_states: &CircuitState<f64>,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> crate::result::Result<()> {
        Ok(())
    }

    fn load_transient(
        &self,
        circuit_states: &CircuitState<f64>,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>>;

    fn load_transient_dynamic(
        &self,
        circuit_states: &CircuitState<f64>,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        vec![]
    }

    fn initial_transient_values(
        &self,
        context: &Context,
    ) -> Vec<InitialValue<CircuitReference, f64>> {
        Vec::new()
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
