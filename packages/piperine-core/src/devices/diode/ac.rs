use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::devices::diode::Diode;
use crate::math::linear::Stamp;
use crate::netlist::CircuitReference;
use crate::solver::Context;
use crate::state::CircuitState;
use num_complex::Complex;

impl AcAnalysis for Diode {
    fn load_ac(
        &self,
        _: &CircuitState<Complex<f64>>,
        _: &AcAnalysisContext,
        _: &Context,
    ) -> Vec<Stamp<CircuitReference, Complex<f64>>> {
        // Use the g_eq calculated during the final OP/Transient step
        let g = Complex::new(self.g_eq.value, 0.0);

        vec![
            Stamp::Matrix(self.node_plus.clone(), self.node_plus.clone(), g),
            Stamp::Matrix(self.node_minus.clone(), self.node_minus.clone(), g),
            Stamp::Matrix(self.node_plus.clone(), self.node_minus.clone(), -g),
            Stamp::Matrix(self.node_minus.clone(), self.node_plus.clone(), -g),
        ]
    }
}
