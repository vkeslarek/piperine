use crate::circuit::netlist::CircuitReference;
use crate::devices::Component;
use crate::math::array::IndexedArray2;
use crate::math::iv::InitialValue;
use crate::math::linear::Stamp;
use crate::math::unit::Second;
use crate::math::vector::IndexedVec1;
use crate::solver::Context;

pub type TransientAnalysisState = IndexedArray2<CircuitReference, f64>;

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
        circuit_states: &TransientAnalysisState,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> crate::result::Result<()> {
        Ok(())
    }

    fn load_transient(
        &self,
        circuit_states: &TransientAnalysisState,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>>;

    fn load_transient_dynamic(
        &self,
        circuit_states: &TransientAnalysisState,
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

pub type TransientAnalysisResult = IndexedVec1<CircuitReference, f64>;
