use crate::analysis::transient::{
    TransientAnalysis, TransientAnalysisContext, TransientAnalysisState,
};
use crate::circuit::netlist::CircuitReference;
use crate::devices::inductor::Inductor;
use crate::math::linear::Stamp;
use crate::solver::Context;

impl TransientAnalysis for Inductor {
    fn load_transient(
        &self,
        _: &TransientAnalysisState,
        _: &TransientAnalysisContext,
        _: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        vec![
            Stamp::Matrix(self.node_plus.clone(), self.current_ref.clone(), 1.0),
            Stamp::Matrix(self.node_minus.clone(), self.current_ref.clone(), -1.0),
            Stamp::Matrix(self.current_ref.clone(), self.node_plus.clone(), 1.0),
            Stamp::Matrix(self.current_ref.clone(), self.node_minus.clone(), -1.0),
        ]
    }

    fn load_transient_dynamic(
        &self,
        _: &TransientAnalysisState,
        _: &TransientAnalysisContext,
        _: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        let l = self.inductance;

        vec![Stamp::Matrix(
            self.current_ref.clone(),
            self.current_ref.clone(),
            -l,
        )]
    }
}
