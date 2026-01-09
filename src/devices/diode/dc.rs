use crate::analysis::dc::{DcAnalysis, DcCircuitState};
use crate::devices::diode::Diode;
use crate::math::linear::Stamp;
use crate::netlist::CircuitReference;
use crate::solver::Context;

impl DcAnalysis for Diode {
    fn update_dc(&mut self, state: &DcCircuitState, context: &Context) -> crate::result::Result<()> {
        self.model
            .clone()
            .update_linearization(self, state, context);
        Ok(())
    }

    fn load_dc(&self, _: &DcCircuitState, _: &Context) -> Vec<Stamp<CircuitReference, f64>> {
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
