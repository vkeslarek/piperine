use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::analysis::dc::DcAnalysisResult;
use crate::devices::voltage_source::VoltageSource;
use crate::math::linear::Stamp;
use crate::circuit::netlist::CircuitReference;
use crate::solver::Context;
use num_complex::Complex;
use num_traits::One;

impl AcAnalysis for VoltageSource {
    fn load_ac(
        &self,
        _: &DcAnalysisResult,
        _: &AcAnalysisContext,
        _: &Context,
    ) -> Vec<Stamp<CircuitReference, Complex<f64>>> {
        let ac_volt = Complex::new(1.0, 0.0);

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
            Stamp::Rhs(self.branch.clone(), ac_volt),
        ]
    }
}
