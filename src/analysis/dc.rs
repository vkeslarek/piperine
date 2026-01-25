use crate::circuit::netlist::{CircuitReference, CircuitVariable};
use crate::devices::Component;
use crate::devices::soa::SoaViolation;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::iv::InitialValue;
use crate::math::linear::Stamp2;
use crate::solver::Context;
use std::collections::HashMap;
use std::sync::Arc;

pub type DcAnalysisState = CircularArrayBuffer2<f64>;

pub trait DcAnalysis: Component {
    fn update_dc(
        &mut self,
        _dc_circuit_state: &DcAnalysisState,
        _context: &Context,
    ) -> crate::result::Result<()> {
        Ok(())
    }

    fn load_dc(
        &self,
        dc_circuit_state: &DcAnalysisState,
        context: &Context,
    ) -> Vec<Stamp2<CircuitReference, f64>>;

    fn initial_dc_values(&self, _context: &Context) -> Vec<InitialValue<CircuitReference, f64>> {
        Vec::new()
    }
}

#[derive(Debug)]
pub struct DcAnalysisResult {
    pub values: HashMap<Arc<CircuitVariable>, f64>,
    pub soa_violations: Vec<SoaViolation>,
}

impl DcAnalysisResult {
    pub fn get_value(&self, reference: &CircuitReference) -> Option<f64> {
        self.values.get(reference.variable()).cloned()
    }
}
