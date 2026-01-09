use crate::analysis::ac::{AcModelInstance, AcAnalysisContext};
use crate::devices::inductor::Inductor;
use crate::math::linear::Stamp;
use crate::math::unit::ReactanceConvert;
use crate::netlist::CircuitReference;
use crate::solver::Context;
use crate::state::CircuitState;
use num_complex::Complex;
use num_traits::One;

impl AcModelInstance for Inductor {
    fn load_ac(
        &self,
        _circuit_states: &CircuitState<Complex<f64>>,
        ac_analysis_context: &AcAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, Complex<f64>>> {
        let z = self.inductance.to_impedance(ac_analysis_context.frequency);

        vec![
            Stamp::Matrix(self.node_plus.clone(), self.branch.clone(), Complex::one()),
            Stamp::Matrix(
                self.node_minus.clone(),
                self.branch.clone(),
                -Complex::one(),
            ),
            Stamp::Matrix(self.branch.clone(), self.node_plus.clone(), Complex::one()),
            Stamp::Matrix(
                self.branch.clone(),
                self.node_minus.clone(),
                -Complex::one(),
            ),
            Stamp::Matrix(self.branch.clone(), self.branch.clone(), -z.value),
        ]
    }
}
