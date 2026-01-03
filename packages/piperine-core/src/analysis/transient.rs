use crate::component::{Component, Context};
use crate::math::unit::Conductance;
use crate::solver::Stamp;
use crate::state::CircuitStates;

#[derive(Clone)]
pub struct TransientAnalysisContext {
    pub time: f64,
    pub dt: f64,
}

pub trait TransientAnalysis: Component {
    fn load_transient(
        &self,
        circuit_states: &CircuitStates,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<Conductance>>;

    fn check_convergence(
        &self,
        circuit_states: &CircuitStates,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> bool {
        true
    }
}
