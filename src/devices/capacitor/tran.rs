use crate::analysis::transient::{TransientAnalysis, TransientAnalysisContext};
use crate::circuit::netlist::CircuitReference;
use crate::circuit::state::CircuitState;
use crate::devices::capacitor::Capacitor;
use crate::math::linear::Stamp;
use crate::solver::Context;

impl TransientAnalysis for Capacitor {
    fn load_transient(
        &self,
        _: &CircuitState<f64>,
        _: &TransientAnalysisContext,
        _: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        vec![]
    }

    fn load_transient_dynamic(
        &self,
        _: &CircuitState<f64>,
        _: &TransientAnalysisContext,
        _: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        let c = self.capacitance.value;

        vec![
            Stamp::Matrix(self.node_plus.clone(), self.node_plus.clone(), c),
            Stamp::Matrix(self.node_minus.clone(), self.node_minus.clone(), c),
            Stamp::Matrix(self.node_plus.clone(), self.node_minus.clone(), -c),
            Stamp::Matrix(self.node_minus.clone(), self.node_plus.clone(), -c),
        ]
    }
}
