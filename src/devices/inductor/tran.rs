use crate::analysis::transient::{TransientModelInstance, TransientAnalysisContext};
use crate::devices::inductor::Inductor;
use crate::math::linear::Stamp;
use crate::netlist::CircuitReference;
use crate::solver::Context;
use crate::state::CircuitState;

impl TransientModelInstance for Inductor {
    fn load_transient(
        &self,
        circuit_states: &CircuitState<f64>,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        let dt = transient_analysis_context.dt;

        // 1. Get differentiation coefficients for the branch current
        // V = L * di/dt
        // di/dt = (alpha_0 * i_now + history_sum) / dt
        let (alpha_0, history_sum) = circuit_states.derivative_coefficients(&self.branch);

        // 2. Linearize the relationship:
        // V_now = (L/dt) * (alpha_0 * i_now + history_sum)
        // V_now = [(L * alpha_0) / dt] * i_now + [L * history_sum / dt]

        let req = (self.inductance * alpha_0) / dt;
        let v_hist = (self.inductance * history_sum) / dt;

        vec![
            Stamp::Matrix(self.branch.clone(), self.node_plus.clone(), 1.0),
            Stamp::Matrix(self.branch.clone(), self.node_minus.clone(), -1.0),
            Stamp::Matrix(self.branch.clone(), self.branch.clone(), -req.value),
            Stamp::Rhs(self.branch.clone(), v_hist.value),
            Stamp::Matrix(self.node_plus.clone(), self.branch.clone(), 1.0),
            Stamp::Matrix(self.node_minus.clone(), self.branch.clone(), -1.0),
        ]
    }
    fn check_convergence(
        &self,
        state: &CircuitState<f64>,
        _: &TransientAnalysisContext,
        context: &Context,
    ) -> bool {
        // Lookback 0: The solution the solver just produced
        // Lookback 1: The guess we used for this specific NR iteration
        let i_now = state.get_commited_value(&self.branch, 0).unwrap_or(0.0);
        let i_prev = state.get_commited_value(&self.branch, 1).unwrap_or(0.0);

        let diff = (i_now - i_prev).abs();

        // We use abstol (absolute tolerance) and reltol (relative tolerance)
        // Standard SPICE values: abstol = 1pA, reltol = 1e-3
        let rel_tol = context.reltol * i_now.abs().max(i_prev.abs());
        let abs_tol = context.abstol;

        // Converged if the change is below the absolute threshold
        // OR below the relative threshold
        diff < abs_tol || diff < rel_tol
    }
}
