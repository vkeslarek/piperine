use crate::analysis::transient::{
    TransientAnalysis, TransientAnalysisContext, TransientAnalysisState,
};
use crate::circuit::netlist::CircuitReference;
use crate::devices::diode::Diode;
use crate::math::linear::Stamp;
use crate::solver::Context;

impl TransientAnalysis for Diode {
    fn update_transient(
        &mut self,
        state: &TransientAnalysisState,
        _transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> crate::result::Result<()> {
        let v_anode_new = state
            .latest()
            .and_then(|val| val.get(&self.node_plus).cloned())
            .unwrap_or(0.0);
        let v_cathode_new = state
            .latest()
            .and_then(|val| val.get(&self.node_minus).cloned())
            .unwrap_or(0.0);

        let v_anode_old = state
            .view(1)
            .and_then(|val| val.get(&self.node_plus).cloned())
            .unwrap_or(0.0);
        let v_cathode_old = state
            .view(1)
            .and_then(|val| val.get(&self.node_minus).cloned())
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
        _circuit_states: &TransientAnalysisState,
        _transient_analysis_context: &TransientAnalysisContext,
        _context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        let g = self.g_eq;
        let i_rhs = self.i_eq;

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
