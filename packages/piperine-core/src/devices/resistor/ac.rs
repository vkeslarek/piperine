use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::devices::resistor::Resistor;
use crate::math::linear::Stamp;
use crate::math::unit::AdmittanceConvert;
use crate::netlist::CircuitReference;
use crate::solver::Context;
use crate::state::CircuitState;
use num_complex::Complex;

impl AcAnalysis for Resistor {
    fn load_ac(
        &self,
        _circuit_states: &CircuitState<Complex<f64>>,
        _: &AcAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, Complex<f64>>> {
        let g = self.conductance.to_admittance();

        vec![
            Stamp::Matrix(self.node_plus.clone(), self.node_plus.clone(), g.value),
            Stamp::Matrix(self.node_minus.clone(), self.node_minus.clone(), g.value),
            Stamp::Matrix(self.node_plus.clone(), self.node_minus.clone(), -g.value),
            Stamp::Matrix(self.node_minus.clone(), self.node_plus.clone(), -g.value),
        ]
    }
}
