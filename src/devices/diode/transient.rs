use crate::analysis::transient::{TransientAnalysis, TransientAnalysisContext};
use crate::circuit::netlist::CircuitReference;
use crate::circuit::state::CircuitState;
use crate::devices::diode::Diode;
use crate::math::linear::Stamp;
use crate::solver::Context;

impl TransientAnalysis for Diode {
    fn update_transient(
        &mut self,
        state: &CircuitState<f64>,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> crate::result::Result<()> {
        let v_anode_new = state.get_dependent_value(&self.node_plus, 0).unwrap_or(0.0);
        let v_cathode_new = state
            .get_dependent_value(&self.node_minus, 0)
            .unwrap_or(0.0);

        let v_anode_old = state.get_dependent_value(&self.node_plus, 1).unwrap_or(0.0);
        let v_cathode_old = state
            .get_dependent_value(&self.node_minus, 1)
            .unwrap_or(0.0);

        self.model.clone().update_linearization(
            self,
            v_anode_new - v_cathode_new,
            v_anode_old - v_cathode_old,
            context,
        );
        Ok(())
    }

    fn load_transient(
        &self,
        circuit_states: &CircuitState<f64>,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        let g = self.g_eq.value;
        let i_rhs = self.i_eq.value;

        // MNA Stamps for a Conductor + Current Source in parallel
        vec![
            // Matrix: Conductance Terms (Similar to Resistor)
            Stamp::Matrix(self.node_plus.clone(), self.node_plus.clone(), g),
            Stamp::Matrix(self.node_minus.clone(), self.node_minus.clone(), g),
            Stamp::Matrix(self.node_plus.clone(), self.node_minus.clone(), -g),
            Stamp::Matrix(self.node_minus.clone(), self.node_plus.clone(), -g),
            // RHS Vector: Current Source Terms
            Stamp::Rhs(self.node_plus.clone(), -i_rhs),
            Stamp::Rhs(self.node_minus.clone(), i_rhs),
        ]
    }
}
