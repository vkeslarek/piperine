use crate::circuit::netlist::CircuitReference;
use crate::circuit::state::CircuitState;
use crate::devices::Component;
use crate::devices::soa::SoaViolation;
use crate::math::Stamp;
use crate::math::vector::{InitialValue, SymbolicVector1};
use crate::solver::Context;
use ndarray::Array1;
use std::collections::HashMap;

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
    pub values: SymbolicVector1<CircuitReference, f64>,
    pub soa_violations: Vec<SoaViolation>,
}

impl DcAnalysisResult {
    pub fn get_value(&self, reference: &CircuitReference) -> Option<f64> {
        self.values.get(reference).cloned()
    }
}
