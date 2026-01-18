use crate::circuit::netlist::CircuitReference;
use crate::devices::Component;
use crate::devices::soa::SoaViolation;
use crate::math::array::{IndexedArray1, IndexedArray2};
use crate::math::iv::InitialValue;
use crate::math::linear::Stamp;
use crate::solver::Context;

pub type DcAnalysisState = IndexedArray2<CircuitReference, f64>;

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
    ) -> Vec<Stamp<CircuitReference, f64>>;

    fn initial_dc_values(&self, _context: &Context) -> Vec<InitialValue<CircuitReference, f64>> {
        Vec::new()
    }
}

#[derive(Debug)]
pub struct DcAnalysisResult {
    pub values: IndexedArray1<CircuitReference, f64>,
    pub soa_violations: Vec<SoaViolation>,
}

impl DcAnalysisResult {
    pub fn get_value(&self, reference: &CircuitReference) -> Option<f64> {
        self.values.get(reference).cloned()
    }
}
