use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::analysis::dc::DcAnalysisResult;
use crate::circuit::netlist::CircuitReference;
use crate::devices::capacitor::Capacitor;
use crate::math::linear::Stamp2;
use crate::solver::Context;
use num_complex::Complex;

impl AcAnalysis for Capacitor {
    fn load_ac(
        &self,
        _: &DcAnalysisResult,
        ac_analysis_context: &AcAnalysisContext,
        _: &Context,
    ) -> Vec<Stamp2<CircuitReference, Complex<f64>>> {
        let omega = 2.0 * std::f64::consts::PI * ac_analysis_context.frequency;
        let cap_val = self.capacitance;

        let admittance = Complex::new(0.0, omega * cap_val);

        vec![
            Stamp2::Matrix(self.node_plus.clone(), self.node_plus.clone(), admittance),
            Stamp2::Matrix(self.node_minus.clone(), self.node_minus.clone(), admittance),
            Stamp2::Matrix(self.node_plus.clone(), self.node_minus.clone(), -admittance),
            Stamp2::Matrix(self.node_minus.clone(), self.node_plus.clone(), -admittance),
        ]
    }
}
