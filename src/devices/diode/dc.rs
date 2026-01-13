use crate::analysis::dc::DcAnalysis;
use crate::circuit::netlist::CircuitReference;
use crate::circuit::state::CircuitState;
use crate::devices::diode::Diode;
use crate::math::Stamp;
use crate::solver::Context;

impl DcAnalysis for Diode {
    fn update_dc(
        &mut self,
        state: &CircuitState<f64>,
        context: &Context,
    ) -> crate::result::Result<()> {
        let v_anode = state.get_dependent_value(&self.node_plus, 0).unwrap_or(0.0);
        let v_cathode = state
            .get_dependent_value(&self.node_minus, 0)
            .unwrap_or(0.0);

        self.model.clone().update_linearization(
            self,
            v_anode - v_cathode,
            v_anode - v_cathode,
            context,
        );
        Ok(())
    }

    fn load_dc(&self, _: &CircuitState<f64>, _: &Context) -> Vec<Stamp<CircuitReference, f64>> {
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
