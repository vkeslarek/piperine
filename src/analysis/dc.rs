use crate::circuit::netlist::CircuitReference;
use crate::circuit::state::CircuitState;
use crate::devices::Component;
use crate::solver::Context;
use ndarray::Array1;
use std::collections::HashMap;
use crate::math::{InitialValue, Stamp};

pub trait DcAnalysis: Component {
    fn update_dc(
        &mut self,
        dc_circuit_state: &CircuitState<f64>,
        context: &Context,
    ) -> crate::result::Result<()> {
        Ok(())
    }

    fn load_dc(
        &self,
        dc_circuit_state: &CircuitState<f64>,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>>;

    fn initial_dc_values(&self, context: &Context) -> Vec<InitialValue<CircuitReference, f64>> {
        Vec::new()
    }
}

#[derive(Debug)]
pub struct DcAnalysisResult {
    pub values: Array1<f64>,
    pub mapping: HashMap<CircuitReference, usize>,
}

impl DcAnalysisResult {
    pub fn get_value(&self, reference: &CircuitReference) -> Option<f64> {
        self.mapping
            .get(reference)
            .and_then(|&idx| self.values.get(idx).cloned())
    }
}
