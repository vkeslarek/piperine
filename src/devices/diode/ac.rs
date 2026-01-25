use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::analysis::dc::DcAnalysisResult;
use crate::circuit::netlist::CircuitReference;
use crate::devices::diode::Diode;
use crate::math::linear::Stamp;
use crate::solver::Context;
use num_complex::Complex;

impl AcAnalysis for Diode {
    fn update_ac(
        &mut self,
        dc_analysis_result: &DcAnalysisResult,
        _ac_analysis_context: &AcAnalysisContext,
        context: &Context,
    ) -> crate::result::Result<()> {
        let v_plus = dc_analysis_result
            .get(self.node_plus.clone())
            .unwrap_or(0.0);
        let v_minus = dc_analysis_result
            .get(self.node_minus.clone())
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
        let g_d = Complex::new(self.g_eq, 0.0);

        vec![
            Stamp::Matrix(self.node_plus.clone(), self.node_plus.clone(), g_d),
            Stamp::Matrix(self.node_minus.clone(), self.node_minus.clone(), g_d),
            Stamp::Matrix(self.node_plus.clone(), self.node_minus.clone(), -g_d),
            Stamp::Matrix(self.node_minus.clone(), self.node_plus.clone(), -g_d),
        ]
    }
}
