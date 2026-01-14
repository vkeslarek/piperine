use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::analysis::dc::DcAnalysisResult;
use crate::circuit::netlist::CircuitReference;
use crate::devices::diode::Diode;
use crate::math::Stamp;
use crate::solver::Context;
use num_complex::Complex;

impl AcAnalysis for Diode {
    fn update_ac(
        &mut self,
        dc_analysis_result: &DcAnalysisResult,
        _ac_analysis_context: &AcAnalysisContext,
        context: &Context,
    ) -> crate::result::Result<()> {
        // 1. Get the converged DC voltages
        let v_plus = dc_analysis_result.get_value(&self.node_plus).unwrap_or(0.0);
        let v_minus = dc_analysis_result
            .get_value(&self.node_minus)
            .unwrap_or(0.0);
        let v_d = v_plus - v_minus;

        self.model
            .clone()
            .update_linearization(self, v_d, v_d, context);

        Ok(())
    }

    fn load_ac(
        &self,
        _dc_analysis_result: &DcAnalysisResult,
        _ac_analysis_context: &AcAnalysisContext,
        _context: &Context,
    ) -> Vec<Stamp<CircuitReference, Complex<f64>>> {
        // Use the g_eq calculated during update_ac (or the last DC iteration)
        // In AC, g_eq is a real conductance.
        let g_d = Complex::new(self.g_eq, 0.0);

        // Note: For high-frequency AC, you would eventually add
        // Junction Capacitance here: Complex::new(g_d, omega * C_j)

        vec![
            Stamp::Matrix(self.node_plus.clone(), self.node_plus.clone(), g_d),
            Stamp::Matrix(self.node_minus.clone(), self.node_minus.clone(), g_d),
            Stamp::Matrix(self.node_plus.clone(), self.node_minus.clone(), -g_d),
            Stamp::Matrix(self.node_minus.clone(), self.node_plus.clone(), -g_d),
        ]
    }
}
