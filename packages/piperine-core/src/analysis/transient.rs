use crate::component::Component;
use crate::math::linear::Stamp;
use crate::netlist::CircuitReference;
use crate::solver::Context;
use crate::state::CircuitState;

#[derive(Clone)]
pub struct TransientAnalysisContext {
    pub time: f64,
    pub dt: f64,
}

pub trait TransientAnalysis: Component {
    fn update_transient(
        &self,
        circuit_states: &CircuitState<f64>,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> crate::error::Result<()> {
        Ok(())
    }

    fn load_transient(
        &self,
        circuit_states: &CircuitState<f64>,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>>;

    fn check_convergence(
        &self,
        circuit_states: &CircuitState<f64>,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> bool {
        true
    }
}
