use crate::analysis::transient::{TransientModelInstance, TransientAnalysisContext};
use crate::devices::diode::Diode;
use crate::math::linear::Stamp;
use crate::math::unit::UnitExt;
use crate::netlist::CircuitReference;
use crate::solver::Context;
use crate::state::CircuitState;

impl TransientModelInstance for Diode {
    fn update_transient(
        &mut self,
        circuit_states: &CircuitState<f64>,
        _: &TransientAnalysisContext,
        _: &Context,
    ) -> crate::error::Result<()> {
        let v_plus = circuit_states
            .get_guess_value(&self.node_plus)
            .unwrap_or(0.0);
        let v_minus = circuit_states
            .get_guess_value(&self.node_minus)
            .unwrap_or(0.0);

        // STORE THE RAW GUESS. Do not touch v_linearized!
        self.v_guess = (v_plus - v_minus).V();
        Ok(())
    }

    fn load_transient(
        &self,
        _states: &CircuitState<f64>,
        _ctx: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        let g = self.g_eq.value;
        let i = self.i_eq.value;

        vec![
            Stamp::Matrix(self.node_plus.clone(), self.node_plus.clone(), g),
            Stamp::Matrix(self.node_minus.clone(), self.node_minus.clone(), g),
            Stamp::Matrix(self.node_plus.clone(), self.node_minus.clone(), -g),
            Stamp::Matrix(self.node_minus.clone(), self.node_plus.clone(), -g),
            // RHS: Current Id flows PLUS -> MINUS
            // So we subtract from the PLUS node and add to the MINUS node
            Stamp::Rhs(self.node_plus.clone(), -i),
            Stamp::Rhs(self.node_minus.clone(), i),
        ]
    }
    fn check_convergence(
        &self,
        circuit_states: &CircuitState<f64>,
        _ctx: &TransientAnalysisContext,
        context: &Context,
    ) -> bool {
        let v_now = circuit_states
            .get_guess_value(&self.node_plus)
            .unwrap_or(0.0)
            - circuit_states
                .get_guess_value(&self.node_minus)
                .unwrap_or(0.0);

        // Compare the Solver's Result (v_now) against the Voltage we Linearized Around (v_linearized)
        let v_lin = self.v_linearized.value;

        (v_now - v_lin).abs() < (context.reltol * v_now.abs().max(v_lin.abs()) + context.vntol)
    }
}
