use crate::analysis::transient::{TransientAnalysis, TransientAnalysisContext, TransientAnalysisState};
use crate::circuit::netlist::CircuitReference;
use crate::devices::capacitor::Capacitor;
use crate::math::linear::Stamp2;
use crate::solver::Context;

impl TransientAnalysis for Capacitor {
    fn load_transient(
        &self,
        _: &TransientAnalysisState,
        _: &TransientAnalysisContext,
        _: &Context,
    ) -> Vec<Stamp2<CircuitReference, f64>> {
        vec![]
    }

    fn load_transient_dynamic(
        &self,
        _: &TransientAnalysisState,
        _: &TransientAnalysisContext,
        _: &Context,
    ) -> Vec<Stamp2<CircuitReference, f64>> {
        let c = self.capacitance;

        vec![
            Stamp2::Matrix(self.node_plus.clone(), self.node_plus.clone(), c),
            Stamp2::Matrix(self.node_minus.clone(), self.node_minus.clone(), c),
            Stamp2::Matrix(self.node_plus.clone(), self.node_minus.clone(), -c),
            Stamp2::Matrix(self.node_minus.clone(), self.node_plus.clone(), -c),
        ]
    }
}
