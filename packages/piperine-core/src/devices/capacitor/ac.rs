use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::devices::capacitor::Capacitor;
use crate::math::linear::Stamp;
use crate::netlist::CircuitReference;
use crate::solver::Context;
use crate::state::CircuitState;
use num_complex::Complex;

impl AcAnalysis for Capacitor {
    fn load_ac(
        &self,
        _circuit_states: &CircuitState<Complex<f64>>,
        ac_analysis_context: &AcAnalysisContext,
        _context: &Context,
    ) -> Vec<Stamp<CircuitReference, Complex<f64>>> {
        // Convert Hz to rad/s
        let omega = 2.0 * std::f64::consts::PI * ac_analysis_context.frequency;

        // Y = j * omega * C
        // We create a Complex number with 0.0 real part and (omega * C) imaginary part
        let y = Complex::new(0.0, omega.value * self.capacitance.value);

        vec![
            Stamp::Matrix(self.node_plus.clone(), self.node_plus.clone(), y),
            Stamp::Matrix(self.node_minus.clone(), self.node_minus.clone(), y),
            Stamp::Matrix(self.node_plus.clone(), self.node_minus.clone(), -y),
            Stamp::Matrix(self.node_minus.clone(), self.node_plus.clone(), -y),
        ]
    }
}
