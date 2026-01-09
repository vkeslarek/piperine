use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::analysis::dc::DcAnalysisResult;
use crate::devices::capacitor::Capacitor;
use crate::math::linear::Stamp;
use crate::math::unit::ReactanceConvert;
use crate::netlist::CircuitReference;
use crate::solver::Context;
use num_complex::Complex;

impl AcAnalysis for Capacitor {
    fn load_ac(
        &self,
        _: &DcAnalysisResult,
        ac_analysis_context: &AcAnalysisContext,
        _: &Context,
    ) -> Vec<Stamp<CircuitReference, Complex<f64>>> {
        let impedance = self.capacitance.to_impedance(ac_analysis_context.frequency);

        vec![
            Stamp::Matrix(
                self.node_plus.clone(),
                self.node_plus.clone(),
                impedance.value,
            ),
            Stamp::Matrix(
                self.node_minus.clone(),
                self.node_minus.clone(),
                impedance.value,
            ),
            Stamp::Matrix(
                self.node_plus.clone(),
                self.node_minus.clone(),
                -impedance.value,
            ),
            Stamp::Matrix(
                self.node_minus.clone(),
                self.node_plus.clone(),
                -impedance.value,
            ),
        ]
    }
}
