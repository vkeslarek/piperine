use crate::circuit::netlist::{CircuitReference, CircuitVariable};
use crate::devices::Component;
use crate::math::array::IndexedArray2;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::iv::{InitialValue, InitialValue2};
use crate::math::linear::{Stamp, Stamp2};
use crate::math::unit::Second;
use crate::math::vector::IndexedVec1;
use crate::solver::Context;
use crate::solver::transient::TransientStep;

pub type TransientAnalysisState = CircularArrayBuffer2<f64>;

#[derive(Clone)]
pub struct TransientAnalysisOptions {
    pub stop_time: Second,
    pub dt: Second,
}

#[derive(Clone)]
pub struct TransientAnalysisContext {
    pub time: Second,
    pub dt: Second,
}

pub trait TransientAnalysis: Component {
    fn update_transient(
        &mut self,
        _circuit_states: &TransientAnalysisState,
        _transient_analysis_context: &TransientAnalysisContext,
        _context: &Context,
    ) -> crate::result::Result<()> {
        Ok(())
    }

    fn load_transient(
        &self,
        circuit_states: &TransientAnalysisState,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp2<CircuitReference, f64>>;

    fn load_transient_dynamic(
        &self,
        _circuit_states: &TransientAnalysisState,
        _transient_analysis_context: &TransientAnalysisContext,
        _context: &Context,
    ) -> Vec<Stamp2<CircuitReference, f64>> {
        vec![]
    }

    fn initial_transient_values(
        &self,
        _context: &Context,
    ) -> Vec<InitialValue2<CircuitReference, f64>> {
        Vec::new()
    }
}

#[derive(Debug, Clone)]
pub struct TransientAnalysisResult {
    pub values: Vec<TransientStep>,
}
