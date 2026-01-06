use crate::analysis::transient::{TransientAnalysis, TransientAnalysisContext};
use crate::devices::capacitor::Capacitor;
use crate::math::linear::Stamp;
use crate::netlist::CircuitReference;
use crate::solver::Context;
use crate::state::CircuitState;

impl TransientAnalysis for Capacitor {
    fn load_transient(
        &self,
        states: &CircuitState<f64>,
        _trans_context: &TransientAnalysisContext,
        _context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        // states.derivative_coefficients already returns values in units of 1/s
        let (alpha_0, history_sum_p) = states.derivative_coefficients(&self.node_plus);
        let (_, history_sum_m) = states.derivative_coefficients(&self.node_minus);

        // Geq = C * alpha_0
        let g_eq = self.capacitance.value * alpha_0;
        // Ieq = C * (hist_p - hist_m)
        let i_hist = self.capacitance.value * (history_sum_p - history_sum_m);

        vec![
            Stamp::Matrix(self.node_plus.clone(), self.node_plus.clone(), g_eq),
            Stamp::Matrix(self.node_minus.clone(), self.node_minus.clone(), g_eq),
            Stamp::Matrix(self.node_plus.clone(), self.node_minus.clone(), -g_eq),
            Stamp::Matrix(self.node_minus.clone(), self.node_plus.clone(), -g_eq),
            Stamp::Rhs(self.node_plus.clone(), -i_hist),
            Stamp::Rhs(self.node_minus.clone(), i_hist),
        ]
    }
}
